use serde::{Deserialize, Serialize};

pub const PARAMETER_COUNT: usize = 20;
#[derive(Clone, Copy, Debug)]
pub struct ParamDef {
    pub id: usize,
    pub key: &'static str,
    pub label: &'static str,
    pub min: f32,
    pub max: f32,
    pub initial: f32,
    pub toggle: bool,
    pub cc: Option<u8>,
}
#[rustfmt::skip]
#[path="parameter_table.rs"] mod table;
pub use table::PARAMETERS;
impl ParamDef {
    pub fn normalized_default(&self) -> f32 {
        ((self.initial - self.min) / (self.max - self.min)).clamp(0.0, 1.0)
    }
    pub fn physical(&self, normalized: f32) -> f32 {
        // The firmware's mapper at 0x6002cbe0 uses VFP VFMA (one rounding).
        (self.max - self.min).mul_add(normalized.clamp(0.0, 1.0), self.min)
    }
    pub fn display(&self, normalized: f32) -> String {
        let p = self.physical(normalized);
        match self.id {
            0 => if p >= 0.5 { "BBD" } else { "Tape" }.into(),
            3 => {
                let ms = 1000.0 * 2.0f32.powf(p / 12.0);
                if ms >= 1000.0 {
                    format!("{:.2} s", ms / 1000.0)
                } else {
                    format!("{:.0} ms", ms)
                }
            }
            4 => {
                let hz = 440.0 * 2.0f32.powf((p - 69.0) / 12.0);
                if hz >= 1000.0 {
                    format!("{:.2} kHz", hz / 1000.0)
                } else {
                    format!("{hz:.0} Hz")
                }
            }
            5 => format!("{p:.2} Hz"),
            7 => format!("{p:+.1} st"),
            11 => {
                if p.abs() < 0.02 {
                    "Centre".into()
                } else if p < 0.0 {
                    format!("L {:.0}", -p * 100.0)
                } else {
                    format!("R {:.0}", p * 100.0)
                }
            }
            13 => if p >= 0.5 { "B" } else { "A" }.into(),
            14 => if p >= 0.5 { "Series" } else { "Feedback" }.into(),
            17 => if p >= 0.5 { "Frequency" } else { "Pitch" }.into(),
            _ if self.toggle => if p >= 0.5 { "On" } else { "Off" }.into(),
            _ => format!("{:.0}%", p * 100.0),
        }
    }
}
pub fn default_values() -> [f32; PARAMETER_COUNT] {
    std::array::from_fn(|i| PARAMETERS[i].normalized_default())
}

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct HostSettings {
    pub input_db: f32,
    pub output_db: f32,
    pub bypass: bool,
    pub morph_seconds: f32,
    pub midi_channel: u8,
    pub midi_out_channel: u8,
    pub midi_thru: bool,
    pub midi_output: bool,
    pub midi_cc: [u8; 13],
    pub load_last: bool,
    pub footswitch_single: String,
    pub footswitch_double: String,
    pub footswitch_hold: String,
}
impl Default for HostSettings {
    fn default() -> Self {
        Self {
            input_db: 0.0,
            output_db: 0.0,
            bypass: false,
            morph_seconds: 0.0,
            midi_channel: 1,
            midi_out_channel: 1,
            midi_thru: false,
            midi_output: false,
            midi_cc: std::array::from_fn(|i| 70 + i as u8),
            load_last: true,
            footswitch_single: "freeze".into(),
            footswitch_double: "bypass".into(),
            footswitch_hold: "off".into(),
        }
    }
}
