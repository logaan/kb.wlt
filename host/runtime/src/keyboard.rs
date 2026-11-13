use crate::{CALL_FUEL, Error, Runtime};
use core::cell::RefCell;
use kb_core::pipeline::{
    Action, Bounded, Input, Key, KeyEvent, KeyState, Middleware, Output, Pipeline, PipelineState,
    StageError, TransformedOutput, Transition,
};
use wasmi::TypedFunc;

type Process = TypedFunc<(i32, i32, i32, i32, i32), i32>;
type Transform = TypedFunc<(i32, i32, i32, i32), i32>;
type Realloc = TypedFunc<(i32, i32, i32, i32), i32>;

struct Instance {
    runtime: Runtime,
    process: Process,
    transform: Transform,
    post_process: TypedFunc<i32, ()>,
    post_transform: TypedFunc<i32, ()>,
    realloc: Realloc,
    poisoned: bool,
}

pub struct KeyboardComponent {
    instance: RefCell<Instance>,
}

impl KeyboardComponent {
    pub fn new(bytes: &[u8]) -> Result<Self, Error> {
        Self::with_engine(bytes, &Runtime::engine(false))
    }

    fn with_engine(bytes: &[u8], engine: &wasmi::Engine) -> Result<Self, Error> {
        let runtime = Runtime::with_engine(&[bytes], engine)?;
        let instance = runtime.instance;
        let store = &runtime.store;
        let process = instance.get_typed_func(store, "kb:firmware/keyboard@0.1.0#process")?;
        let transform =
            instance.get_typed_func(store, "kb:firmware/keyboard@0.1.0#transform-output")?;
        let post_process =
            instance.get_typed_func(store, "cabi_post_kb:firmware/keyboard@0.1.0#process")?;
        let post_transform = instance.get_typed_func(
            store,
            "cabi_post_kb:firmware/keyboard@0.1.0#transform-output",
        )?;
        let realloc = instance.get_typed_func(store, "cabi_realloc")?;
        Ok(Self {
            instance: RefCell::new(Instance {
                runtime,
                process,
                transform,
                post_process,
                post_transform,
                realloc,
                poisoned: false,
            }),
        })
    }

    pub fn process<const S: usize, const E: usize, const A: usize>(
        &self,
        state: &[u8],
        input: &Input<E>,
    ) -> Result<Output<S, E, A>, Error> {
        if state.len() > S {
            return Err(Error::Capacity);
        }
        let mut instance = self
            .instance
            .try_borrow_mut()
            .map_err(|_| Error::InvalidState)?;
        instance.begin()?;
        let result = instance.process::<S, E, A>(state, input);
        if matches!(result, Err(Error::Wasm(_))) {
            instance.poisoned = true;
        }
        result
    }

    pub fn transform_output<const S: usize, const A: usize>(
        &self,
        state: &[u8],
        actions: &[Action],
    ) -> Result<TransformedOutput<S, A>, Error> {
        if state.len() > S || actions.len() > A {
            return Err(Error::Capacity);
        }
        let mut instance = self
            .instance
            .try_borrow_mut()
            .map_err(|_| Error::InvalidState)?;
        instance.begin()?;
        let result = instance.transform::<S, A>(state, actions);
        if matches!(result, Err(Error::Wasm(_))) {
            instance.poisoned = true;
        }
        result
    }
}

pub struct KeyboardPipeline<const M: usize> {
    stages: [KeyboardComponent; M],
}

impl<const M: usize> KeyboardPipeline<M> {
    pub fn new(modules: &[&[u8]; M]) -> Result<Self, Error> {
        if M == 0 || M > 2 {
            return Err(Error::InvalidChain);
        }
        let engine = Runtime::engine(M > 1);
        let mut stages: [Option<KeyboardComponent>; M] = core::array::from_fn(|_| None);
        for (stage, module) in stages.iter_mut().zip(modules) {
            *stage = Some(KeyboardComponent::with_engine(module, &engine)?);
        }
        Ok(Self {
            stages: stages.map(|stage| stage.unwrap()),
        })
    }

    pub fn process<const S: usize, const E: usize, const A: usize>(
        &self,
        state: PipelineState<M, S>,
        input: Input<E>,
    ) -> Result<Transition<M, S, E, A>, Error> {
        let stages: [&dyn Middleware<S, E, A>; M] =
            core::array::from_fn(|index| &self.stages[index] as &dyn Middleware<S, E, A>);
        Pipeline::new(stages)
            .process(state, input)
            .map_err(Error::Pipeline)
    }
}

