//! Shared native/WASM DAW adapter. All storage is prepared outside process().
use crate::machine::Result;
use crate::{parameters::PARAMETERS, presets::PresetBank, sequencer::Sequence, Engine};
use std::f64::consts::PI;

use crate::extended as x;
pub use crate::extended::PARAM_COUNT;
pub const MORPH: usize = 63;
pub const BYPASS: usize = 20;
pub const DELAY_SYNC: usize = 21;
pub const INPUT_GAIN: usize = 22;
pub const OUTPUT_GAIN: usize = 23;
pub const SEQ_ENABLE: usize = 24;
pub const SEQ_HOST: usize = 25;
pub const TEMPO: usize = 26;
pub const SEQ_RATE: usize = 27;
pub const SEQ_LEVEL: usize = 28;
pub const TRANSPOSE: usize = 29;
pub const LENGTH: usize = 30;
pub const NOTES: usize = 31;
pub const VELOCITIES: usize = 47;
pub const HOST_NAMES: [&str; 11] = [
    "Bypass",
    "Delay sync",
    "Input gain",
    "Output level",
    "Sequencer",
    "Host sync",
    "Tempo",
    "Step division",
    "Sequence level",
    "Transpose",
    "Sequence length",
];

pub fn defaults() -> [f32; PARAM_COUNT] {
    let mut p = [0.; PARAM_COUNT];
    let bank = PresetBank::factory();
    p[..20].copy_from_slice(
        &bank
            .presets
            .iter()
            .find(|p| p.name == "GREAT EYE")
            .unwrap()
            .values,
    );
    let s = Sequence::default();
    p[DELAY_SYNC] = 1.;
    p[INPUT_GAIN] = 0.8;
    p[OUTPUT_GAIN] = 1.;
    p[SEQ_HOST] = 1.;
    p[TEMPO] = (120. - 20.) / 380.;
    p[SEQ_RATE] = s.division as f32 / 10.;
    p[SEQ_LEVEL] = s.level;
    p[TRANSPOSE] = 0.5;
    p[LENGTH] = 1.;
    for i in 0..16 {
        p[NOTES + i] = s.notes[i] as f32 / 127.;
        p[VELOCITIES + i] = s.velocities[i];
    }
    x::initialize(&mut p);
    p
}
pub fn sequence(p: &[f32; PARAM_COUNT]) -> Sequence {
    Sequence {
        enabled: p[SEQ_ENABLE] >= 0.5,
        host_sync: p[SEQ_HOST] >= 0.5,
        bpm: 20. + 380. * p[TEMPO] as f64,
        division: (p[SEQ_RATE] * 10.).round() as usize,
        level: p[SEQ_LEVEL],
        transpose: (p[TRANSPOSE] * 48. - 24.).round() as i32,
        length: (p[LENGTH] * 15.).round() as usize + 1,
        notes: std::array::from_fn(|i| (p[NOTES + i] * 127.).round() as u8),
        velocities: std::array::from_fn(|i| p[VELOCITIES + i]),
    }
}
pub fn name(id: usize) -> String {
    match id {
        0..=19 => PARAMETERS[id].label.into(),
        20..=30 => HOST_NAMES[id - 20].into(),
        31..=46 => format!("Step {} note", id - 30),
        47..=62 => format!("Step {} velocity", id - 46),
        63 => "Morph time".into(),
        _ => x::name(id),
    }
}

