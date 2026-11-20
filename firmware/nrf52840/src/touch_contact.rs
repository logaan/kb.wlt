use kb_client_session::Navigation;

const HOLD_MS: u32 = 550;
const SWIPE_DISTANCE: u16 = 22;

#[derive(Default)]
pub(super) struct Contact {
    origin: Option<(u32, [i16; 2], bool)>,
    travel: [i16; 2],
    acted: bool,
    cancelled: bool,
}

impl Contact {
    pub(super) fn active(&self) -> bool {
        self.origin.is_some()
    }

    pub(super) fn sample(
        &mut self,
        now: u32,
        point: Option<[i16; 2]>,
        destination: bool,
    ) -> Option<Navigation> {
        let Some((start, [x, y], origin)) = self.origin else {
            self.origin = point.map(|point| (now, point, destination));
            return None;
        };
        self.cancelled |= destination != origin;
        if let Some([px, py]) = point {
            let travel = [py - y, x - px];
            if distance(travel) > distance(self.travel) {
                self.travel = travel;
            }
            if !self.acted
                && !self.cancelled
                && self.direction().is_none()
                && now.wrapping_sub(start) >= HOLD_MS
            {
                self.acted = true;
                return Some(Navigation::Escape);
            }
            return None;
        }
        let navigation = (!self.acted && !self.cancelled).then(|| {
            self.direction().unwrap_or_else(|| {
                if now.wrapping_sub(start) >= HOLD_MS {
                    Navigation::Escape
                } else {
                    Navigation::Enter
                }
            })
        });
        *self = Self::default();
        navigation
    }

    fn direction(&self) -> Option<Navigation> {
        let [x, y] = self.travel;
        if distance(self.travel) < SWIPE_DISTANCE {
            None
        } else if x.unsigned_abs() > y.unsigned_abs() {
            Some(if x > 0 {
                Navigation::Right
            } else {
                Navigation::Left
            })
        } else {
            Some(if y > 0 {
                Navigation::Down
            } else {
                Navigation::Up
            })
        }
    }
}

fn distance([x, y]: [i16; 2]) -> u16 {
    x.unsigned_abs().max(y.unsigned_abs())
}