impl<const S: usize, const E: usize, const A: usize> Middleware<S, E, A> for KeyboardComponent {
    fn process(&self, state: &[u8], input: &Input<E>) -> Result<Output<S, E, A>, StageError> {
        self.process(state, input).map_err(stage_error)
    }

    fn transform_output(
        &self,
        state: &[u8],
        actions: &[Action],
    ) -> Result<TransformedOutput<S, A>, StageError> {
        self.transform_output(state, actions).map_err(stage_error)
    }
}

fn stage_error(error: Error) -> StageError {
    match error {
        Error::Capacity => StageError::Capacity,
        Error::InvalidState | Error::InvalidReturn => StageError::InvalidState,
        _ => StageError::Trap(1),
    }
}

impl Instance {
    fn begin(&mut self) -> Result<(), Error> {
        if self.poisoned {
            return Err(Error::InvalidState);
        }
        self.runtime.store.set_fuel(CALL_FUEL * 10)?;
        Ok(())
    }

    fn allocate(&mut self, bytes: usize, alignment: i32) -> Result<i32, Error> {
        if bytes == 0 {
            return Ok(0);
        }
        let bytes = i32::try_from(bytes).map_err(|_| Error::Capacity)?;
        let pointer = self
            .realloc
            .call(&mut self.runtime.store, (0, 0, alignment, bytes))?;
        if pointer < 0 {
            return Err(Error::InvalidReturn);
        }
        Ok(pointer)
    }

    fn write(&mut self, pointer: i32, offset: usize, bytes: &[u8]) -> Result<(), Error> {
        let offset = (pointer as usize)
            .checked_add(offset)
            .ok_or(Error::InvalidReturn)?;
        self.runtime
            .memory
            .write(&mut self.runtime.store, offset, bytes)
            .map_err(|_| Error::InvalidReturn)
    }

    fn write_state(&mut self, state: &[u8]) -> Result<i32, Error> {
        let pointer = self.allocate(state.len(), 1)?;
        self.write(pointer, 0, state)?;
        Ok(pointer)
    }

    fn write_events(&mut self, events: &[KeyEvent]) -> Result<i32, Error> {
        let pointer = self.allocate(events.len().checked_mul(8).ok_or(Error::Capacity)?, 4)?;
        for (index, event) in events.iter().enumerate() {
            let mut bytes = [0; 8];
            bytes[..2].copy_from_slice(&event.position.to_le_bytes());
            bytes[2] = match event.state {
                KeyState::Pressed => 0,
                KeyState::Released => 1,
            };
            bytes[4..8].copy_from_slice(&event.timestamp_ms.to_le_bytes());
            self.write(pointer, index * 8, &bytes)?;
        }
        Ok(pointer)
    }

    fn write_actions(&mut self, actions: &[Action]) -> Result<i32, Error> {
        let pointer = self.allocate(actions.len().checked_mul(6).ok_or(Error::Capacity)?, 2)?;
        for (index, action) in actions.iter().enumerate() {
            let mut bytes = [0; 6];
            let (tag, key) = match action {
                Action::Press(key) => (0, Some(key)),
                Action::Release(key) => (1, Some(key)),
                Action::Tap(key) => (2, Some(key)),
                Action::ReleaseAll => (3, None),
            };
            bytes[0] = tag;
            if let Some(key) = key {
                bytes[2..4].copy_from_slice(&key.usage.to_le_bytes());
                bytes[4] = key.modifiers;
            }
            self.write(pointer, index * 6, &bytes)?;
        }
        Ok(pointer)
    }

    fn process<const S: usize, const E: usize, const A: usize>(
        &mut self,
        state: &[u8],
        input: &Input<E>,
    ) -> Result<Output<S, E, A>, Error> {
        let state_pointer = self.write_state(state)?;
        let events_pointer = self.write_events(input.events.as_slice())?;
        let pointer = self.process.call(
            &mut self.runtime.store,
            (
                state_pointer,
                state.len() as i32,
                events_pointer,
                input.events.len() as i32,
                input.now_ms as i32,
            ),
        )?;
        let result = self.read_output(pointer);
        self.post_process.call(&mut self.runtime.store, pointer)?;
        result
    }

