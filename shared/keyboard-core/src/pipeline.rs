#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CapacityError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bounded<T: Copy, const N: usize> {
    items: [T; N],
    len: usize,
}

impl<T: Copy + Default, const N: usize> Default for Bounded<T, N> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T: Copy, const N: usize> Bounded<T, N> {
    pub const fn new(empty: T) -> Self {
        Self {
            items: [empty; N],
            len: 0,
        }
    }

    pub fn from_slice(items: &[T], empty: T) -> Result<Self, CapacityError> {
        let mut buffer = Self::new(empty);
        buffer.extend_from_slice(items)?;
        Ok(buffer)
    }

    pub fn as_slice(&self) -> &[T] {
        &self.items[..self.len]
    }

    pub fn as_mut_slice(&mut self) -> &mut [T] {
        &mut self.items[..self.len]
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn push(&mut self, item: T) -> Result<(), CapacityError> {
        if self.len == N {
            return Err(CapacityError);
        }
        self.items[self.len] = item;
        self.len += 1;
        Ok(())
    }

    pub fn extend_from_slice(&mut self, items: &[T]) -> Result<(), CapacityError> {
        if items.len() > N - self.len {
            return Err(CapacityError);
        }
        self.items[self.len..self.len + items.len()].copy_from_slice(items);
        self.len += items.len();
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum KeyState {
    #[default]
    Released,
    Pressed,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KeyEvent {
    pub position: u16,
    pub state: KeyState,
    pub timestamp_ms: u32,
}

impl KeyEvent {
    pub const EMPTY: Self = Self {
        position: 0,
        state: KeyState::Released,
        timestamp_ms: 0,
    };
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Key {
    pub usage: u16,
    pub modifiers: u8,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Action {
    Press(Key),
    Release(Key),
    Tap(Key),
    #[default]
    ReleaseAll,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Input<const E: usize> {
    pub events: Bounded<KeyEvent, E>,
    pub now_ms: u32,
}

impl<const E: usize> Input<E> {
    pub const fn tick(now_ms: u32) -> Self {
        Self {
            events: Bounded::new(KeyEvent::EMPTY),
            now_ms,
        }
    }

    pub fn with_events(now_ms: u32, events: &[KeyEvent]) -> Result<Self, CapacityError> {
        Ok(Self {
            events: Bounded::from_slice(events, KeyEvent::EMPTY)?,
            now_ms,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Output<const S: usize, const E: usize, const A: usize> {
    pub state: Bounded<u8, S>,
    pub events: Bounded<KeyEvent, E>,
    pub actions: Bounded<Action, A>,
    pub wake_at_ms: Option<u32>,
}

impl<const S: usize, const E: usize, const A: usize> Output<S, E, A> {
    pub fn pass(state: &[u8], input: &Input<E>) -> Result<Self, StageError> {
        Ok(Self {
            state: Bounded::from_slice(state, 0)?,
            events: input.events,
            actions: Bounded::new(Action::ReleaseAll),
            wake_at_ms: None,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TransformedOutput<const S: usize, const A: usize> {
    pub state: Bounded<u8, S>,
    pub actions: Bounded<Action, A>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StageError {
    Capacity,
    InvalidState,
    Trap(u16),
}

impl From<CapacityError> for StageError {
    fn from(_: CapacityError) -> Self {
        Self::Capacity
    }
}

pub trait Middleware<const S: usize, const E: usize, const A: usize> {
    fn process(&self, state: &[u8], input: &Input<E>) -> Result<Output<S, E, A>, StageError> {
        Output::pass(state, input)
    }

    fn transform_output(
        &self,
        state: &[u8],
        actions: &[Action],
    ) -> Result<TransformedOutput<S, A>, StageError> {
        Ok(TransformedOutput {
            state: Bounded::from_slice(state, 0)?,
            actions: Bounded::from_slice(actions, Action::ReleaseAll)?,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PipelineState<const M: usize, const S: usize> {
    pub stages: [Bounded<u8, S>; M],
}

impl<const M: usize, const S: usize> Default for PipelineState<M, S> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const M: usize, const S: usize> PipelineState<M, S> {
    pub const fn new() -> Self {
        Self {
            stages: [Bounded::new(0); M],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Input,
    Output,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PipelineError {
    pub stage: usize,
    pub direction: Direction,
    pub cause: StageError,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Transition<const M: usize, const S: usize, const E: usize, const A: usize> {
    pub state: PipelineState<M, S>,
    pub events: Bounded<KeyEvent, E>,
    pub actions: Bounded<Action, A>,
    pub wake_at_ms: Option<u32>,
}

pub struct Pipeline<'a, const M: usize, const S: usize, const E: usize, const A: usize> {
    stages: [&'a dyn Middleware<S, E, A>; M],
}

impl<'a, const M: usize, const S: usize, const E: usize, const A: usize> Pipeline<'a, M, S, E, A> {
    pub const fn new(stages: [&'a dyn Middleware<S, E, A>; M]) -> Self {
        Self { stages }
    }

    pub fn process(
        &self,
        mut state: PipelineState<M, S>,
        mut input: Input<E>,
    ) -> Result<Transition<M, S, E, A>, PipelineError> {
        let mut actions = Bounded::new(Action::ReleaseAll);
        let mut wake_at_ms = None;
        for (index, stage) in self.stages.iter().enumerate() {
            let output = stage
                .process(state.stages[index].as_slice(), &input)
                .map_err(|cause| PipelineError {
                    stage: index,
                    direction: Direction::Input,
                    cause,
                })?;
            actions
                .extend_from_slice(output.actions.as_slice())
                .map_err(|_| PipelineError {
                    stage: index,
                    direction: Direction::Input,
                    cause: StageError::Capacity,
                })?;
            state.stages[index] = output.state;
            input.events = output.events;
            wake_at_ms = earliest_wake(input.now_ms, wake_at_ms, output.wake_at_ms);
        }
        for (index, stage) in self.stages.iter().enumerate().rev() {
            let output = stage
                .transform_output(state.stages[index].as_slice(), actions.as_slice())
                .map_err(|cause| PipelineError {
                    stage: index,
                    direction: Direction::Output,
                    cause,
                })?;
            state.stages[index] = output.state;
            actions = output.actions;
        }
        Ok(Transition {
            state,
            events: input.events,
            actions,
            wake_at_ms,
        })
    }
}

fn earliest_wake(now_ms: u32, first: Option<u32>, second: Option<u32>) -> Option<u32> {
    let relative = |deadline: u32| (deadline.wrapping_sub(now_ms) as i32).max(0) as u32;
    match (first, second) {
        (None, None) => None,
        (Some(deadline), None) | (None, Some(deadline)) => {
            Some(now_ms.wrapping_add(relative(deadline)))
        }
        (Some(first), Some(second)) => {
            Some(now_ms.wrapping_add(relative(first).min(relative(second))))
        }
    }
}
