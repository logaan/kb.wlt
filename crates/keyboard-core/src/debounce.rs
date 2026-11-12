#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DebounceUpdate {
    pub stable: u64,
    pub pressed: u64,
    pub released: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Debouncer<const N: usize> {
    stable: u64,
    candidate: u64,
    changed_at: [u32; N],
    debounce_ms: u32,
}

impl<const N: usize> Debouncer<N> {
    pub const fn new(debounce_ms: u32) -> Self {
        assert!(N <= 64);
        assert!(debounce_ms < 0x8000_0000);
        Self {
            stable: 0,
            candidate: 0,
            changed_at: [0; N],
            debounce_ms,
        }
    }

    pub const fn stable(&self) -> u64 {
        self.stable
    }

    pub fn sample(mut self, raw_mask: u64, now_ms: u32) -> (Self, DebounceUpdate) {
        let previous = self.stable;
        for (index, changed_at) in self.changed_at.iter_mut().enumerate() {
            let bit = 1_u64 << index;
            if (raw_mask ^ self.candidate) & bit != 0 {
                self.candidate ^= bit;
                *changed_at = now_ms;
            }
            if (self.candidate ^ self.stable) & bit != 0
                && now_ms.wrapping_sub(*changed_at) >= self.debounce_ms
            {
                self.stable ^= bit;
            }
        }
        let update = DebounceUpdate {
            stable: self.stable,
            pressed: self.stable & !previous,
            released: previous & !self.stable,
        };
        (self, update)
    }
}
