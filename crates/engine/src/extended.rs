//! Optional performance controls. The firmware path is unchanged in Original.
use crate::{
    daw,
    parameters::PARAMETERS,
    sequencer::{Sequence, DIVISIONS, LABELS},
};
pub const MODE: usize = 64;
pub const DUCK_AMOUNT: usize = 65;
pub const DUCK_THRESHOLD: usize = 66;
pub const DUCK_ATTACK: usize = 67;
pub const DUCK_RELEASE: usize = 68;
pub const SWING: usize = 69;
pub const SEED: usize = 70;
pub const MOD_ENABLE: usize = 71;
pub const PROBABILITY: usize = 72;
pub const GATE: usize = 88;
pub const RATCHET: usize = 104;
pub const LOCK_MASK: usize = 120;
pub const LOCK_VALUES: usize = 136;
pub const PARAM_COUNT: usize = 216;
pub const LOCK_IDS: [usize; 5] = [3, 9, 4, 7, 19];
pub const LOCK_NAMES: [&str; 5] = ["Time", "Feedback", "Filter", "Pitch", "Freeze"];
pub fn lock_value(step: usize, control: usize) -> usize {
    LOCK_VALUES + step * 5 + control
}
pub fn seed(value: f32) -> u32 {
    (value * 65535.).round() as u32
}
pub fn ratchets(value: f32) -> usize {
    (value * 7.).round() as usize + 1
}
pub fn mask(value: f32) -> u32 {
    (value * 31.).round() as u32
}
pub fn attack_ms(value: f32) -> f32 {
    100f32.powf(value)
}
pub fn release_ms(value: f32) -> f32 {
    20. * 100f32.powf(value)
}
pub fn initialize(p: &mut [f32; PARAM_COUNT]) {
    p[DUCK_THRESHOLD] = 24. / 42.; // -24 dB
    p[DUCK_ATTACK] = 0.5; // 10 ms
    p[DUCK_RELEASE] = (15f32).log(100.); // 300 ms
    p[SEED] = 1337. / 65535.;
    for step in 0..16 {
        p[PROBABILITY + step] = 1.;
        p[GATE + step] = 1.;
        for (k, &id) in LOCK_IDS.iter().enumerate() {
            p[lock_value(step, k)] = p[id];
        }
    }
}
pub fn name(id: usize) -> String {
    match id {
        MODE => "Mode".into(),
        DUCK_AMOUNT => "Duck amount".into(),
        DUCK_THRESHOLD => "Duck threshold".into(),
        DUCK_ATTACK => "Duck attack".into(),
        DUCK_RELEASE => "Duck release".into(),
        SWING => "Swing".into(),
        SEED => "Random seed".into(),
        MOD_ENABLE => "Effect sequencer".into(),
        PROBABILITY..=87 => format!("Step {} probability", id - PROBABILITY + 1),
        GATE..=103 => format!("Step {} gate", id - GATE + 1),
        RATCHET..=119 => format!("Step {} ratchets", id - RATCHET + 1),
        LOCK_MASK..=135 => format!("Step {} locks", id - LOCK_MASK + 1),
        LOCK_VALUES..=215 => format!(
            "Step {} {}",
            (id - LOCK_VALUES) / 5 + 1,
            LOCK_NAMES[(id - LOCK_VALUES) % 5]
        ),
        _ => String::new(),
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Position {
    pub step: usize,
    pub absolute: i64,
    pub phase: f64,
    pub duration: f64,
}
/// Swing delays odd steps; a pair still occupies exactly two divisions.
pub fn position(beat: f64, division: usize, length: usize, swing: f32) -> Position {
    let d = DIVISIONS[division.min(10)];
    let pair = (beat / (2. * d) + 1e-10).floor();
    let local = (beat - pair * 2. * d).max(0.);
    let first = d * (1. + swing as f64 * 0.5);
    let odd = local + 1e-10 >= first;
    let duration = if odd { 2. * d - first } else { first };
    let absolute = pair as i64 * 2 + i64::from(odd);
    Position {
        step: absolute.rem_euclid(length.clamp(1, 16) as i64) as usize,
        absolute,
        phase: ((local - if odd { first } else { 0. }) / duration).clamp(0., 1.),
        duration,
    }
}
/// Counter-based random: independent of buffer size, seeks and prior playback.
pub fn accepts(seed: u32, absolute: i64, probability: f32) -> bool {
    if probability >= 1. {
        return true;
    }
    if probability <= 0. {
        return false;
    }
    let mut z = (absolute as u64)
        .wrapping_add((seed as u64) << 32)
        .wrapping_add(0x9e3779b97f4a7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    z ^= z >> 31;
    (z >> 40) as f64 / 16777216. < probability as f64
}
pub fn sample(seq: &Sequence, pos: Position, bpm: f64, gate: f32, repeats: usize) -> [f32; 2] {
    let phase = (pos.phase * repeats as f64).fract();
    if phase >= gate as f64 || gate <= 0. || seq.velocities[pos.step] <= 0. {
        return [0.; 2];
    }
    let seconds = pos.duration * 60. / bpm / repeats as f64;
    let local = phase * seconds;
    let note = (seq.notes[pos.step] as i32 + seq.transpose).clamp(0, 127);
    let angle = local * 440. * 2f64.powf((note as f64 - 69.) / 12.) * std::f64::consts::TAU;
    let pluck = (angle.sin() + 0.3 * (angle * 2.).sin()) * (-local * 7.).exp() * 0.16;
    let tick = ((local * 1371.35).sin() + (local * 7813.91).sin()) * (-local * 140.).exp() * 0.008;
    let gain = seq.velocities[pos.step] as f64
        * seq.level as f64
        * (local / 0.002).clamp(0., 1.)
        * ((gate as f64 - phase) * seconds / 0.004).clamp(0., 1.);
    [
        ((pluck + tick) * gain) as f32,
        ((pluck * 0.93 - tick) * gain) as f32,
    ]
}
/// Host/UI text. The normalized parameter domain stays compatible with 1.0 projects.
pub fn display(id: usize, value: f32, p: &[f32; PARAM_COUNT], tempo: f64) -> String {
    let v = value.clamp(0., 1.);
    match id {
        3 if p[daw::DELAY_SYNC] >= 0.5 => {
            crate::midi::SYNC_LABELS[((v * 16.99) as usize).min(16)].into()
        }
        7 if p[17] >= 0.5 => format!("{:+.1} Hz", PARAMETERS[7].physical(v)),
        0..=19 => PARAMETERS[id].display(v),
        daw::INPUT_GAIN => format!("{:+.1} dB", -24. + 30. * v),
        daw::OUTPUT_GAIN => format!("{:+.1} dB", -12. + 12. * v),
        daw::TEMPO => format!("{:.1} BPM", 20. + 380. * v),
        daw::SEQ_RATE => LABELS[(v * 10.).round() as usize].into(),
        daw::TRANSPOSE => format!("{:+.0} st", v * 48. - 24.),
        daw::LENGTH => format!("{}", (v * 15.).round() as usize + 1),
        daw::NOTES..=46 => format!("{}", (v * 127.).round() as u32),
        daw::MORPH => format!("{:.2} s", v * 10.),
        MODE => if v >= 0.5 { "Extended" } else { "Original" }.into(),
        DUCK_THRESHOLD => format!("{:.1} dB", -48. + 42. * v),
        DUCK_ATTACK => format!("{:.1} ms", attack_ms(v)),
        DUCK_RELEASE => format!("{:.0} ms", release_ms(v)),
        SWING => format!("{:.0}%", v * 50.),
        SEED => format!("{}", seed(v)),
        RATCHET..=119 => format!("{}", ratchets(v)),
        LOCK_MASK..=135 => format!("{}", mask(v)),
        LOCK_VALUES..=215 => display(LOCK_IDS[(id - LOCK_VALUES) % 5], v, p, tempo),
        daw::BYPASS | daw::DELAY_SYNC | daw::SEQ_ENABLE | daw::SEQ_HOST | MOD_ENABLE => {
            if v >= 0.5 { "On" } else { "Off" }.into()
        }
        _ => format!("{:.0}%", v * 100.),
    }
}

/// Parse the units shown to the host without changing saved automation domains.
pub fn parse(id: usize, text: &str, p: &[f32; PARAM_COUNT]) -> Option<f32> {
    if id >= PARAM_COUNT {
        return None;
    }
    let text = text.trim().to_lowercase();
    let id = if (LOCK_VALUES..PARAM_COUNT).contains(&id) {
        LOCK_IDS[(id - LOCK_VALUES) % 5]
    } else {
        id
    };
    if id == 3 && p[daw::DELAY_SYNC] >= 0.5 {
        return crate::midi::SYNC_LABELS
            .iter()
            .position(|s| s.to_lowercase() == text)
            .map(|i| (i as f32 + 0.5) / 16.99);
    }
    if id == daw::SEQ_RATE {
        return LABELS
            .iter()
            .position(|s| s.to_lowercase() == text)
            .map(|i| i as f32 / 10.);
    }
    if id == MODE {
        return match text.as_str() {
            "original" => Some(0.),
            "extended" => Some(1.),
            _ => None,
        };
    }
    if id < 20 && PARAMETERS[id].toggle
        || [
            daw::BYPASS,
            daw::DELAY_SYNC,
            daw::SEQ_ENABLE,
            daw::SEQ_HOST,
            MOD_ENABLE,
        ]
        .contains(&id)
    {
        return match text.as_str() {
            "on" | "true" | "1" | "bbd" | "b" | "series" | "frequency" => Some(1.),
            "off" | "false" | "0" | "tape" | "a" | "feedback" | "pitch" => Some(0.),
            _ => None,
        };
    }
    if id == 11 {
        if text == "centre" || text == "center" {
            return Some(0.5);
        }
        if let Some(t) = text.strip_prefix('l') {
            return t.trim().parse::<f32>().ok().map(|v| (1. - v / 100.) * 0.5);
        }
        if let Some(t) = text.strip_prefix('r') {
            return t.trim().parse::<f32>().ok().map(|v| (1. + v / 100.) * 0.5);
        }
    }
    let end = text
        .find(|c: char| !c.is_ascii_digit() && c != '.' && c != '-' && c != '+' && c != 'e')
        .unwrap_or(text.len());
    let v = text[..end].trim().parse::<f32>().ok()?;
    if !v.is_finite() {
        return None;
    }
    let unit = text[end..].trim();
    let physical = match id {
        3 => {
            let seconds = if unit == "s" { v } else { v / 1000. };
            if seconds <= 0. {
                return None;
            }
            12. * seconds.log2()
        }
        4 => {
            let hz = if unit == "khz" { v * 1000. } else { v };
            if hz <= 0. {
                return None;
            }
            69. + 12. * (hz / 440.).log2()
        }
        5 | 7 | 11 => v,
        0..=19 => v / 100.,
        daw::INPUT_GAIN => return Some(((v + 24.) / 30.).clamp(0., 1.)),
        daw::OUTPUT_GAIN => return Some(((v + 12.) / 12.).clamp(0., 1.)),
        daw::TEMPO => return Some(((v - 20.) / 380.).clamp(0., 1.)),
        daw::TRANSPOSE => return Some(((v + 24.) / 48.).clamp(0., 1.)),
        daw::LENGTH => return Some(((v - 1.) / 15.).clamp(0., 1.)),
        daw::NOTES..=46 => return Some((v / 127.).clamp(0., 1.)),
        daw::MORPH => return Some((v / 10.).clamp(0., 1.)),
        DUCK_THRESHOLD => return Some(((v + 48.) / 42.).clamp(0., 1.)),
        DUCK_ATTACK => return Some(v.max(1.).log(100.).clamp(0., 1.)),
        DUCK_RELEASE => return Some((v.max(20.) / 20.).log(100.).clamp(0., 1.)),
        SWING => return Some((v / 50.).clamp(0., 1.)),
        SEED => return Some((v / 65535.).clamp(0., 1.)),
        RATCHET..=119 => return Some(((v - 1.) / 7.).clamp(0., 1.)),
        LOCK_MASK..=135 => return Some((v / 31.).clamp(0., 1.)),
        _ => return Some((v / 100.).clamp(0., 1.)),
    };
    let param = &PARAMETERS[id];
    Some(((physical - param.min) / (param.max - param.min)).clamp(0., 1.))
}
