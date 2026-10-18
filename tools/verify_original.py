#!/usr/bin/env python3
"""Differential audio test: unchanged vendor machine code versus translated Rust.

The golden path imports neither generated Rust nor Ghidra P-code. Comparison is
at the original 24-frame, signed Q31 stereo boundary, before WAV quantization.
"""
import argparse,array,hashlib,json,math,struct,subprocess,time,zlib
from pathlib import Path
from reference_arm import OriginalArm,ROOT,ROM,STATE

OUT=ROOT/'research/original-reference'
F32=lambda v:struct.unpack('<f',struct.pack('<f',v))[0]

def factory():
    data=ROM[0x37398:0x392f8]
    defaults=[]
    for i in range(20):
        low,high,initial=struct.unpack_from('<3f',data,0x88+i*72+24)
        defaults.append(F32(F32(initial-low)/F32(high-low)))
    raw=(ROOT/'crates/engine/firmware/default-backup.elb').read_bytes()
    magic,version,count,crc=struct.unpack_from('<4I',raw)
    assert magic==0x1a13bfef and version in (1,2,3)
    assert zlib.crc32(raw[16:],0xffffffff)==crc
    stride=50 if version==1 else 52
    assert len(raw)==16+count*stride
    result=[]
    for n in range(count):
        row=raw[16+n*stride:16+(n+1)*stride]
        values=defaults.copy()
        for i,(value,) in enumerate(struct.iter_unpack('<H',row[16:])):values[i]=F32(value/4095)
        if version<=2: values[15]=0;values[16]=F32(2047/4095)
        if version==2: values[17]=0
        values[19]=0
        result.append((row[:16].split(b'\0')[0].decode().strip(),values))
    return defaults,result

def source(frames,kind='mixed'):
    out=array.array('i');rng=0x12345678
    for i in range(frames):
        t=i/48000
        if kind=='silence': l=r=0
        elif kind=='impulse': l,r=(0.5,-0.25)if i==0 else(0,0)
        elif kind=='clip': l,r=(0.999999,-0.999999)if i<4800 else(0,0)
        elif i<33600:
            rng=(1664525*rng+1013904223)&0xffffffff
            noise=(rng/2147483648-1)*0.035
            envelope=math.exp(-(i%6000)/1800)
            l=0.12*math.sin(2*math.pi*(110*t+1700*t*t))*envelope+noise
            r=0.10*math.sin(2*math.pi*(277.18*t+530*t*t))*envelope-noise
            if i==0:l+=0.4;r-=0.25
        else:l=r=0
        # 24-bit ADC-aligned Q31 samples, exactly representable as binary32 / 2^31.
        out.extend((int(max(-1,min(0.999999,l))*8388608)<<8,int(max(-1,min(0.999999,r))*8388608)<<8))
    return out.tobytes()