const TAPS: usize = 64;
const HALF: usize = TAPS / 2;
const PHASES: usize = 256;
const HISTORY: usize = 256;
fn padding(rate: f64) -> usize {
    if rate == 48000. {
        0
    } else {
        ((rate / 48000.).ceil() as usize + 2).max(4)
    }
}
struct Resampler {
    history: [[f32; 2]; HISTORY],
    count: i64,
    next: f64,
    step: f64,
    coefficients: Vec<[f32; TAPS]>,
}
impl Resampler {
    fn new(input: f64, output: f64) -> Self {
        let cutoff = (output / input).min(1.) * 0.94;
        let mut coefficients = vec![[0.; TAPS]; PHASES + 1];
        for (phase, row) in coefficients.iter_mut().enumerate() {
            let fraction = phase as f64 / PHASES as f64;
            let mut sum = 0.;
            for (tap, c) in row.iter_mut().enumerate() {
                let x = tap as f64 - (HALF - 1) as f64 - fraction;
                let w = if x.abs() <= HALF as f64 {
                    0.35875
                        + 0.48829 * (PI * x / HALF as f64).cos()
                        + 0.14128 * (2. * PI * x / HALF as f64).cos()
                        + 0.01168 * (3. * PI * x / HALF as f64).cos()
                } else {
                    0.
                };
                let sinc = if x.abs() < 1e-12 {
                    cutoff
                } else {
                    (PI * x * cutoff).sin() / (PI * x)
                };
                *c = (w * sinc) as f32;
                sum += *c as f64;
            }
            for c in row {
                *c = (*c as f64 / sum) as f32;
            }
        }
        Self {
            history: [[0.; 2]; HISTORY],
            count: 0,
            next: -(HALF as f64),
            step: input / output,
            coefficients,
        }
    }
    fn push(&mut self, s: [f32; 2]) {
        self.history[self.count as usize % HISTORY] = s;
        self.count += 1;
    }
    fn pop(&mut self) -> Option<[f32; 2]> {
        if self.next + HALF as f64 > (self.count - 1) as f64 + 1e-9 {
            return None;
        }
        let center = self.next.floor() as i64;
        let phase = (self.next - center as f64) * PHASES as f64;
        let p = (phase.floor() as usize).min(PHASES - 1);
        let f = phase - p as f64;
        let mut sum = [0.; 2];
        for tap in 0..TAPS {
            let i = center + tap as i64 - (HALF - 1) as i64;
            if i < 0 || i >= self.count || self.count - i > HISTORY as i64 {
                continue;
            }
            let c = self.coefficients[p][tap] as f64
                + (self.coefficients[p + 1][tap] as f64 - self.coefficients[p][tap] as f64) * f;
            let s = self.history[i as usize % HISTORY];
            sum[0] += s[0] as f64 * c;
            sum[1] += s[1] as f64 * c;
        }
        self.next += self.step;
        Some([sum[0] as f32, sum[1] as f32])
    }
}
pub struct DawProcessor {
    pub engine: Engine,
    bank: PresetBank,
    pub parameters: [f32; PARAM_COUNT],
    sample_rate: f64,
    input: Resampler,
    output: Resampler,
    queue: [[f32; 2]; 64],
    read: usize,
    len: usize,
    free_samples: u64,
    free_beat: f64,
    pub step: i32,
    pub tempo: f64,
    duck_envelope: f32,
    was_extended: bool,
    lock_active_mask: u32,
    pub duck_reduction: f32,
}
impl DawProcessor {
    pub fn new(sample_rate: f64) -> Result<Self> {
        let mut engine = Engine::new()?;
        if !sample_rate.is_finite() || !(8000. ..=384000.).contains(&sample_rate) {
            return Err(engine.machine.fault("unsupported sample rate"));
        }
        let parameters = defaults();
        engine.apply_preset(parameters[..20].try_into().unwrap(), 0.)?;
        let mut s = Self {
            engine,
            bank: PresetBank::factory(),
            parameters,
            sample_rate,
            input: Resampler::new(sample_rate, 48000.),
            output: Resampler::new(48000., sample_rate),
            queue: [[0.; 2]; 64],
            read: 0,
            len: padding(sample_rate),
            free_samples: 0,
            free_beat: 0.,
            step: -1,
            tempo: 120.,
            duck_envelope: 0.,
            was_extended: false,
            lock_active_mask: 0,
            duck_reduction: 0.,
        };
        s.sync_settings(120.)?;
        Ok(s)
    }
    pub fn latency(&self) -> u32 {
        if self.sample_rate == 48000. {
            23
        } else {
            (padding(self.sample_rate) as f64
                + HALF as f64
                + (HALF + 23) as f64 * self.sample_rate / 48000.)
                .round() as u32
        }
    }
    pub fn set_parameter(&mut self, id: usize, value: f32) -> Result<()> {
        if id >= PARAM_COUNT || !value.is_finite() {
            return Err(self.engine.machine.fault("invalid plugin parameter"));
        }
        let value = value.clamp(0., 1.);
        if self.parameters[id] == value {
            return Ok(());
        }
        self.parameters[id] = value;
        if id < 20 {
            self.engine.set_parameter(id, value)?;
        }
        if (id == SEQ_ENABLE || id == x::MOD_ENABLE || id == x::MODE)
            && value < 0.5
            && self.parameters[SEQ_ENABLE] < 0.5
            && !(self.parameters[x::MODE] >= 0.5 && self.parameters[x::MOD_ENABLE] >= 0.5)
        {
            self.free_samples = 0;
            self.free_beat = 0.;
            self.step = -1;
        }
        Ok(())
    }
    pub fn preset(&mut self, values: [f32; 20], seconds: f32) -> Result<()> {
        self.engine.apply_preset(values, seconds)?;
        self.lock_active_mask = 0;
        self.parameters[..20].copy_from_slice(&values);
        self.parameters[19] = 0.;
        Ok(())
    }
    pub fn program(&mut self, index: usize) -> Result<bool> {
        let Some(p) = self.bank.presets.get(index) else {
            return Ok(false);
        };
        let values = p.values;
        self.preset(values, self.parameters[MORPH] * 10.)?;
        Ok(true)
    }
    fn sync_settings(&mut self, tempo: f64) -> Result<()> {
        self.engine.settings.bypass = self.parameters[BYPASS] >= 0.5;
        self.engine.settings.input_db = -24. + 30. * self.parameters[INPUT_GAIN];
        self.engine.settings.output_db = -12. + 12. * self.parameters[OUTPUT_GAIN];
        self.engine.set_sync(
            self.parameters[DELAY_SYNC] >= 0.5,
            (tempo as f32).clamp(20., 400.),
        )
    }
    pub fn process(
        &mut self,
        audio: &mut [[f32; 2]],
        host_tempo: f64,
        host_beat: f64,
        host_playing: bool,
    ) -> Result<()> {
        let seq = sequence(&self.parameters);
        let host_tempo = if host_tempo.is_finite() && host_tempo > 0. && host_tempo <= 1000. {
            host_tempo
        } else {
            120.
        };
        let host_beat = if host_beat.is_finite() { host_beat } else { 0. };
        self.tempo = if seq.host_sync { host_tempo } else { seq.bpm };
        self.sync_settings(self.tempo)?;
        let playing = if seq.host_sync { host_playing } else { true };
        let start = if seq.host_sync {
            host_beat
        } else {
            self.free_beat
        };
        let increment = self.tempo / (60. * self.sample_rate);
        let extended = self.parameters[x::MODE] >= 0.5;
        if self.was_extended && !extended {
            for (k, id) in x::LOCK_IDS.into_iter().enumerate() {
                if self.lock_active_mask & (1 << k) != 0 {
                    self.engine.set_locked_parameter(id, self.parameters[id])?;
                }
            }
            self.lock_active_mask = 0;
        }
        self.was_extended = extended;
        let modulating = extended && self.parameters[x::MOD_ENABLE] >= 0.5;
        let ducking = extended && self.parameters[x::DUCK_AMOUNT] > 0.;
        let attack = (-1.
            / (x::attack_ms(self.parameters[x::DUCK_ATTACK]) as f64 * 0.001 * self.sample_rate))
            .exp() as f32;
        let release = (-1.
            / (x::release_ms(self.parameters[x::DUCK_RELEASE]) as f64 * 0.001 * self.sample_rate))
            .exp() as f32;
        let threshold = 10f32.powf((-48. + 42. * self.parameters[x::DUCK_THRESHOLD]) / 20.);
        let detector_gain = 10f32.powf(self.engine.settings.input_db / 20.);
        let random_seed = x::seed(self.parameters[x::SEED]);
        for (frame, s) in audio.iter_mut().enumerate() {
            let beat = start + frame as f64 * increment;
            let generated = if extended {
                let pos = x::position(beat, seq.division, seq.length, self.parameters[x::SWING]);
                let accepted = playing
                    && x::accepts(
                        random_seed,
                        pos.absolute,
                        self.parameters[x::PROBABILITY + pos.step],
                    );
                self.step = if playing && (seq.enabled || modulating) {
                    pos.step as i32
                } else {
                    -1
                };
                let locks = if accepted && modulating {
                    x::mask(self.parameters[x::LOCK_MASK + pos.step])
                } else {
                    0
                };
                for (k, id) in x::LOCK_IDS.into_iter().enumerate() {
                    if locks & (1 << k) != 0 {
                        let raw = self.parameters[x::lock_value(pos.step, k)];
                        let v = if id == 19 {
                            if raw >= 0.5 {
                                1.
                            } else {
                                0.
                            }
                        } else {
                            raw
                        };
                        if self.lock_active_mask & (1 << k) == 0 || self.engine.values[id] != v {
                            self.engine.set_locked_parameter(id, v)?;
                        }
                    } else if self.lock_active_mask & (1 << k) != 0 {
                        self.engine.set_locked_parameter(id, self.parameters[id])?;
                    }
                }
                self.lock_active_mask = locks;
                if accepted && seq.enabled {
                    x::sample(
                        &seq,
                        pos,
                        self.tempo,
                        self.parameters[x::GATE + pos.step],
                        x::ratchets(self.parameters[x::RATCHET + pos.step]),
                    )
                } else {
                    [0.; 2]
                }
            } else {
                self.step = seq.step(beat, playing).map_or(-1, |s| s as i32);
                seq.sample(beat, self.tempo, playing)
            };
            s[0] += generated[0];
            s[1] += generated[1];
            if ducking {
                let peak = s[0].abs().max(s[1].abs()) * detector_gain;
                let coefficient = if peak > self.duck_envelope {
                    attack
                } else {
                    release
                };
                self.duck_envelope = coefficient * self.duck_envelope + (1. - coefficient) * peak;
                let activity = (1. - threshold / self.duck_envelope.max(threshold)).clamp(0., 1.);
                self.engine.wet_gain = 1. - self.parameters[x::DUCK_AMOUNT] * activity;
            } else {
                self.duck_envelope = 0.;
                self.engine.wet_gain = 1.;
            }
            self.duck_reduction = 1. - self.engine.wet_gain;
            if self.sample_rate == 48000. {
                let mut out = [[0.; 2]];
                self.engine.process(std::slice::from_ref(s), &mut out)?;
                *s = out[0];
            } else {
                self.input.push(*s);
                while let Some(core) = self.input.pop() {
                    let mut out = [[0.; 2]];
                    self.engine.process(&[core], &mut out)?;
                    self.output.push(out[0]);
                    while let Some(converted) = self.output.pop() {
                        if self.len == 64 {
                            return Err(self.engine.machine.fault("resampler overflow"));
                        }
                        self.queue[(self.read + self.len) % 64] = converted;
                        self.len += 1;
                    }
                }
                if self.len == 0 {
                    return Err(self.engine.machine.fault("resampler underflow"));
                }
                *s = self.queue[self.read];
                self.read = (self.read + 1) % 64;
                self.len -= 1;
            }
        }
        if !seq.host_sync && (seq.enabled || modulating) {
            self.free_beat += audio.len() as f64 * increment;
            self.free_samples += audio.len() as u64;
        }
        Ok(())
    }
}
