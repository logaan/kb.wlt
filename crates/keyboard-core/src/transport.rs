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

#[cfg(test)]
mod tests {
    use super::SessionGate;

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
}
