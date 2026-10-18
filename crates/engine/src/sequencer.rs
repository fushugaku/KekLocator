//! Beat-addressed source. DAW PPQ is the clock; there is no looping audio file.
use serde::{Deserialize, Serialize};

pub const DIVISIONS: [f64; 11] = [
    0.0625,
    0.125,
    1. / 6.,
    0.25,
    1. / 3.,
    0.5,
    2. / 3.,
    1.,
    1.5,
    2.,
    4.,
];
pub const LABELS: [&str; 11] = [
    "1/64", "1/32", "1/16T", "1/16", "1/8T", "1/8", "1/4T", "1/4", "1/4D", "1/2", "1/1",
];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Sequence {
    pub enabled: bool,
    pub host_sync: bool,
    pub bpm: f64,
    pub division: usize,
    pub level: f32,
    pub transpose: i32,
    pub length: usize,
    pub notes: [u8; 16],
    pub velocities: [f32; 16],
}
impl Default for Sequence {
    fn default() -> Self {
        Self {
            enabled: false,
            host_sync: true,
            bpm: 120.,
            division: 7,
            level: 0.8,
            transpose: 0,
            length: 16,
            notes: [
                45, 52, 57, 49, 45, 54, 52, 57, 45, 52, 57, 49, 45, 54, 52, 57,
            ],
            velocities: [
                1., 0.7, 0.85, 0.6, 1., 0.75, 0.85, 0.65, 1., 0.7, 0.85, 0.6, 1., 0.75, 0.85, 0.65,
            ],
        }
    }
}
impl Sequence {
    pub fn step(&self, beat: f64, playing: bool) -> Option<usize> {
        if !self.enabled || !playing || !beat.is_finite() {
            return None;
        }
        let step = ((beat / DIVISIONS[self.division.min(10)] + 1e-9).floor() as i64)
            .rem_euclid(self.length.clamp(1, 16) as i64);
        Some(step as usize)
    }
    pub fn sample(&self, beat: f64, bpm: f64, playing: bool) -> [f32; 2] {
        let Some(step) = self.step(beat, playing) else {
            return [0.; 2];
        };
        let velocity = self.velocities[step].clamp(0., 1.);
        if velocity == 0. || self.level == 0. {
            return [0.; 2];
        }
        let division = DIVISIONS[self.division.min(10)];
        let phase = (beat / division + 1e-9).fract();
        let phase = if phase < 0. { phase + 1. } else { phase };
        let local = phase * division * 60. / bpm;
        let note = (self.notes[step] as i32 + self.transpose).clamp(0, 127);
        let frequency = 440. * 2f64.powf((note as f64 - 69.) / 12.);
        let angle = local * frequency * std::f64::consts::TAU;
        let pluck = (angle.sin() + 0.3 * (angle * 2.).sin()) * (-local * 7.).exp() * 0.16;
        let tick =
            ((local * 1371.35).sin() + (local * 7813.91).sin()) * (-local * 140.).exp() * 0.008;
        let attack = (local / 0.002).clamp(0., 1.);
        let release = ((1. - phase) * division * 60. / bpm / 0.004).clamp(0., 1.);
        let gain = velocity as f64 * self.level as f64 * attack * release;
        [
            ((pluck + tick) * gain) as f32,
            ((pluck * 0.93 - tick) * gain) as f32,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn musical_grid_supports_tempo_seek_stop_and_negative_preroll() {
        let mut s = Sequence {
            enabled: true,
            division: 3,
            ..Sequence::default()
        };
        for (beat, want) in [(0., 0), (0.25, 1), (1.25, 5), (4., 0), (-0.25, 15)] {
            assert_eq!(s.step(beat, true), Some(want));
        }
        assert_eq!(s.step(1., false), None);
        s.length = 5;
        assert_eq!(s.step(1.25, true), Some(0));
        s.velocities[0] = 0.;
        assert_eq!(s.sample(0.1, 120., true), [0.; 2]);
        s.velocities[0] = 1.;
        assert_ne!(s.sample(0.1, 120., true), [0.; 2]);
    }
}