    fn transform<const S: usize, const A: usize>(
        &mut self,
        state: &[u8],
        actions: &[Action],
    ) -> Result<TransformedOutput<S, A>, Error> {
        let state_pointer = self.write_state(state)?;
        let actions_pointer = self.write_actions(actions)?;
        let pointer = self.transform.call(
            &mut self.runtime.store,
            (
                state_pointer,
                state.len() as i32,
                actions_pointer,
                actions.len() as i32,
            ),
        )?;
        let result = self.read_transform(pointer);
        self.post_transform.call(&mut self.runtime.store, pointer)?;
        result
    }

    fn read_output<const S: usize, const E: usize, const A: usize>(
        &self,
        pointer: i32,
    ) -> Result<Output<S, E, A>, Error> {
        let bytes = self.runtime.read::<32>(pointer)?;
        let wake_at_ms = match bytes[24] {
            0 => None,
            1 => Some(word(&bytes, 28)),
            _ => return Err(Error::InvalidReturn),
        };
        Ok(Output {
            state: self.read_state(word(&bytes, 0), word(&bytes, 4))?,
            events: self.read_events(word(&bytes, 8), word(&bytes, 12))?,
            actions: self.read_actions(word(&bytes, 16), word(&bytes, 20))?,
            wake_at_ms,
        })
    }

    fn read_transform<const S: usize, const A: usize>(
        &self,
        pointer: i32,
    ) -> Result<TransformedOutput<S, A>, Error> {
        let bytes = self.runtime.read::<16>(pointer)?;
        Ok(TransformedOutput {
            state: self.read_state(word(&bytes, 0), word(&bytes, 4))?,
            actions: self.read_actions(word(&bytes, 8), word(&bytes, 12))?,
        })
    }

    fn read_state<const S: usize>(
        &self,
        pointer: u32,
        length: u32,
    ) -> Result<Bounded<u8, S>, Error> {
        if length as usize > S {
            return Err(Error::Capacity);
        }
        let mut state = Bounded::new(0);
        for index in 0..length {
            let byte = self.read_item::<1>(pointer, index)?;
            state.push(byte[0]).map_err(|_| Error::Capacity)?;
        }
        Ok(state)
    }

    fn read_events<const E: usize>(
        &self,
        pointer: u32,
        length: u32,
    ) -> Result<Bounded<KeyEvent, E>, Error> {
        if length as usize > E {
            return Err(Error::Capacity);
        }
        let mut events = Bounded::new(KeyEvent::EMPTY);
        for index in 0..length {
            let bytes = self.read_item::<8>(pointer, index)?;
            let state = match bytes[2] {
                0 => KeyState::Pressed,
                1 => KeyState::Released,
                _ => return Err(Error::InvalidReturn),
            };
            events
                .push(KeyEvent {
                    position: u16::from_le_bytes([bytes[0], bytes[1]]),
                    state,
                    timestamp_ms: word(&bytes, 4),
                })
                .map_err(|_| Error::Capacity)?;
        }
        Ok(events)
    }

    fn read_actions<const A: usize>(
        &self,
        pointer: u32,
        length: u32,
    ) -> Result<Bounded<Action, A>, Error> {
        if length as usize > A {
            return Err(Error::Capacity);
        }
        let mut actions = Bounded::new(Action::ReleaseAll);
        for index in 0..length {
            let bytes = self.read_item::<6>(pointer, index)?;
            let key = Key {
                usage: u16::from_le_bytes([bytes[2], bytes[3]]),
                modifiers: bytes[4],
            };
            let action = match bytes[0] {
                0 => Action::Press(key),
                1 => Action::Release(key),
                2 => Action::Tap(key),
                3 => Action::ReleaseAll,
                _ => return Err(Error::InvalidReturn),
            };
            actions.push(action).map_err(|_| Error::Capacity)?;
        }
        Ok(actions)
    }

    fn read_item<const N: usize>(&self, pointer: u32, index: u32) -> Result<[u8; N], Error> {
        let offset = index
            .checked_mul(N as u32)
            .and_then(|offset| pointer.checked_add(offset))
            .ok_or(Error::InvalidReturn)?;
        self.runtime.read(offset as i32)
    }
}

fn word(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}
