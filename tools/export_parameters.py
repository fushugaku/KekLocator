#!/usr/bin/env python3
import struct
from pathlib import Path
root=Path(__file__).resolve().parent.parent
rom=(root/'crates/engine/firmware/echolocator-1.05.bin').read_bytes()
data=rom[0x37398:0x392f8]
keys=['dirty','orbit','reverb','time','cutoff','mod_speed','mod_depth','pitch','shimmer','feedback','resonance','ping_pong','compression','shimmer_mode','filter_mode','dub','dub_mix','shimmer_style','mix','freeze']
cc={3:70,10:71,4:72,5:73,6:74,2:75,1:76,18:77,9:78,7:79,8:80,11:81,19:82}
lines=['// @generated from the original DTCM parameter table.','use super::ParamDef;','pub const PARAMETERS: [ParamDef;20] = [']
for i,key in enumerate(keys):
 off=0x88+i*72
 name=data[off:off+24].split(b'\0')[0].decode()
 lo,hi,initial=struct.unpack_from('<3f',data,off+24)
 fmt=lambda x:repr(x)+'f32'
 lines.append(f'ParamDef {{ id:{i}, key:"{key}", label:"{name}", min:{fmt(lo)}, max:{fmt(hi)}, initial:{fmt(initial)}, toggle:{str(i in [0,13,14,15,17,19]).lower()}, cc:{f"Some({cc[i]})" if i in cc else "None"} }},')
lines.append('];')
(root/'crates/engine/src/parameter_table.rs').write_text('\n'.join(lines)+'\n')
