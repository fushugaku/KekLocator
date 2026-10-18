/* Native format bridge; in a browser, preview the same Rust adapter in WASM. */
(() => {
  const stored = (key, legacy) => localStorage.getItem(key) ?? localStorage.getItem(legacy);
  const native = typeof globalThis.IPlugSendMsg === 'function';
  let ready = false, values, node, context, opening;
  const gestures = new Set();
  const emit = data => { if (ready) dispatchEvent(new CustomEvent('keklocator-plugin', { detail: JSON.stringify(data) })); };
  const snapshot = () => emit({ type: 'parameters', values });
  const notify = message => globalThis.IPlugSendMsg(message);
  const base64 = text => { const bytes=new TextEncoder().encode(text);let result="";for(let i=0;i<bytes.length;i+=16384)result+=String.fromCharCode(...bytes.subarray(i,i+16384));return btoa(result); };
  const param = (id, value) => {
    if (native) { if(!gestures.has(id)) notify({ msg:'BPCFUI', paramIdx:id }); notify({ msg:'SPVFUI', paramIdx:id, value }); if(!gestures.has(id)) notify({ msg:'EPCFUI', paramIdx:id }); }
    else { values[id] = value; localStorage.setItem('keklocator:plugin-preview:1', JSON.stringify(values)); ensure().then(() => node.port.postMessage({ type:'parameter', id, value })).catch(fail); }
  };
  const fail = error => emit({ type:'status', message:String(error?.message || error) });
  globalThis.SPVFD = (id, value) => { values ??= []; values[id] = value; if(ready) emit({ type:'parameter', id, value }); };
  globalThis.SAMFD = (_tag,_size,text) => { try { emit(JSON.parse(new TextDecoder().decode(Uint8Array.from(atob(text), c => c.charCodeAt(0))))); } catch (e) { fail(e); } };
  globalThis.SCVFD = globalThis.SCMFD = globalThis.SMMFD = globalThis.SSMFD = () => {};
  async function ensure() {
    if (opening) return opening;
    context = new AudioContext({ sampleRate:48000, latencyHint:'interactive' });
    opening = (async () => {
      if (context.sampleRate !== 48000) throw new Error('The audio context must run at 48 kHz');
      await context.resume();
      await context.audioWorklet.addModule('./plugin-worklet.js');
      const module = await WebAssembly.compile(await (await fetch('./pkg/engine/engine_bg.wasm')).arrayBuffer());
      node = new AudioWorkletNode(context,'keklocator-plugin',{ numberOfInputs:1, numberOfOutputs:1, outputChannelCount:[2], processorOptions:{ module, values } });
      node.port.onmessage = ({data}) => emit(data); node.connect(context.destination);
    })();
    return opening;
  }
  globalThis.keklocatorPlugin = { send(text) {
    try {
      const data = JSON.parse(text);
      if (data.type === 'ready') {
        ready = true;
        if (native) { notify({ msg:'SAMFUI', msgTag:101, ctrlTag:-1, data:'' }); }
        else {
          try { values = JSON.parse(stored('keklocator:plugin-preview:1','echolocator:plugin-preview:1')); } catch {}
          if (!Array.isArray(values) || ![64,data.values.length].includes(values.length) || !values.every(v=>Number.isFinite(v)&&v>=0&&v<=1)) values = data.values;
          values = data.values.map((v,i)=>values[i]??v);
          values[71] = 0;
          values[24] = 0; // A browser preview starts only after a user gesture.
          snapshot();
          try {const library=JSON.parse(stored('keklocator:library:1','echolocator:library:1'));if(library)emit({type:'library',library});} catch {}
          emit({type:'status',host:'Browser preview',message:'Internal source'});
        }
      } else if (data.type === 'gesture-start') { if(native&&!gestures.has(data.id))notify({msg:'BPCFUI',paramIdx:data.id});gestures.add(data.id); }
      else if (data.type === 'gesture-end') { if(native&&gestures.has(data.id))notify({msg:'EPCFUI',paramIdx:data.id});gestures.delete(data.id); }
      else if (data.type === 'library-save') {
        if(native)notify({msg:'SAMFUI',msgTag:102,ctrlTag:-1,data:base64(JSON.stringify(data.library))});
        else {localStorage.setItem('keklocator:library:1',JSON.stringify(data.library));emit({type:'status',message:'Library saved'});}
      } else if (data.type === 'parameter') param(data.id,data.value);
      else if (data.type === 'preset') {
        if (native) notify({ msg:'SAMFUI', msgTag:100, ctrlTag:-1, data:base64(text) });
        else { values.splice(0,20,...data.values); localStorage.setItem('keklocator:plugin-preview:1',JSON.stringify(values)); ensure().then(()=>node.port.postMessage(data)).catch(fail); }
      } else if (data.type === 'save') {
        const blob=new Blob([JSON.stringify({format:'keklocator-plugin/2',parameters:data.values,library:data.library},null,2)],{type:'application/json'});
        const url=URL.createObjectURL(blob),a=document.createElement('a');a.href=url;a.download='keklocator-plugin.json';a.click();setTimeout(()=>URL.revokeObjectURL(url),1000);
      }
    } catch(error) { fail(error); }
  }};
  (async () => {
    try { const bytes=Uint8Array.from(atob(globalThis.keklocatorUiBytes),c=>c.charCodeAt(0)); await wasm_bindgen({module_or_path:bytes}); wasm_bindgen.mount_plugin(); }
    catch(error) { document.querySelector('#app').textContent=String(error?.message||error); console.error(error); }
  })();
})();
