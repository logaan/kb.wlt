use kb_core::pipeline::{
    Action, Bounded, Direction, Input, Key, KeyEvent, KeyState, Middleware, Output, Pipeline,
    PipelineError, PipelineState, StageError, TransformedOutput,
};

struct Arithmetic {
    add: u16,
    multiply: u16,
}

impl Middleware<8, 4, 8> for Arithmetic {
    fn process(&self, state: &[u8], input: &Input<4>) -> Result<Output<8, 4, 8>, StageError> {
        let mut output = Output::pass(state, input)?;
        for event in output.events.as_mut_slice() {
            event.position += self.add;
        }
        let previous = state.first().copied().unwrap_or(0);
        output.state = Bounded::from_slice(&[previous + 1, self.add as u8], 0)?;
        Ok(output)
    }

    fn transform_output(
        &self,
        state: &[u8],
        actions: &[Action],
    ) -> Result<TransformedOutput<8, 8>, StageError> {
        let mut actions = Bounded::from_slice(actions, Action::ReleaseAll)?;
        for action in actions.as_mut_slice() {
            match action {
                Action::Press(key) | Action::Release(key) | Action::Tap(key) => {
                    key.usage = key.usage * self.multiply + self.add;
                }
                Action::ReleaseAll => {}
            }
        }
        Ok(TransformedOutput {
            state: Bounded::from_slice(&[state[0], state[1], 99], 0)?,
            actions,
        })
    }
}

struct Tap;

impl Middleware<8, 4, 8> for Tap {
    fn process(&self, state: &[u8], input: &Input<4>) -> Result<Output<8, 4, 8>, StageError> {
        let mut output = Output::pass(state, input)?;
        for event in input.events.as_slice() {
            if event.state == KeyState::Pressed {
                output.actions.push(Action::Tap(Key {
                    usage: event.position,
                    modifiers: 0,
                }))?;
            }
        }
        output.events = Bounded::new(KeyEvent::EMPTY);
        Ok(output)
    }
}

#[test]
fn incoming_order_and_outgoing_reverse_order_preserve_isolated_stage_state() {
    let first = Arithmetic {
        add: 1,
        multiply: 2,
    };
    let second = Arithmetic {
        add: 3,
        multiply: 5,
    };
    let pipeline = Pipeline::new([&first as &dyn Middleware<8, 4, 8>, &second, &Tap]);
    let input = Input::with_events(
        50,
        &[KeyEvent {
            position: 4,
            state: KeyState::Pressed,
            timestamp_ms: 49,
        }],
    )
    .unwrap();
    let initial = PipelineState::new();
    let result = pipeline.process(initial, input).unwrap();
    assert_eq!(
        result.actions.as_slice(),
        &[Action::Tap(Key {
            usage: 87,
            modifiers: 0
        })]
    );
    assert!(result.events.is_empty());
    assert_eq!(result.state.stages[0].as_slice(), &[1, 1, 99]);
    assert_eq!(result.state.stages[1].as_slice(), &[1, 3, 99]);
    assert!(result.state.stages[2].is_empty());
    assert!(initial.stages.iter().all(Bounded::is_empty));
    let result = pipeline.process(result.state, Input::tick(60)).unwrap();
    assert_eq!(result.state.stages[0].as_slice(), &[2, 1, 99]);
    assert_eq!(result.state.stages[1].as_slice(), &[2, 3, 99]);
    assert!(result.actions.is_empty());
}

struct Wake(u32);

impl Middleware<8, 4, 8> for Wake {
    fn process(&self, state: &[u8], input: &Input<4>) -> Result<Output<8, 4, 8>, StageError> {
        let mut output = Output::pass(state, input)?;
        output.wake_at_ms = Some(self.0);
        Ok(output)
    }
}

#[test]
fn wake_deadlines_select_the_earliest_across_clock_wrap() {
    let early = Wake(u32::MAX - 1);
    let later = Wake(3);
    let pipeline = Pipeline::new([&later as &dyn Middleware<8, 4, 8>, &early]);
    let result = pipeline
        .process(PipelineState::new(), Input::tick(u32::MAX - 3))
        .unwrap();
    assert_eq!(result.wake_at_ms, Some(u32::MAX - 1));
    let result = pipeline
        .process(PipelineState::new(), Input::tick(0))
        .unwrap();
    assert_eq!(result.wake_at_ms, Some(0));
}

struct Emit;

impl Middleware<8, 4, 1> for Emit {
    fn process(&self, state: &[u8], input: &Input<4>) -> Result<Output<8, 4, 1>, StageError> {
        let mut output = Output::pass(state, input)?;
        output.state.push(1)?;
        output.actions.push(Action::ReleaseAll)?;
        Ok(output)
    }
}

#[test]
fn aggregate_overflow_is_transactional_and_identifies_the_emitting_stage() {
    let pipeline = Pipeline::new([&Emit as &dyn Middleware<8, 4, 1>, &Emit]);
    let initial = PipelineState::new();
    assert_eq!(
        pipeline.process(initial, Input::tick(0)),
        Err(PipelineError {
            stage: 1,
            direction: Direction::Input,
            cause: StageError::Capacity
        })
    );
    assert!(initial.stages.iter().all(Bounded::is_empty));
}

struct FailOutput;

impl Middleware<8, 4, 8> for FailOutput {
    fn transform_output(
        &self,
        _: &[u8],
        _: &[Action],
    ) -> Result<TransformedOutput<8, 8>, StageError> {
        Err(StageError::Trap(17))
    }
}

#[test]
fn output_failure_keeps_original_state_and_reports_direction() {
    let first = Arithmetic {
        add: 1,
        multiply: 2,
    };
    let pipeline = Pipeline::new([&first as &dyn Middleware<8, 4, 8>, &FailOutput]);
    let initial = PipelineState::new();
    assert_eq!(
        pipeline.process(initial, Input::tick(0)),
        Err(PipelineError {
            stage: 1,
            direction: Direction::Output,
            cause: StageError::Trap(17)
        })
    );
    assert!(initial.stages.iter().all(Bounded::is_empty));
}

#[test]
fn empty_pipeline_passes_events_and_does_not_schedule_a_wake() {
    let pipeline = Pipeline::<0, 0, 1, 0>::new([]);
    let input = Input::with_events(0, &[KeyEvent::EMPTY]).unwrap();
    let result = pipeline.process(PipelineState::new(), input).unwrap();
    assert_eq!(result.events, input.events);
    assert_eq!(result.wake_at_ms, None);
    assert!(result.actions.is_empty());
}

#[test]
fn bounded_extension_rejects_overflow_without_partial_writes() {
    let mut buffer = Bounded::<u8, 3>::from_slice(&[1], 0).unwrap();
    assert!(buffer.extend_from_slice(&[2, 3, 4]).is_err());
    assert_eq!(buffer.as_slice(), &[1]);
    buffer.extend_from_slice(&[2, 3]).unwrap();
    assert_eq!(buffer.as_slice(), &[1, 2, 3]);
}
