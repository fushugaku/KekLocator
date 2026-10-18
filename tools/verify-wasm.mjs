// Integration check: no browser automation. Compare native and WASM PCM from the same source.
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { resolve } from 'node:path';
import { performance } from 'node:perf_hooks';
import { initSync, AudioEngine, encode_wav } from '../dist/pkg/engine/engine.js';

const root = fileURLToPath(new URL('..', import.meta.url));
const native = resolve(root, 'target/release/keklocator');
const bank = JSON.parse(execFileSync(native, ['bank'], { encoding: 'utf8' }));
const wasm = initSync({ module: readFileSync(resolve(root, 'dist/pkg/engine/engine_bg.wasm')) });
const directory = resolve(root, 'research'); mkdirSync(directory, { recursive: true });
const frames = 96000, raw = new Float32Array(frames * 2);
for (let i = 0; i < 24000; i++) {
  const envelope = Math.exp(-(i % 6000) / 1500);
  raw[i * 2] = Math.sin(i * 2 * Math.PI * 220 / 48000) * envelope * 0.12;
  raw[i * 2 + 1] = Math.sin(i * 2 * Math.PI * 277.18 / 48000) * envelope * 0.1;
}
const input = Buffer.from(encode_wav(raw, 48000));
const inputPath = resolve(directory, 'verification-input.wav'); writeFileSync(inputPath, input);
const left = new Float32Array(frames), right = new Float32Array(frames);
for (let i = 0; i < frames; i++) { left[i] = input.readInt16LE(44 + i * 4) / 32768; right[i] = input.readInt16LE(46 + i * 4) / 32768; }
const names = ['GREAT EYE', 'DIRTY SPACE', 'SHIMMER MAD', 'AMBIENTISER', 'GLITCHNOISE'];
const results = [];
for (const name of names) {
  const preset = bank.presets.find(p => p.name === name);
  if (!preset) throw new Error(`Missing original preset: ${name}`);
  const nativePath = resolve(directory, 'verification-native.wav');
  execFileSync(native, ['render', inputPath, nativePath, '--preset', name, '--tail', '0']);
  const expected = readFileSync(nativePath), output = new Float32Array(frames * 2);
  const engine = new AudioEngine(); const start = performance.now();
  engine.set_preset(new Float32Array(preset.values), 0);
  for (let offset = 0; offset < frames; offset += 128) {
    const n = Math.min(128, frames - offset);
    new Float32Array(wasm.memory.buffer, engine.input_left(), n).set(left.subarray(offset, offset + n));
    new Float32Array(wasm.memory.buffer, engine.input_right(), n).set(right.subarray(offset, offset + n));
    engine.process(n);
    const l = new Float32Array(wasm.memory.buffer, engine.output_left(), n), r = new Float32Array(wasm.memory.buffer, engine.output_right(), n);
    for (let i = 0; i < n; i++) { output[(offset + i) * 2] = l[i]; output[(offset + i) * 2 + 1] = r[i]; }
  }
  if (!output.every(Number.isFinite)) throw new Error(`Non-finite WASM audio: ${name}`);
  const actual = Buffer.from(encode_wav(output, 48000)); let maxDelta = 0, differentSamples = 0;
  if (actual.length !== expected.length) throw new Error(`Output length mismatch: ${name}`);
  for (let i = 44; i < actual.length; i += 2) { const delta = Math.abs(actual.readInt16LE(i) - expected.readInt16LE(i)); maxDelta = Math.max(maxDelta, delta); if (delta) differentSamples++; }
  if (maxDelta > 1) throw new Error(`Native/WASM mismatch in ${name}: ${maxDelta} PCM units`);
  results.push({ preset: name, frames, wasm_ms: Math.round(performance.now() - start), max_pcm_delta: maxDelta, different_samples: differentSamples });
  engine.free();
}
console.log(JSON.stringify({ status: 'PASS', sample_rate: 48000, results }, null, 2));
