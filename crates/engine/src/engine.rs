use crate::machine::{Machine, Result};
use crate::parameters::{default_values, HostSettings, PARAMETERS};
pub const SAMPLE_RATE: u32 = 48000;
pub const FRAMES_PER_FIRMWARE_BLOCK: usize = 24;
pub const STATE_ADDRESS: u32 = 0x20000628;
const INPUT: u32 = 0x20002674;
const OUTPUT: u32 = 0x200027f4;

pub struct Engine {
    pub machine: Machine,
    pub values: [f32; 20],
    pub settings: HostSettings,
    pub sync_enabled: bool,
    pub tempo: f32,
    /// Optional attenuation of the final wet mix only; 1.0 takes the untouched path.
    pub wet_gain: f32,
    pending_input: [[f32; 2]; 24],
    pending_output: [[f32; 2]; 24],
    index: usize,
    available: usize,
    output_index: usize,
    morph_index: u32,
    morph_blocks: u32,
    morph_active: bool,
}
impl Engine {
    pub fn new() -> Result<Self> {
        let mut machine = Machine::new();
        // Install the original settings defaults without invoking physical OLED/UART/GPIO setup.
        machine.store(0x20003414, 4, 0x20001234)?;
        for i in 0..36 {
            let a = 0x20001234 + i * 32;
            let value = machine.load(a + 16, 4)?;
            machine.store(a + 4, 4, value)?;
        }
        let values = default_values();
        for (i, &value) in values.iter().enumerate() {
            machine.write_f32(0x200000a8 + i as u32 * 72, value)?;
        }
        machine.invoke(0x6002a77c, &[], None, 10_000_000)?;
        // The web version has no analogue VCA: use the firmware's digital mix route.
        machine.invoke(0x6002aabc, &[0], None, 1_000_000)?;
        let mut engine = Self {
            machine,
            values,
            settings: HostSettings::default(),
            sync_enabled: false,
            tempo: 120.0,
            wet_gain: 1.0,
            pending_input: [[0.0; 2]; 24],
            pending_output: [[0.0; 2]; 24],
            index: 0,
            available: 0,
            output_index: 0,
            morph_index: 0,
            morph_blocks: 0,
            morph_active: false,
        };
        // A physical device has already processed silence by the time audio arrives.
        for _ in 0..128 {
            engine.firmware_block()?;
        }
        Ok(engine)
    }
    pub fn set_parameter(&mut self, id: usize, normalized: f32) -> Result<()> {
        if id >= 20 || !normalized.is_finite() {
            return Err(self.machine.fault("invalid parameter"));
        }
        let value = if PARAMETERS[id].toggle {
            if normalized >= 0.5 {
                1.0
            } else {
                0.0
            }
        } else {
            normalized.clamp(0.0, 1.0)
        };
        self.values[id] = value;
        self.machine.write_f32(0x200000a8 + id as u32 * 72, value)?;
        if self.morph_active {
            return Ok(());
        }
        // ITCM control tick 0x322c only applies a callback when normalized input
        // differs from the last applied value. Reapplying Dirty resets delay state.
        let previous = 0x200000b0 + id as u32 * 72;
        if self.machine.read_f32(previous)? == value {
            return Ok(());
        }
        self.machine.write_f32(previous, value)?;
        let callback = self.machine.load(0x200000c8 + id as u32 * 72, 4)? as u32;
        let mapped = PARAMETERS[id].physical(value);
        self.machine
            .invoke(callback, &[STATE_ADDRESS], Some(mapped), 1_000_000)?;
        Ok(())
    }
    /// A step lock takes ownership of just this control while other preset ramps continue.
    pub fn set_locked_parameter(&mut self, id: usize, value: f32) -> Result<()> {
        if id >= 20 {
            return Err(self.machine.fault("invalid lock parameter"));
        }
        self.machine.store(0x200000c0 + id as u32 * 72, 1, 0)?;
        let morph = self.morph_active;
        self.morph_active = false;
        let result = self.set_parameter(id, value);
        self.morph_active = morph;
        result
    }
    pub fn apply_preset(&mut self, mut values: [f32; 20], seconds: f32) -> Result<()> {
        values[19] = 0.0;
        if values
            .iter()
            .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
        {
            return Err(self.machine.fault("invalid preset values"));
        }
        if !seconds.is_finite() {
            return Err(self.machine.fault("invalid morph duration"));
        }
        for i in 0..20 {
            if PARAMETERS[i].toggle {
                values[i] = if values[i] >= 0.5 { 1.0 } else { 0.0 };
            }
        }
        self.morph_index = 0;
        self.morph_blocks = (seconds.clamp(0.0, 10.0) * 2000.0).round() as u32;
        self.morph_active = false;
        for i in 0..20 {
            self.machine.store(0x200000c0 + i * 72, 1, 0)?;
        }
        if self.morph_blocks == 0 {
            for (i, value) in values.into_iter().enumerate() {
                self.set_parameter(i, value)?;
            }
        } else {
            self.set_parameter(19, 0.0)?;
            let blocks = seconds.clamp(0.0, 10.0) * 2000.0;
            for i in 0..19 {
                let current = self.machine.read_f32(0x200000a8 + i as u32 * 72)?;
                let step = (values[i] - current) / blocks;
                self.machine
                    .write_f32(0x200000ac + i as u32 * 72, values[i])?;
                self.machine.write_f32(0x200000bc + i as u32 * 72, step)?;
                self.machine
                    .store(0x200000c0 + i as u32 * 72, 1, (step != 0.0) as u64)?;
                self.morph_active |= step != 0.0;
            }
        }
        Ok(())
    }
    /// Run the original 2 kHz control tick for a raw firmware audio block.
    pub fn advance_controls(&mut self) -> Result<()> {
        if !self.morph_active {
            return Ok(());
        }
        // The browser replaces ADC pots. Detach only their input acquisition while
        // the original tick performs incremental f32 ramps, mapping and callbacks.
        let mut sources = [0u64; 20];
        for (i, source) in sources.iter_mut().enumerate() {
            let address = 0x200000cc + i as u32 * 72;
            *source = self.machine.load(address, 1)?;
            self.machine.store(address, 1, 0)?;
        }
        let result = self.machine.invoke(0x322c, &[], None, 5_000_000);
        for (i, source) in sources.into_iter().enumerate() {
            self.machine.store(0x200000cc + i as u32 * 72, 1, source)?;
        }
        result?;
        self.morph_active = false;
        for i in 0..20 {
            self.values[i] = self.machine.read_f32(0x200000a8 + i as u32 * 72)?;
            self.morph_active |= self.machine.load(0x200000c0 + i as u32 * 72, 1)? != 0;
        }
        self.morph_index += 1;
        Ok(())
    }
    fn firmware_block(&mut self) -> Result<()> {
        self.advance_controls()?;
        let gain = 10f32.powf(self.settings.input_db.clamp(-24.0, 6.0) / 20.0);
        for i in 0..24 {
            for channel in 0..2 {
                let sample = (self.pending_input[i][channel] * gain).clamp(-1.0, 1.0);
                let pcm = (sample as f64 * 2147483648.0) as i32;
                self.machine
                    .store(INPUT + (i * 2 + channel) as u32 * 4, 4, pcm as u32 as u64)?;
            }
        }
        // ITCM literal 0x16e8 points at the final wet gain (0x2000122c).
        // DSP_Process_Block reads it only in the final L/R mixer, after feedback.
        // Restore it after the call so ducking never alters firmware control state.
        let wet = self.machine.read_f32(0x2000122c)?;
        let duck = self.wet_gain.clamp(0.0, 1.0);
        if duck != 1.0 {
            self.machine.write_f32(0x2000122c, wet * duck)?;
        }
        let result = self
            .machine
            .invoke(0x208, &[INPUT, OUTPUT, 24], None, 5_000_000);
        if duck != 1.0 {
            self.machine.write_f32(0x2000122c, wet)?;
        }
        result?;
        let gain = 10f32.powf(self.settings.output_db.clamp(-12.0, 0.0) / 20.0);
        for i in 0..24 {
            for channel in 0..2 {
                let pcm = self
                    .machine
                    .load(OUTPUT + (i * 2 + channel) as u32 * 4, 4)?
                    as u32 as i32;
                let sample = pcm as f32 * (1.0 / 2147483648.0);
                self.pending_output[i][channel] = if self.settings.bypass {
                    self.pending_input[i][channel]
                } else {
                    sample * gain
                };
            }
        }
        Ok(())
    }
    pub fn process(&mut self, input: &[[f32; 2]], output: &mut [[f32; 2]]) -> Result<()> {
        if input.len() != output.len() {
            return Err(self.machine.fault("input/output length mismatch"));
        }
        for (sample, out) in input.iter().zip(output.iter_mut()) {
            self.pending_input[self.index] = *sample;
            self.index += 1;
            if self.index == 24 {
                self.firmware_block()?;
                self.index = 0;
                self.available = 24;
                self.output_index = 0;
            }
            *out = if self.available > 0 {
                let value = self.pending_output[self.output_index];
                self.output_index += 1;
                self.available -= 1;
                value
            } else {
                [0.0; 2]
            };
        }
        Ok(())
    }
    pub fn reset(&mut self) -> Result<()> {
        let values = self.values;
        let settings = self.settings.clone();
        let sync = self.sync_enabled;
        let tempo = self.tempo;
        *self = Self::new()?;
        self.settings = settings;
        self.apply_preset(values, 0.0)?;
        self.set_sync(sync, tempo)
    }
    pub fn set_sync(&mut self, enabled: bool, bpm: f32) -> Result<()> {
        if !bpm.is_finite() || !(20.0..=400.0).contains(&bpm) {
            return Err(self.machine.fault("invalid tempo"));
        }
        if self.sync_enabled == enabled && self.tempo == bpm {
            return Ok(());
        }
        self.sync_enabled = enabled;
        self.tempo = bpm;
        self.machine
            .store(0x20001234 + 27 * 32 + 4, 1, enabled as u64)?;
        self.machine.write_f32(0x20004f10, 60.0 / bpm)?;
        self.machine.store(0x20001e70, 1, 255)?;
        // Clock routing invalidates the Time mapping even at an unchanged knob.
        self.machine.write_f32(0x200000b0 + 3 * 72, -1.0)?;
        self.set_parameter(3, self.values[3])
    }
    pub fn morph_progress(&self) -> f32 {
        if self.morph_blocks == 0 || !self.morph_active {
            1.0
        } else {
            (self.morph_index as f32 / self.morph_blocks as f32).min(1.0)
        }
    }
}
