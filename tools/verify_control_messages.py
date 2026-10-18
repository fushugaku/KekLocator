#!/usr/bin/env python3
"""Compare MIDI mappings and quarter-clock calculation with vendor ARM handlers."""
import json,math,struct,subprocess
from pathlib import Path
from reference_arm import OriginalArm,ROOT,ROM,STATE

def main():
    original=OriginalArm();original.initialize();original.use_firmware_controls();original.install_clock_source()
    mapping={}
    for offset in range(0x24028,0x24042,2):
        setting,index=ROM[offset:offset+2]
        mapping[original.u32(0x20001234+setting*32+4)]=index
    packets=[]
    for cc in mapping:
        for value in(0,1,63,64,126,127):packets.append(dict(data=[0xb0,cc,value],time=0))
    for note in range(128):packets.append(dict(data=[0x90,note,100],time=0))
    packets.append(dict(data=[0x90,60,0],time=0))
    packets.append(dict(data=[0xb1,70,127],time=0))
    packets.append(dict(data=[0xfa],time=0))
    for pulse in range(1,121):packets.append(dict(data=[0xf8],time=pulse*500/24+(.4 if pulse%7==0 else 0)))
    path=ROOT/'research/midi-reference-input.json';path.write_text(json.dumps(packets))
    actual=json.loads(subprocess.check_output([str(ROOT/'target/release/examples/midi_probe'),str(path)]))
    if len(actual)!=len(packets):raise ValueError('MIDI probe returned an incomplete packet list')
    differences=[];counts=dict(cc=0,notes=0,clock_quarters=0,ignored=0);max_clamp_delta=0.
    for packet,rust in zip(packets,actual):
        data=packet['data'];status=data[0];original.clock_ms=math.floor(packet['time']);expected=None
        if status in(0xfa,0xf8):
            original.invoke(0x60033744,[status])
            if status==0xf8 and original.cpu.mem_read(0x20004f04,1)[0]==0:
                period=struct.unpack('<f',original.cpu.mem_read(0x20004f10,4))[0]
                if period>0:expected=dict(tempo=60/period);counts['clock_quarters']+=1
        elif status in(0xb0,0x90):
            before=bytes(original.cpu.mem_read(0x20000088,20*72))
            original.cpu.mem_write(0x20270000,bytes(data+[0]));original.invoke(0x600286fc,[0x20270000]);original.invoke(0x322c)
            if status==0x90 and data[2]==0:
                if before!=bytes(original.cpu.mem_read(0x20000088,20*72)):raise ValueError('Velocity-zero Note changed original parameters')
                counts['ignored']+=1
            else:
                index=mapping[data[1]]if status==0xb0 else 3
                value=struct.unpack('<f',original.cpu.mem_read(0x200000a8+index*72,4))[0]
                if index==19:value=struct.unpack('<f',original.cpu.mem_read(STATE+0xbb0,4))[0]
                clamped=max(0,min(1,value));max_clamp_delta=max(max_clamp_delta,abs(value-clamped))
                expected=dict(parameter=index,value=clamped)
                counts['cc'if status==0xb0 else'notes']+=1
        else:counts['ignored']+=1
        match=(expected is None and rust is None)
        if expected is not None and rust is not None:
            match=expected.keys()==rust.keys()and all(struct.pack('<f',v)==struct.pack('<f',rust[k])if k in('value','tempo')else v==rust[k]for k,v in expected.items())
        if not match:differences.append(dict(packet=packet,original=expected,rust=rust))
    report=dict(status='PASS'if not differences else'DIFFERENCES',packets=len(packets),counts=counts,differences=differences,
        boundary='normalized browser controls; original MIDI handler 0x600286fc and clock handler 0x60033744; ADC pots detached; SysTick replaced by supplied integer milliseconds',
        normalization=dict(original_clamped_to_browser_range=[0,1],max_clamp_delta=max_clamp_delta,detail='VFMA gives a tiny negative Time at notes 84 and above; browser controls clamp to zero.'),
        host_only_checks=['Wrong-channel rejection belongs to the upstream MIDI router, outside the invoked original handler.'],
        limits='Does not emulate DIN electrical timing, MIDI OUT throttle, physical ADC pot quantization or out-of-range browser tempo policy. Custom CC collisions, Program Change and THRU are not covered.')
    (ROOT/'research/midi-reference.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps(report,indent=2));return 0 if not differences else 1

if __name__=='__main__':raise SystemExit(main())
