import './utf8.js';
import { initSync, AudioEngine } from './pkg/engine/engine.js';

class KekLocatorProcessor extends AudioWorkletProcessor {
  constructor(options) {
    super();
    this.failed = false;
    this.recording = false;
    this.recordBuffer = new Float32Array(2048);
    this.recordOffset = 0;
    this.frames = 0;
    this.sumL = 0; this.sumR = 0;
    this.wave = [];
    try {
      this.wasm = initSync({ module: options.processorOptions.module });
      this.engine = new AudioEngine();
      this.refresh();
      this.port.onmessage = ({ data }) => {
        try {
          switch (data.type) {
            case 'parameter': this.engine.set_parameter(data.id, data.value); break;
            case 'preset': this.engine.set_preset(new Float32Array(data.values), data.seconds || 0); break;
            case 'host': this.engine.set_host(data.input_db, data.output_db, data.bypass); break;
            case 'sync': this.engine.set_sync(data.enabled, data.bpm); break;
            case 'clear': this.engine.clear(); this.failed = false; break;
            case 'record':
              this.recording = data.enabled;
              if (!data.enabled) { this.flushRecord(); this.port.postMessage({ type: 'record-ended' }); }
              break;
          }
        } catch (error) { this.fail(error); }
      };
      this.port.postMessage({ type: 'ready', sampleRate });
    } catch (error) { this.fail(error); }
  }
  refresh() {
    if (this.buffer === this.wasm.memory.buffer) return;
    this.buffer = this.wasm.memory.buffer;
    this.inL = new Float32Array(this.buffer, this.engine.input_left(), 1024);
    this.inR = new Float32Array(this.buffer, this.engine.input_right(), 1024);
    this.outL = new Float32Array(this.buffer, this.engine.output_left(), 1024);
    this.outR = new Float32Array(this.buffer, this.engine.output_right(), 1024);
  }
  fail(error) {
    this.failed = true;
    this.port.postMessage({ type: 'error', message: String(error?.message || error) });
  }
  flushRecord() {
    if (!this.recordOffset) return;
    const data = this.recordBuffer.slice(0, this.recordOffset);
    this.port.postMessage({ type: 'record', data }, [data.buffer]);
    this.recordOffset = 0;
  }
  process(inputs, outputs) {
    const output = outputs[0];
    if (!output?.length) return true;
    const n = output[0].length;
    if (this.failed || !this.engine) { for (const channel of output) channel.fill(0); return true; }
    try {
      this.refresh();
      const input = inputs[0];
      if (input?.[0]) this.inL.set(input[0]); else this.inL.fill(0, 0, n);
      if (input?.[1]) this.inR.set(input[1]); else if (input?.[0]) this.inR.set(input[0]); else this.inR.fill(0, 0, n);
      this.engine.process(n);
      output[0].set(this.outL.subarray(0, n));
      if (output[1]) output[1].set(this.outR.subarray(0, n));
      for (let i = 0; i < n; i++) {
        const l = this.outL[i], r = this.outR[i];
        this.sumL += l * l; this.sumR += r * r;
        if (this.recording) {
          this.recordBuffer[this.recordOffset++] = l;
          this.recordBuffer[this.recordOffset++] = r;
          if (this.recordOffset === this.recordBuffer.length) this.flushRecord();
        }
      }
      this.wave.push([this.outL[n >> 1], this.outR[n >> 1]]);
      this.frames += n;
      if (this.frames >= 2048) {
        this.port.postMessage({ type: 'meter', left: Math.sqrt(this.sumL / this.frames), right: Math.sqrt(this.sumR / this.frames),
          wave: this.wave, morph: this.engine.morph_progress(), taps: Array.from(this.engine.delay_times()) });
        this.frames = 0; this.sumL = 0; this.sumR = 0; this.wave = [];
      }
    } catch (error) { this.fail(error); for (const channel of output) channel.fill(0); }
    return true;
  }
}
registerProcessor('keklocator', KekLocatorProcessor);
