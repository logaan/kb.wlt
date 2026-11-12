#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SessionGate {
    ready: bool,
    generation: u32,
}

impl SessionGate {
    pub const fn new() -> Self {
        Self {
            ready: false,
            generation: 0,
        }
    }

    pub fn set_ready(self, ready: bool) -> Self {
        Self {
            ready,
            generation: self.generation.wrapping_add(u32::from(ready != self.ready)),
        }
    }

    pub fn reset(self) -> Self {
        Self {
            ready: false,
            generation: self.generation.wrapping_add(1),
        }
    }

    pub const fn ready(self) -> bool {
        self.ready
    }

    pub const fn generation(self) -> u32 {
        self.generation
    }

    pub fn token(self) -> Option<u32> {
        self.ready.then_some(self.generation)
    }

    pub fn accepts(self, token: u32) -> bool {
        self.token() == Some(token)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InputGate {
    generation: Option<u32>,
    ready: bool,
    armed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InputGateUpdate {
    pub cancel: bool,
    pub keys: Option<u64>,
}

impl InputGate {
    pub const fn new() -> Self {
        Self {
            generation: None,
            ready: false,
            armed: false,
        }
    }

    pub fn observe(
        &mut self,
        generation: u32,
        ready: bool,
        keys: u64,
        invalidated: bool,
    ) -> InputGateUpdate {
        let cancel = invalidated || self.generation != Some(generation) || self.ready != ready;
        self.generation = Some(generation);
        self.ready = ready;
        if cancel || !ready {
            self.armed = false;
        }
        if ready && keys == 0 {
            self.armed = true;
        }
        InputGateUpdate {
            cancel,
            keys: (!cancel && ready && self.armed).then_some(keys),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{InputGate, SessionGate};

    #[test]
    fn disconnected_output_never_becomes_valid_after_reconnect() {
        let connected = SessionGate::new().set_ready(true);
        let queued = connected.token().unwrap();
        let disconnected = connected.set_ready(false);
        assert!(!disconnected.accepts(queued));
        let reconnected = disconnected.set_ready(true);
        assert!(!reconnected.accepts(queued));
        assert!(reconnected.accepts(reconnected.token().unwrap()));
    }

    #[test]
    fn repeated_ready_observations_preserve_pending_output() {
        let connected = SessionGate::new().set_ready(true);
        let queued = connected.token().unwrap();
        assert!(connected.set_ready(true).accepts(queued));
    }

    #[test]
    fn reset_is_visible_even_without_observing_intermediate_disconnect() {
        let connected = SessionGate::new().set_ready(true);
        let queued = connected.token().unwrap();
        let reconfigured = connected.reset().set_ready(true);
        assert!(!reconfigured.accepts(queued));
        assert_ne!(connected.generation(), reconfigured.generation());
    }

    #[test]
    fn generation_wrap_does_not_revalidate_previous_session() {
        let connected = SessionGate {
            ready: true,
            generation: u32::MAX,
        };
        let reconnected = connected.reset().set_ready(true);
        assert!(!reconnected.accepts(u32::MAX));
        assert!(reconnected.accepts(1));
    }

    #[test]
    fn held_startup_keys_do_not_generate_unmatched_releases() {
        let mut gate = InputGate::new();
        assert!(gate.observe(1, true, 0b11, false).cancel);
        assert_eq!(gate.observe(1, true, 0b01, false).keys, None);
        assert_eq!(gate.observe(1, true, 0, false).keys, Some(0));
        assert_eq!(gate.observe(1, true, 0b10, false).keys, Some(0b10));
    }

    #[test]
    fn lost_half_cancels_holds_instead_of_synthesizing_taps() {
        let mut gate = InputGate::new();
        gate.observe(1, true, 0, false);
        assert_eq!(gate.observe(1, true, 0b11, false).keys, Some(0b11));
        let lost = gate.observe(1, true, 0b01, true);
        assert!(lost.cancel);
        assert_eq!(lost.keys, None);
        assert_eq!(gate.observe(1, true, 0b01, false).keys, None);
        assert_eq!(gate.observe(1, true, 0, false).keys, Some(0));
        assert_eq!(gate.observe(1, true, 0b01, false).keys, Some(0b01));
    }

    #[test]
    fn host_resume_waits_for_neutral_even_if_generation_was_unchanged() {
        let mut gate = InputGate::new();
        gate.observe(1, true, 0, false);
        gate.observe(1, true, 0b10, false);
        assert!(gate.observe(1, false, 0b10, false).cancel);
        assert!(gate.observe(1, true, 0b10, false).cancel);
        assert_eq!(gate.observe(1, true, 0b11, false).keys, None);
        assert_eq!(gate.observe(1, true, 0, false).keys, Some(0));
    }

    #[test]
    fn unseen_disconnect_is_caught_by_session_generation() {
        let mut gate = InputGate::new();
        gate.observe(u32::MAX, true, 0, false);
        gate.observe(u32::MAX, true, 1, false);
        let reconnected = gate.observe(0, true, 1, false);
        assert!(reconnected.cancel);
        assert_eq!(reconnected.keys, None);
        assert!(!gate.observe(0, true, 1, false).cancel);
        assert_eq!(gate.observe(0, true, 1, false).keys, None);
    }
}
