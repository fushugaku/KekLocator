// Exercise the shipped WASM API against the independent ARM reference renders.
// This is Node WASM execution, not browser automation. Keep all Q31 reference
// precision until the browser API's required Q31 -> f32 conversion.
import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { resolve, dirname } from 'node:path';
import { performance } from 'node:perf_hooks';
import { initSync, AudioEngine, DawEngine } from '../dist/pkg/engine/engine.js';

const root = fileURLToPath(new URL('..', import.meta.url));
const args=process.argv.slice(2), daw=args.includes('--daw');
const directory = resolve(root, args.find(a=>!a.startsWith('--')) ?? 'research/original-reference/controls');
const plan = JSON.parse(readFileSync(resolve(directory, 'plan.json'), 'utf8'));
const reference = JSON.parse(readFileSync(resolve(directory, 'report.json'), 'utf8'));
if (reference.status !== 'PASS' || reference.case_count !== plan.cases.length) {
  throw new Error('Run ./tools/verify-original.sh successfully before the WASM comparison');
}
const wasm = initSync({ module: readFileSync(resolve(root, 'dist/pkg/engine/engine_bg.wasm')) });
const results = [], started = performance.now();
for (const c of plan.cases) {
  const stamp = performance.now();
  const input = readFileSync(c.input);
  const expected = readFileSync(resolve(dirname(c.output), `${c.name}.original.q31`));
  if (input.length !== expected.length || input.length % 192 !== 0) {
    throw new Error(`Invalid Q31 reference length: ${c.name}`);
  }
  const frames = input.length / 8, blocks = frames / 24;
  const engine = daw ? new DawEngine() : new AudioEngine();
  if(daw) engine.set_parameter(21,0);
  let tempo=120;
  engine.set_preset(new Float32Array(c.values), 0);
  let differentSamples = 0, maxDelta = 0, firstDifference = null, samples = 0;
  // The stream adapter has 23 frames of fixed latency. The last block drains
  // those frames; compare their samples with the unshifted original DSP output.
  for (let block = 0; block <= blocks; block++) {
    for (const event of c.events ?? []) {
      if (event.block !== block || block === blocks) continue;
      if ('parameter' in event) engine.set_parameter(event.parameter, event.value);
      if ('sync' in event) {tempo=event.bpm??120;if(daw)engine.set_parameter(21,event.sync?1:0);else engine.set_sync(event.sync,tempo);}
      if ('preset' in event) engine.set_preset(new Float32Array(event.preset), event.seconds ?? 0);
    }
    const l = new Float32Array(wasm.memory.buffer, engine.input_left(), 24);
    const r = new Float32Array(wasm.memory.buffer, engine.input_right(), 24);
    for (let i = 0; i < 24; i++) {
      const offset = (block * 24 + i) * 8;
      l[i] = block < blocks ? input.readInt32LE(offset) / 2147483648 : 0;
      r[i] = block < blocks ? input.readInt32LE(offset + 4) / 2147483648 : 0;
    }
    if(daw) engine.process(24,tempo,block*24/48000*tempo/60,true);else engine.process(24);
    const out = [new Float32Array(wasm.memory.buffer, engine.output_left(), 24),
      new Float32Array(wasm.memory.buffer, engine.output_right(), 24)];
    for (let i = 0; i < 24; i++) {
      const frame = block * 24 + i - 23;
      if (frame < 0 || frame >= frames) continue;
      for (let channel = 0; channel < 2; channel++) {
        const value = Math.fround(expected.readInt32LE(frame * 8 + channel * 4) / 2147483648);
        const delta = Math.abs(out[channel][i] - value);
        samples++;
        if (!Number.isFinite(out[channel][i])) throw new Error(`Non-finite audio: ${c.name}`);
        if (delta !== 0) {
          differentSamples++;
          firstDifference ??= frame;
          maxDelta = Math.max(maxDelta, delta);
        }
      }
    }
  }
  engine.free();
  if (samples !== frames * 2) throw new Error(`Incomplete comparison: ${c.name}`);
  results.push({ name: c.name, samples, different_samples: differentSamples,
    max_f32_delta: maxDelta, first_difference_frame: firstDifference,
    wasm_ms: Math.round(performance.now() - stamp) });
  console.log(`${differentSamples === 0 ? 'PASS' : 'DIFF'} ${c.name} changed=${differentSamples} max=${maxDelta}`);
}
const report = {
  status: results.every(r => r.different_samples === 0) ? 'PASS' : 'DIFFERENCES',
  case_count: results.length,
  samples: results.reduce((n, r) => n + r.samples, 0),
  elapsed_seconds: Math.round((performance.now() - started) / 10) / 100,
  boundary: `Shipped WASM ${daw?'DawEngine (sequence disabled)':'AudioEngine'} streaming API; original Q31 converted to f32; 23-frame bridge latency compensated; no WAV rounding`,
  reference: 'Original ARM renders from verify_original.py --controls --all',
  results,
};
writeFileSync(resolve(directory, daw?'daw-wasm-report.json':'wasm-report.json'), JSON.stringify(report, null, 2) + '\n');
if (report.status !== 'PASS') process.exitCode = 1;