def cases(all_cases):
    defaults,bank=factory();great=next(v for n,v in bank if n=='GREAT EYE')
    result=[dict(name='factory-19-GREAT-EYE',values=great,frames=96000)]
    if not all_cases:return result
    result=[]
    for i,(name,values) in enumerate(bank):result.append(dict(name=f'factory-{i+1:02}-{name.replace(" ","-")}',values=values,frames=96000))
    for index in range(20):
        for value in (0.0,0.5,1.0):
            result.append(dict(name=f'parameter-{index:02}-{value:g}',values=great,frames=96000,events=[dict(block=0,parameter=index,value=value)]))
    for dirty in(0,1):
        for routing in(0,1):
            for blend in(0,1):
                for frequency in(0,1):
                    for dub in(0,1):
                        v=great.copy()
                        for i,x in [(0,dirty),(14,routing),(13,blend),(17,frequency),(15,dub),(2,.55),(7,.73),(8,.8),(9,.65),(18,.85)]:v[i]=F32(x)
                        result.append(dict(name=f'modes-{dirty}{routing}{blend}{frequency}{dub}',values=v,frames=96000))
    for kind in('silence','impulse','clip'):
        result.append(dict(name=f'source-{kind}',values=great,frames=240000,kind=kind))
    for ratio in range(17):
        v=great.copy();v[3]=F32((ratio+.5)/16.99)
        result.append(dict(name=f'sync-{ratio:02}',values=v,frames=96000,events=[dict(block=0,sync=True,bpm=123.0)]))
    result.append(dict(name='freeze-on-off',values=great,frames=240000,events=[dict(block=500,parameter=19,value=1),dict(block=5000,parameter=19,value=0)]))
    result.append(dict(name='parameter-sweep',values=great,frames=240000,events=[dict(block=50+i*110,parameter=i%20,value=F32((i%5)/4))for i in range(80)]))
    result.append(dict(name='long-tail',values=great,frames=960000,kind='impulse'))
    target=next(v.copy()for n,v in bank if n=='SHIMMER MAD')
    for i in(0,13,14,15,17):target[i]=1-great[i]
    target[18]=F32(.8)
    for seconds in(.1,1.,10.):
        result.append(dict(name=f'morph-{seconds:g}s',values=great,frames=int((seconds+1)*48000),events=[dict(block=300,preset=target,seconds=seconds)]))
    result.append(dict(name='morph-retarget',values=great,frames=144000,events=[dict(block=200,preset=target,seconds=1),dict(block=800,preset=great,seconds=1)]))
    result.append(dict(name='morph-freeze',values=great,frames=144000,events=[dict(block=200,preset=target,seconds=1),dict(block=700,parameter=19,value=1),dict(block=2200,parameter=19,value=0)]))
    # Long delays must actually reach their first echoes. Also exercise controls
    # with their dependent effect enabled rather than comparing an inactive mix.
    v=great.copy();v[3]=1.
    result.append(dict(name='extended-time-max',values=v,frames=480000,kind='impulse'))
    for ratio in(15,16):
        v=great.copy();v[3]=F32((ratio+.5)/16.99)
        result.append(dict(name=f'extended-sync-{ratio}',values=v,frames=480000,events=[dict(block=0,sync=True,bpm=123.)]))
    for style in(0,1):
        for blend in(0,1):
            for pitch in(0,1):
                v=great.copy()
                for i,x in[(17,style),(13,blend),(7,pitch),(8,.8),(18,.85)]:v[i]=F32(x)
                result.append(dict(name=f'extended-pitch-{style}{blend}{pitch}',values=v,frames=192000))
    for mix in(0,.5,1):
        v=great.copy();v[15]=1.;v[16]=mix
        result.append(dict(name=f'extended-dub-mix-{mix:g}',values=v,frames=192000))
    v=great.copy();v[9]=1.
    result.append(dict(name='extended-feedback-max',values=v,frames=960000,kind='impulse'))
    result.append(dict(name='extended-tempo-changes',values=great,frames=384000,events=[dict(block=0,sync=True,bpm=100.),dict(block=4000,sync=True,bpm=160.),dict(block=8000,sync=False,bpm=160.)]))
    return result

def sync(original,enabled,bpm):
    original.cpu.mem_write(0x20001234+27*32+4,bytes([int(enabled)]))
    original.write_f32(0x20004f10,F32(F32(60)/F32(bpm)))
    original.cpu.mem_write(0x20001e70,b'\xff')
    if original.firmware_controls:original.write_f32(0x200000b0+3*72,-1.)
    original.parameter(3,struct.unpack('<f',original.cpu.mem_read(0x200000a8+3*72,4))[0])

def metrics(expected,actual):
    a=array.array('i');a.frombytes(expected);b=array.array('i');b.frombytes(actual)
    assert len(a)==len(b)
    changed=0;max_delta=0;sum_error=0;sum_signal=0;first=None
    for i,(x,y) in enumerate(zip(a,b)):
        d=abs(x-y)
        if d:changed+=1;first=i if first is None else first
        max_delta=max(max_delta,d);sum_error+=d*d;sum_signal+=x*x
    return dict(samples=len(a),different_samples=changed,max_q31_delta=max_delta,
                rms_error=math.sqrt(sum_error/max(1,len(a)))/2147483648,
                error_dbfs=10*math.log10(sum_error/len(a)/2**62)if sum_error else None,
                snr_db=10*math.log10(sum_signal/sum_error)if sum_error and sum_signal else None,
                first_difference_frame=first//2 if first is not None else None,
                original_sha256=hashlib.sha256(expected).hexdigest(),rust_sha256=hashlib.sha256(actual).hexdigest())

