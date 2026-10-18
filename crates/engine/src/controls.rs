//! Browser pedal gestures use the same single/double/2-second hold actions as the unit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PedalEvent {
    Single,
    Double,
    HoldStart,
    HoldEnd,
    Tap,
}

#[derive(Default)]
pub struct Pedal {
    down: Option<f64>,
    deadline: Option<f64>,
    second: bool,
    held: bool,
    tap_mode: bool,
}
impl Pedal {
    pub fn press(&mut self, now: f64, tap_mode: bool) -> Option<PedalEvent> {
        if self.down.is_some() {
            return None;
        }
        self.second = self.deadline.take().is_some_and(|deadline| now <= deadline);
        self.down = Some(now);
        self.held = false;
        self.tap_mode = tap_mode;
        if tap_mode {
            Some(PedalEvent::Tap)
        } else {
            None
        }
    }
    pub fn release(&mut self, now: f64) -> Option<PedalEvent> {
        self.down.take()?;
        if self.tap_mode {
            return None;
        }
        if self.held {
            self.held = false;
            return Some(PedalEvent::HoldEnd);
        }
        if self.second {
            self.second = false;
            return Some(PedalEvent::Double);
        }
        self.deadline = Some(now + 300.0);
        None
    }
    pub fn tick(&mut self, now: f64) -> Option<PedalEvent> {
        if self.down.is_some_and(|start| now - start >= 2000.0) && !self.held && !self.tap_mode {
            self.held = true;
            self.deadline = None;
            return Some(PedalEvent::HoldStart);
        }
        if self.down.is_none() && self.deadline.is_some_and(|deadline| now >= deadline) {
            self.deadline = None;
            return Some(PedalEvent::Single);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn single_double_hold_and_tap_are_distinct() {
        let mut p = Pedal::default();
        assert_eq!(p.press(0.0, false), None);
        assert_eq!(p.release(50.0), None);
        assert_eq!(p.tick(349.0), None);
        assert_eq!(p.tick(350.0), Some(PedalEvent::Single));
        p.press(1000.0, false);
        p.release(1050.0);
        p.press(1200.0, false);
        assert_eq!(p.release(1250.0), Some(PedalEvent::Double));
        assert_eq!(p.tick(1600.0), None);
        p.press(2000.0, false);
        assert_eq!(p.tick(3999.0), None);
        assert_eq!(p.tick(4000.0), Some(PedalEvent::HoldStart));
        assert_eq!(p.release(4100.0), Some(PedalEvent::HoldEnd));
        assert_eq!(p.tick(5000.0), None);
        assert_eq!(p.press(6000.0, true), Some(PedalEvent::Tap));
        assert_eq!(p.tick(9000.0), None);
        assert_eq!(p.release(9050.0), None);
    }
}
