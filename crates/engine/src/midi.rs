#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MidiAction {
    Parameter(usize, f32),
    Program(usize),
    Tempo(f32),
    Note(u8),
    ClockStart,
    ClockStop,
}
pub const CC_ORDER: [usize; 13] = [3, 10, 4, 5, 6, 2, 1, 18, 9, 7, 8, 11, 19];
/// Original MIDI Note On mapping at 0x60033710 (independent of the preset).
pub fn note_time(note: u8) -> f32 {
    let semitones = (note.clamp(36, 84) - 36) as f32;
    (-semitones).mul_add(0.020833334, 1.0).clamp(0.0, 1.0)
}
#[derive(Default)]
pub struct MidiState {
    last_clock: Option<f64>,
    count: usize,
}
impl MidiState {
    pub fn message(
        &mut self,
        data: &[u8],
        time_ms: f64,
        channel: u8,
        map: &[u8; 13],
    ) -> Option<MidiAction> {
        let &status = data.first()?;
        if status == 0xfa {
            self.last_clock = Some(time_ms.floor());
            self.count = 0;
            return Some(MidiAction::ClockStart);
        }
        if status == 0xfc {
            return Some(MidiAction::ClockStop);
        }
        if status == 0xf8 {
            // 0x60033744 measures complete 24-pulse quarters in integer ms.
            self.count += 1;
            if self.count < 24 {
                return None;
            }
            self.count = 0;
            let now = time_ms.floor();
            let elapsed = now - self.last_clock.replace(now).unwrap_or(0.0);
            if elapsed > 0.0 {
                return Some(MidiAction::Tempo(60.0 / (elapsed as f32 * 0.001)));
            }
            return None;
        }
        if channel != 0 && (status & 15) + 1 != channel {
            return None;
        }
        match (status & 0xf0, data) {
            (0xb0, [_, cc, value, ..]) => map.iter().position(|c| c == cc).map(|i| {
                MidiAction::Parameter(
                    CC_ORDER[i],
                    if i == 12 {
                        if *value >= 64 {
                            1.0
                        } else {
                            0.0
                        }
                    } else {
                        *value as f32 * 0.007874016
                    },
                )
            }),
            (0xc0, [_, program, ..]) => Some(MidiAction::Program(*program as usize)),
            (0x90, [_, note, velocity, ..]) if *velocity > 0 => Some(MidiAction::Note(*note)),
            _ => None,
        }
    }
}
#[derive(Default)]
pub struct TapTempo {
    last: Option<f64>,
    intervals: Vec<f64>,
}
impl TapTempo {
    pub fn tap(&mut self, time_ms: f64) -> Option<f32> {
        let last = self.last.replace(time_ms)?;
        let dt = time_ms - last;
        if !(150.0..=3000.0).contains(&dt) {
            self.intervals.clear();
            return None;
        }
        if self.intervals.len() == 4 {
            self.intervals.remove(0);
        }
        self.intervals.push(dt);
        Some((60000.0 / (self.intervals.iter().sum::<f64>() / self.intervals.len() as f64)) as f32)
    }
}
pub const SYNC_LABELS: [&str; 17] = [
    "1/32", "1/16T", "1/16", "1/8T", "1/16D", "1/8", "1/4T", "1/8D", "1/4", "1/2T", "1/4D", "1/2",
    "1/1T", "1/2D", "1/1", "1/1D", "2/1",
];

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clock_120_bpm_and_channel_filter() {
        let mut state = MidiState::default();
        let map = std::array::from_fn(|i| 70 + i as u8);
        assert_eq!(state.message(&[0xb1, 70, 127], 0.0, 1, &map), None);
        assert_eq!(
            state.message(&[0xb0, 70, 127], 0.0, 1, &map),
            Some(MidiAction::Parameter(3, 1.0))
        );
        let mut last = None;
        for i in 0..49 {
            if let Some(action) = state.message(&[0xf8], i as f64 * (500.0 / 24.0), 1, &map) {
                last = Some(action);
            }
        }
        assert!(matches!(last,Some(MidiAction::Tempo(v)) if (v-120.0).abs()<0.01));
    }
    #[test]
    fn notes_follow_firmware_range() {
        for (n, want) in [
            (0, 1.),
            (36, 1.),
            (48, 0.75),
            (60, 0.5),
            (72, 0.25),
            (84, 0.),
            (127, 0.),
        ] {
            assert!((note_time(n) - want).abs() < 1e-7);
        }
    }
}