def main():
    global OUT
    parser=argparse.ArgumentParser();parser.add_argument('--all',action='store_true');parser.add_argument('--case');parser.add_argument('--frames',type=int);parser.add_argument('--dump',action='store_true');parser.add_argument('--controls',action='store_true');parser.add_argument('--output',type=Path);args=parser.parse_args()
    if args.output:OUT=args.output.resolve()
    if args.controls:OUT=OUT/'controls'
    OUT.mkdir(parents=True,exist_ok=True)
    selected=cases(args.all)
    if args.case:selected=[c for c in selected if args.case in c['name']]
    if not selected:raise ValueError('No matching cases')
    for c in selected:
        if args.frames:c['frames']=args.frames
        path=OUT/f"input-{c['frames']}-{c.get('kind','mixed')}.q31"
        if not path.exists():path.write_bytes(source(c['frames'],c.get('kind','mixed')))
        c['input']=str(path);c['output']=str(OUT/f"{c['name']}.rust.q31")
        if args.dump:c['dump']=str(OUT/f"{c['name']}.rust")
    plan=OUT/'plan.json';plan.write_text(json.dumps(dict(cases=selected)))
    native=ROOT/'target/release/examples/reference_driver'
    subprocess.run([str(native),str(plan)],check=True,stdout=subprocess.DEVNULL,stderr=(OUT/'rust.log').open('w'))
    results=[];started=time.perf_counter()
    for case in selected:
        stamp=time.perf_counter();original=OriginalArm();original.initialize()
        if args.controls:original.use_firmware_controls()
        for i,value in enumerate(case['values']):original.parameter(i,0 if i==19 else value)
        events={}
        for event in case.get('events',[]):events.setdefault(event['block'],[]).append(event)
        data=Path(case['input']).read_bytes();render=bytearray()
        for block,offset in enumerate(range(0,len(data),192)):
            for event in events.get(block,[]):
                if 'parameter'in event:original.parameter(event['parameter'],event['value'])
                if 'sync'in event:sync(original,event['sync'],event.get('bpm',120))
                if 'preset'in event:original.begin_morph(event['preset'],event.get('seconds',0))
            render.extend(original.block(data[offset:offset+192]))
        expected=bytes(render);actual=Path(case['output']).read_bytes()
        (OUT/f"{case['name']}.original.q31").write_bytes(expected)
        if args.dump:
            (OUT/f"{case['name']}.original.dtcm").write_bytes(original.cpu.mem_read(0x20000000,0x20000))
            (OUT/f"{case['name']}.original.heap").write_bytes(original.cpu.mem_read(0x80000000,0x2000000))
        m=metrics(expected,actual);m.update(name=case['name'],seconds=case['frames']/48000,reference_seconds=round(time.perf_counter()-stamp,3));results.append(m)
        print(('PASS'if m['different_samples']==0 else'DIFF'),case['name'],f"changed={m['different_samples']} max={m['max_q31_delta']} first={m['first_difference_frame']} time={m['reference_seconds']}s",flush=True)
    report=dict(status='PASS'if all(r['different_samples']==0 for r in results)else'DIFFERENCES',
        oracle='Unicorn 2.1.4, UC_CPU_ARM_MAX, unchanged vendor Thumb-2 + FPv5 machine code',
        firmware_sha256=hashlib.sha256(ROM).hexdigest(),boundary='stereo signed Q31, 24 frames, 48 kHz, digital mixer, before host float/WAV conversion',
        runtime_hooks=['malloc (zeroed private heap)','free (lifetime ends with engine)','external analogue VCA DAC (unused digital route)'],
        caveats=['Not a physical board test; Cortex-M7 Unicorn model lacks firmware f64 support, so ARM MAX is used.','No GPIO/DMA/OLED/USB boot or ADC/DAC/analogue path model.'],
        parameter_path='Original normalized setter 0x6002a7e0 and control tick 0x322c, ADC detached'if args.controls else'Host physical mapping and original parameter callbacks',
        case_count=len(results),elapsed_seconds=round(time.perf_counter()-started,2),results=results)
    (OUT/'report.json').write_text(json.dumps(report,indent=2)+'\n')
    return 0 if report['status']=='PASS'else 1

if __name__=='__main__':raise SystemExit(main())
