import './utf8.js';
import { initSync, DawEngine } from './pkg/engine/engine.js';
class PluginProcessor extends AudioWorkletProcessor {
  constructor({processorOptions:o}) {
    super(); this.frames=0;this.report=0;this.running=false;
    this.wasm=initSync({module:o.module});this.dsp=new DawEngine();
    for(let i=0;i<o.values.length;i++)this.dsp.set_parameter(i,o.values[i]);
    this.source=o.values[24]>=.5;this.locks=o.values[71]>=.5;this.extended=o.values[64]>=.5;this.running=this.source||(this.extended&&this.locks);
    this.port.onmessage=({data})=>{try{
      if(data.type==='parameter') {this.dsp.set_parameter(data.id,data.value);if(data.id===24||data.id===64||data.id===71){if(data.id===24)this.source=data.value>=.5;if(data.id===64)this.extended=data.value>=.5;if(data.id===71)this.locks=data.value>=.5;const running=this.source||(this.extended&&this.locks);if(!this.running&&running)this.frames=0;this.running=running;}}
      if(data.type==='preset')this.dsp.set_preset(new Float32Array(data.values),data.seconds||0);
    }catch(error){this.port.postMessage({type:'status',message:String(error)});}};
  }
  process(inputs,outputs){
    const out=outputs[0];if(!out?.length)return true;const n=out[0].length;
    try {
      const l=new Float32Array(this.wasm.memory.buffer,this.dsp.input_left(),n),r=new Float32Array(this.wasm.memory.buffer,this.dsp.input_right(),n);
      if(inputs[0]?.[0])l.set(inputs[0][0]);else l.fill(0);
      if(inputs[0]?.[1])r.set(inputs[0][1]);else r.set(l);
      this.dsp.process(n,120,this.frames/48000*2,this.running);
      out[0].set(new Float32Array(this.wasm.memory.buffer,this.dsp.output_left(),n));
      if(out[1])out[1].set(new Float32Array(this.wasm.memory.buffer,this.dsp.output_right(),n));
      if(this.running)this.frames+=n;this.report+=n;
      if(this.report>=2048){this.report=0;let left=0,right=0;for(let i=0;i<n;i++){left=Math.max(left,Math.abs(out[0][i]));right=Math.max(right,Math.abs(out[1]?.[i]||0));}
        this.port.postMessage({type:'transport',tempo:this.dsp.tempo(),step:this.dsp.step(),playing:this.running,left,right,duck:this.dsp.duck_reduction(),error:0});}
    }catch(error){for(const ch of out)ch.fill(0);this.port.postMessage({type:'status',message:String(error)});}
    return true;
  }
}
registerProcessor('keklocator-plugin',PluginProcessor);
