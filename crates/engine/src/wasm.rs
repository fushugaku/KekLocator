use crate::Engine;
use wasm_bindgen::prelude::*;
const CAPACITY: usize = 1024;
fn js_error(e: impl ToString) -> JsValue {
    JsValue::from_str(&e.to_string())
}

#[wasm_bindgen]
pub struct AudioEngine {
    engine: Engine,
    input: Vec<f32>,
    output: Vec<f32>,
    interleaved_in: Vec<[f32; 2]>,
    interleaved_out: Vec<[f32; 2]>,
}
#[wasm_bindgen]
impl AudioEngine {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<AudioEngine, JsValue> {
        Ok(Self {
            engine: Engine::new().map_err(js_error)?,
            input: vec![0.0; CAPACITY * 2],
            output: vec![0.0; CAPACITY * 2],
            interleaved_in: vec![[0.0; 2]; CAPACITY],
            interleaved_out: vec![[0.0; 2]; CAPACITY],
        })
    }
    pub fn input_left(&mut self) -> *mut f32 {
        self.input.as_mut_ptr()
    }
    pub fn input_right(&mut self) -> *mut f32 {
        self.input[CAPACITY..].as_mut_ptr()
    }
    pub fn output_left(&self) -> *const f32 {
        self.output.as_ptr()
    }
    pub fn output_right(&self) -> *const f32 {
        self.output[CAPACITY..].as_ptr()
    }
    pub fn process(&mut self, frames: usize) -> Result<(), JsValue> {
        if frames > CAPACITY {
            return Err(js_error("Audio block exceeds 1024 frames"));
        }
        for i in 0..frames {
            self.interleaved_in[i] = [self.input[i], self.input[CAPACITY + i]];
        }
        self.engine
            .process(
                &self.interleaved_in[..frames],
                &mut self.interleaved_out[..frames],
            )
            .map_err(js_error)?;
        for i in 0..frames {
            self.output[i] = self.interleaved_out[i][0];
            self.output[CAPACITY + i] = self.interleaved_out[i][1];
        }
        Ok(())
    }
    pub fn set_parameter(&mut self, id: usize, value: f32) -> Result<(), JsValue> {
        self.engine.set_parameter(id, value).map_err(js_error)
    }
    pub fn set_preset(&mut self, values: &[f32], seconds: f32) -> Result<(), JsValue> {
        let values: [f32; 20] = values
            .try_into()
            .map_err(|_| js_error("A preset needs 20 parameters"))?;
        self.engine.apply_preset(values, seconds).map_err(js_error)
    }
    pub fn set_host(&mut self, input_db: f32, output_db: f32, bypass: bool) {
        self.engine.settings.input_db = if input_db.is_finite() {
            input_db.clamp(-24.0, 6.0)
        } else {
            0.0
        };
        self.engine.settings.output_db = if output_db.is_finite() {
            output_db.clamp(-12.0, 0.0)
        } else {
            0.0
        };
        self.engine.settings.bypass = bypass;
    }
    pub fn set_sync(&mut self, enabled: bool, bpm: f32) -> Result<(), JsValue> {
        self.engine.set_sync(enabled, bpm).map_err(js_error)
    }
    pub fn clear(&mut self) -> Result<(), JsValue> {
        self.engine.reset().map_err(js_error)
    }
    pub fn morph_progress(&self) -> f32 {
        self.engine.morph_progress()
    }
    pub fn instructions(&self) -> f64 {
        self.engine.machine.ticks as f64
    }
    pub fn delay_times(&self) -> Vec<f32> {
        (0..8)
            .map(|i| {
                self.engine
                    .machine
                    .read_f32(crate::engine::STATE_ADDRESS + 0xbd0 + i * 4)
                    .unwrap_or(0.0)
            })
            .collect()
    }
}
#[wasm_bindgen]
pub fn encode_wav(samples: &[f32], sample_rate: u32) -> Result<Vec<u8>, JsValue> {
    crate::wave::encode_wav(samples, 2, sample_rate).map_err(js_error)
}

/// Browser preview of the same native plug-in adapter and beat-driven sequencer.
#[wasm_bindgen]
pub struct DawEngine {
    dsp: crate::daw::DawProcessor,
    input: Vec<f32>,
    output: Vec<f32>,
    audio: Vec<[f32; 2]>,
}
#[wasm_bindgen]
impl DawEngine {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<Self, JsValue> {
        Ok(Self {
            dsp: crate::daw::DawProcessor::new(48000.).map_err(js_error)?,
            input: vec![0.; CAPACITY * 2],
            output: vec![0.; CAPACITY * 2],
            audio: vec![[0.; 2]; CAPACITY],
        })
    }
    pub fn input_left(&mut self) -> *mut f32 {
        self.input.as_mut_ptr()
    }
    pub fn input_right(&mut self) -> *mut f32 {
        self.input[CAPACITY..].as_mut_ptr()
    }
    pub fn output_left(&self) -> *const f32 {
        self.output.as_ptr()
    }
    pub fn output_right(&self) -> *const f32 {
        self.output[CAPACITY..].as_ptr()
    }
    pub fn set_parameter(&mut self, id: usize, value: f32) -> Result<(), JsValue> {
        self.dsp.set_parameter(id, value).map_err(js_error)
    }
    pub fn set_preset(&mut self, values: &[f32], seconds: f32) -> Result<(), JsValue> {
        let values = values
            .try_into()
            .map_err(|_| js_error("20 preset parameters required"))?;
        self.dsp.preset(values, seconds).map_err(js_error)
    }
    pub fn process(
        &mut self,
        frames: usize,
        bpm: f64,
        beat: f64,
        playing: bool,
    ) -> Result<(), JsValue> {
        if frames > CAPACITY {
            return Err(js_error("Block exceeds capacity"));
        }
        for i in 0..frames {
            self.audio[i] = [self.input[i], self.input[CAPACITY + i]];
        }
        self.dsp
            .process(&mut self.audio[..frames], bpm, beat, playing)
            .map_err(js_error)?;
        for i in 0..frames {
            self.output[i] = self.audio[i][0];
            self.output[CAPACITY + i] = self.audio[i][1];
        }
        Ok(())
    }
    pub fn duck_reduction(&self) -> f32 {
        self.dsp.duck_reduction
    }
    pub fn step(&self) -> i32 {
        self.dsp.step
    }
    pub fn tempo(&self) -> f64 {
        self.dsp.tempo
    }
}
