let context, processor, initialization, wasmModule, mainInitialization;
let buffer, source, media, mediaNode, midiAccess;
let offset = 0, startedAt = 0, playing = false, looping = true;
let sourceName = '', recordChunks = [], recording = false, recordBarrier;
let snapshot = { values: null, host: { input_db: 0, output_db: 0, bypass: false }, sync: { enabled: false, bpm: 120 } };
const emit = (data) => window.dispatchEvent(new CustomEvent('erika-audio', { detail: JSON.stringify(data) }));
const errorText = (error) => String(error?.message || error);
const post = (data) => processor?.port.postMessage(data);

async function module() {
  if (!wasmModule) wasmModule = WebAssembly.compileStreaming(fetch('/pkg/engine/engine_bg.wasm')).catch(error => { wasmModule = undefined; throw error; });
  return wasmModule;
}
async function mainEngine() {
  if (!mainInitialization) mainInitialization = (async () => {
    const api = await import('/pkg/engine/engine.js');
    return { api, wasm: api.initSync({ module: await module() }) };
  })().catch(error => { mainInitialization = undefined; throw error; });
  return mainInitialization;
}
async function ensure() {
  if (initialization) return initialization;
  initialization = (async () => {
    emit({ type: 'status', message: 'Starting audio engine…', busy: true });
    context = new AudioContext({ sampleRate: 48000, latencyHint: 'interactive' });
    if (context.sampleRate !== 48000) throw new Error('This engine requires a 48 kHz audio context');
    await context.audioWorklet.addModule('/worklet.js');
    processor = new AudioWorkletNode(context, 'keklocator', { numberOfInputs: 1, numberOfOutputs: 1,
      outputChannelCount: [2], channelCount: 2, channelCountMode: 'explicit', processorOptions: { module: await module() } });
    await new Promise((resolve, reject) => {
      processor.port.onmessage = ({ data }) => {
        if (data.type === 'ready') resolve();
        else if (data.type === 'error') reject(new Error(data.message));
        else handleMessage(data);
      };
    });
    processor.port.onmessage = ({ data }) => handleMessage(data);
    processor.connect(context.destination);
    if (snapshot.values) processor.port.postMessage({ type: 'preset', values: snapshot.values, seconds: 0 });
    processor.port.postMessage({ type: 'host', ...snapshot.host });
    processor.port.postMessage({ type: 'sync', ...snapshot.sync });
    emit({ type: 'status', message: 'Audio engine ready', busy: false, sample_rate: context.sampleRate });
    return context;
  })().catch((error) => { initialization = undefined; processor?.disconnect(); processor = undefined; context?.close(); context = undefined; throw error; });
  return initialization;
}
function handleMessage(data) {
  if (data.type === 'record') { recordChunks.push(data.data); return; }
  if (data.type === 'record-ended') { recordBarrier?.resolve(); return; }
  if (data.type === 'error') { recordBarrier?.reject(new Error(data.message)); stopSource(); playing = false; recording = false; emit({ type: 'error', message: data.message, fatal: true }); return; }
  emit(data);
}
function stopSource(releaseMic = true) {
  if (source) { try { source.onended = null; source.stop(); source.disconnect(); } catch {} source = undefined; }
  if (releaseMic && media) { media.getTracks().forEach(t => t.stop()); media = undefined; mediaNode?.disconnect(); mediaNode = undefined; }
}
function startSource() {
  stopSource();
  source = context.createBufferSource(); source.buffer = buffer; source.loop = looping; source.connect(processor);
  source.start(0, offset % buffer.duration); startedAt = context.currentTime;
  source.onended = () => { if (!looping) { playing = false; offset = 0; emit({ type: 'playback', playing: false }); } };
  playing = true; emit({ type: 'playback', playing: true, name: sourceName, duration: buffer.duration });
}
export async function loadFile(file) {
  try {
    await ensure();
    const decoded = await context.decodeAudioData(await file.arrayBuffer());
    stopSource(); playing = false; buffer = decoded; offset = 0; sourceName = file.name;
    emit({ type: 'loaded', name: sourceName, duration: buffer.duration, playing: false });
  } catch (error) { emit({ type: 'error', message: errorText(error) }); }
}
export async function loadDemo(samples, rate) {
  try {
    await ensure(); await context.resume(); stopSource();
    buffer = context.createBuffer(2, samples.length / 2, rate);
    const l = buffer.getChannelData(0), r = buffer.getChannelData(1);
    for (let i = 0; i < l.length; i++) { l[i] = samples[i * 2]; r[i] = samples[i * 2 + 1]; }
    offset = 0; sourceName = 'Orbit sequence'; startSource();
    emit({ type: 'loaded', name: sourceName, duration: buffer.duration, playing: true });
  } catch (error) { emit({ type: 'error', message: errorText(error) }); }
}
export function command(text) {
  try {
    const data = JSON.parse(text);
    switch (data.type) {
      case 'parameter': if (snapshot.values) snapshot.values[data.id] = data.value; post(data); break;
      case 'preset': snapshot.values = [...data.values]; post(data); break;
      case 'host': snapshot.host = data; post(data); break;
      case 'sync': snapshot.sync = data; post(data); break;
      case 'clear': post(data); break;
      case 'loop': looping = data.enabled; if (source) source.loop = looping; break;
      case 'play': play().catch(error => emit({ type: 'error', message: errorText(error) })); break;
      case 'mic': microphone().catch(error => emit({ type: 'error', message: errorText(error) })); break;
      case 'record': record(data.enabled).catch(error => emit({ type: 'error', message: errorText(error) })); break;
      case 'render': render(data.tail || 0).catch(error => emit({ type: 'error', message: errorText(error) })); break;
      case 'midi': connectMidi().catch(error => emit({ type: 'error', message: errorText(error) })); break;
      case 'midi_out': if (midiAccess) for (const output of midiAccess.outputs.values()) output.send(data.data); break;
    }
  } catch (error) { emit({ type: 'error', message: errorText(error) }); }
}
async function play() {
  await ensure(); await context.resume();
  if (media) { stopSource(); playing = false; emit({ type: 'playback', playing: false }); return; }
  if (!buffer) throw new Error('Choose an audio file or load the demo');
  if (playing) { offset = (offset + context.currentTime - startedAt) % buffer.duration; stopSource(); playing = false; emit({ type: 'playback', playing: false }); }
  else startSource();
}
async function microphone() {
  await ensure(); await context.resume();
  if (media) { stopSource(); playing = false; emit({ type: 'playback', playing: false, name: sourceName }); return; }
  const stream = await navigator.mediaDevices.getUserMedia({ audio: { channelCount: 2, echoCancellation: false, noiseSuppression: false, autoGainControl: false } });
  stopSource(); media = stream; mediaNode = context.createMediaStreamSource(stream); mediaNode.connect(processor);
  playing = true; emit({ type: 'playback', playing: true, name: 'Live input', live: true });
}
async function record(enabled) {
  await ensure(); await context.resume();
  recording = enabled;
  if (enabled) {
    recordChunks = []; post({ type: 'record', enabled });
    emit({ type: 'recording', recording: true });
  } else {
    emit({ type: 'recording', recording: false, saving: true });
    // MessagePort preserves ordering: the acknowledgement follows the final samples.
    await new Promise((resolve, reject) => {
      const timer = setTimeout(() => { recordBarrier = undefined; reject(new Error('The audio engine did not finish recording')); }, 5000);
      recordBarrier = {
        resolve: () => { clearTimeout(timer); recordBarrier = undefined; resolve(); },
        reject: error => { clearTimeout(timer); recordBarrier = undefined; reject(error); }
      };
      post({ type: 'record', enabled: false });
    });
    const count = recordChunks.reduce((n, a) => n + a.length, 0);
    if (!count) throw new Error('The recording contains no audio yet');
    const samples = new Float32Array(count); let offset = 0;
    for (const chunk of recordChunks) { samples.set(chunk, offset); offset += chunk.length; }
    const { api } = await mainEngine(); download(api.encode_wav(samples, context.sampleRate), 'keklocator-recording.wav', 'audio/wav'); recordChunks = [];
    emit({ type: 'recording', recording: false, saving: false });
  }
}
async function render(tail) {
  if (!buffer) throw new Error('Choose an audio file or demo first');
  emit({ type: 'render', progress: 0, busy: true });
  const { api, wasm } = await mainEngine(); const engine = new api.AudioEngine();
  try {
    if (snapshot.values) engine.set_preset(new Float32Array(snapshot.values), 0);
    engine.set_host(snapshot.host.input_db, snapshot.host.output_db, snapshot.host.bypass);
    engine.set_sync(snapshot.sync.enabled, snapshot.sync.bpm);
    const total = buffer.length + Math.round(tail * 48000), output = new Float32Array(total * 2);
    const left = buffer.getChannelData(0), right = buffer.numberOfChannels > 1 ? buffer.getChannelData(1) : left;
    for (let offset = 0; offset < total; offset += 1024) {
      const n = Math.min(1024, total - offset);
      const il = new Float32Array(wasm.memory.buffer, engine.input_left(), 1024), ir = new Float32Array(wasm.memory.buffer, engine.input_right(), 1024);
      il.fill(0); ir.fill(0);
      if (offset < left.length) { il.set(left.subarray(offset, Math.min(offset + n, left.length))); ir.set(right.subarray(offset, Math.min(offset + n, right.length))); }
      engine.process(n);
      const ol = new Float32Array(wasm.memory.buffer, engine.output_left(), n), or = new Float32Array(wasm.memory.buffer, engine.output_right(), n);
      for (let i = 0; i < n; i++) { output[(offset + i) * 2] = ol[i]; output[(offset + i) * 2 + 1] = or[i]; }
      if (offset % 8192 === 0) { emit({ type: 'render', progress: offset / total, busy: true }); await new Promise(resolve => setTimeout(resolve, 0)); }
    }
    download(api.encode_wav(output, 48000), `${(sourceName || 'audio').replace(/\.[^.]+$/, '')}-keklocator.wav`, 'audio/wav');
    emit({ type: 'render', progress: 1, busy: false });
  } finally { engine.free(); }
}
async function connectMidi() {
  if (!navigator.requestMIDIAccess) throw new Error('Web MIDI is unavailable in this browser');
  midiAccess = await navigator.requestMIDIAccess({ sysex: false });
  const attach = () => {
    for (const input of midiAccess.inputs.values()) input.onmidimessage = (event) => window.dispatchEvent(new CustomEvent('erika-midi', { detail: JSON.stringify({ data: Array.from(event.data), time: event.timeStamp }) }));
    emit({ type: 'midi', count: midiAccess.inputs.size });
  };
  midiAccess.onstatechange = attach; attach();
}
export function midiSupported() { return typeof navigator.requestMIDIAccess === 'function'; }
export function download(bytes, name, type) {
  const blob = new Blob([bytes], { type }), url = URL.createObjectURL(blob), link = document.createElement('a');
  link.href = url; link.download = name; link.click(); setTimeout(() => URL.revokeObjectURL(url), 1000);
}
