#!/usr/bin/env python3
"""Inventory is distinct from functional coverage. No emulator semantics here."""
import collections,hashlib,json,re,struct
from pathlib import Path
ROOT=Path(__file__).resolve().parent.parent
ROM=(ROOT/'crates/engine/firmware/echolocator-1.05.bin').read_bytes()
EXPECTED='9cea9f9a998ac7bf05297821290d36ef2843ad72b10f0da583765cc914c58211'

def main():
    assert hashlib.sha256(ROM).hexdigest()==EXPECTED
    module=(ROOT/'crates/engine/src/generated/mod.rs').read_text()
    functions={int(s,16)for s in re.findall(r',\s*0x([0-9a-f]+)\)',module)}
    instructions=collections.Counter()
    traps=[]
    for path in (ROOT/'crates/engine/src/generated').glob('page_*.rs'):
        source=path.read_text();instructions.update(int(a,16)for a in re.findall(r'// 0x([0-9a-f]+):',source))
        traps.extend(re.findall(r'hardware trap: ([^"]+)',source))
    pcode=ROOT/'research/pcode.json';inventory=None
    if pcode.is_file():
        exported=json.loads(pcode.read_text());expected_entries={f['entry']for f in exported['functions']};expected_instructions={i['address']for i in exported['instructions']}
        inventory=dict(function_entries=len(functions),instructions=len(instructions),missing_function_entries=[hex(a)for a in expected_entries-functions],missing_instructions=[hex(a)for a in expected_instructions-instructions.keys()],duplicate_instructions=sum(v-1 for v in instructions.values()))
        assert not inventory['missing_function_entries']and not inventory['missing_instructions']and inventory['duplicate_instructions']==0
    params=[]
    for i in range(20):
        row=ROM[0x37398+0x88+i*72:0x37398+0x88+(i+1)*72]
        mapper,callback=struct.unpack_from('<2I',row,60)
        params.append(dict(id=i,name=row[:24].split(b'\0')[0].decode(),mapper=hex(mapper),callback=hex(callback&~1),callback_translated=(callback&~1)in instructions,randomized_by_firmware=bool(row[71])))
    assert all(p['callback_translated']for p in params)
    uf2=ROOT.parent/'echolocator-1.05-reverse/firmware/echolocator-1.05.uf2';source_match=None
    if uf2.is_file():
        raw=uf2.read_bytes();seen={};covered=bytearray(len(ROM))
        for offset in range(0,len(raw),512):
            block=raw[offset:offset+512];a,b,flags,address,length,number,count,family=struct.unpack_from('<8I',block)
            assert a==0x0a324655 and b==0x9e5d5157 and struct.unpack_from('<I',block,508)[0]==0x0ab16f30
            assert number not in seen and count==len(raw)//512
            seen[number]=address;start=address-0x60020000
            assert 0<=start and start+length<=len(ROM)
            assert block[32:32+length]==ROM[start:start+length]
            covered[start:start+length]=b'\1'*length
        assert all(covered)
        source_match=dict(uf2_sha256=hashlib.sha256(raw).hexdigest(),blocks=len(seen),payload_matches_rust_rom=True,all_payload_bytes_checked=len(ROM))
    report=dict(status='INCOMPLETE',static_translation_status='PASS',firmware_sha256=EXPECTED,uf2_source=source_match,static_inventory=inventory,parameters=params,
        hardware_traps=dict(collections.Counter(traps)),
        warning='All recognized instructions are translated; this is not proof that every firmware function is callable or that physical peripherals are emulated.',
        product_gaps=[
            dict(feature='Original MAGIC parameter randomizer',status='different',detail='Host xorshift and bounded values replace newlib rand and firmware randomizable flags. Modes 13/14/17 are not randomized.'),
            dict(feature='MAGIC patch names',status='missing'),
            dict(feature='Single-preset .elp import/export',status='missing',detail='Only JSON/ELB bank files are exposed.'),
            dict(feature='Factory reset command',status='missing',detail='Clear effect buffers is a different operation.'),
            dict(feature='ADC/DAC and analogue VCA signal path',status='not modeled',detail='The existing firmware digital mix route is used.'),
            dict(feature='Physical Input/Output gain ranges',status='different',detail='Web gains -24..+6 dB and -12..0 dB differ from physical gain/volume controls.'),
            dict(feature='OLED/LED brightness, screensaver, pot catch',status='hardware replaced'),
            dict(feature='Physical footswitch, DIN MIDI timing and MIDI OUT throttling',status='adapted; not hardware verified'),
            dict(feature='USB mass storage and UF2 bootloader',status='not executed'),
            dict(feature='Duplicate remapped MIDI CC numbers',status='different',detail='Firmware loops over all matching mappings; the host MIDI parser takes the first matching CC.'),
            dict(feature='Tap gestures and extended MIDI routing/settings',status='not differentially verified',detail='Host tap averages up to four intervals. Program Change, THRU, custom mappings and pedal gestures are implemented but not compared with the original complete control state machine.'),
        ])
    path=ROOT/'research/functional-audit.json';path.write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps(dict(status=report['status'],static_translation_status=report['static_translation_status'],inventory=inventory,uf2=source_match,known_product_gaps=len(report['product_gaps'])),indent=2))

if __name__=='__main__':main()
