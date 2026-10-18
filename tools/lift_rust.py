#!/usr/bin/env python3
"""Statically lower all exported firmware P-code to Rust, with explicit FPv5 fixes."""
import collections
import hashlib
import json
import re
import struct
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "crates/engine/src/generated"
ROM = (ROOT / "crates/engine/firmware/echolocator-1.05.bin").read_bytes()
DATA = json.loads((ROOT / "research/pcode.json").read_text())
REGS = {r["name"]: r for r in DATA["registers"]}
INSNS = {i["address"]: i for i in DATA["instructions"]}
M64 = (1 << 64)-1


def nmask(size): return (1 << (size*8))-1
def const(n): return f"{n & M64}u64"
def read_reg(name):
    reg=REGS[name];return f"m.reg({reg['offset']}, {reg['size']})"
def write_reg(name, value):
    reg=REGS[name];return f"m.set_reg({reg['offset']}, {reg['size']}, {value});"
def condition(name):
    n,z,c,v=(read_reg(x) for x in ("NG","ZR","CY","OV"))
    return {"eq":f"{z} != 0","ne":f"{z} == 0","cs":f"{c} != 0","hs":f"{c} != 0",
        "cc":f"{c} == 0","lo":f"{c} == 0","mi":f"{n} != 0","pl":f"{n} == 0",
        "vs":f"{v} != 0","vc":f"{v} == 0","hi":f"{c} != 0 && {z} == 0",
        "ls":f"{c} == 0 || {z} != 0","ge":f"{n} == {v}","lt":f"{n} != {v}",
        "gt":f"{z} == 0 && {n} == {v}","le":f"{z} != 0 || {n} != {v}"}[name]


def specialized(insn):
    text=insn["assembly"]
    if any(o.get("userop")=="coprocessor_function2" for o in insn["pcode"]):
        address=insn["address"]
        source=ROM[address-0x60020000:address-0x60020000+4] if address>=0x60020000 else ROM[0x392f8+address:0x392fc+address]
        decoded=subprocess.run(["cstool","thumbv8",source.hex(),f"{address:x}"],capture_output=True,text=True,check=True).stdout
        match=re.search(r"vsel(eq|vs|ge|gt)\.f(32|64)\s+([sd]\d+), ([sd]\d+), ([sd]\d+)",decoded)
        if not match: raise ValueError((address,text,decoded))
        cond,size,d,n,s=match.groups()
        expression=f"if {condition(cond)} {{ {read_reg(n)} }} else {{ {read_reg(s)} }}"
        return [write_reg(d,expression)],"VSEL"
    match=re.match(r"^(vfma|vfms|vfnma|vfnms)([a-z]*)\.f(32|64)\s+([sd]\d+),([sd]\d+),([sd]\d+)$",text.replace(" ","",1))
    # Ghidra separates mnemonic and operands by a space, operand commas have none.
    if not match: match=re.match(r"^(vfma|vfms|vfnma|vfnms)([a-z]*)\.f(32|64)\s+([sd]\d+),\s*([sd]\d+),\s*([sd]\d+)$",text)
    if match:
        op,cond,size,d,n,s=match.groups(); fp="f32" if size=="32" else "f64"; integer="u32" if size=="32" else "u64"
        val=lambda r:f"{fp}::from_bits({read_reg(r)} as {integer})"
        lhs=val(n);rhs=val(s);add=val(d)
        if op in ("vfms","vfnma"):lhs=f"(-{lhs})"
        if op in ("vfnma","vfnms"):add=f"(-{add})"
        statement=write_reg(d,f"({lhs}).mul_add({rhs}, {add}).to_bits() as u64")
        return ([f"if {condition(cond)} {{ {statement} }}"] if cond else [statement]),"FMA"
    return None,None


class Instruction:
    def __init__(self, insn):
        self.i=insn; self.groups=[]
        intervals=sorted({(n['offset'],n['offset']+n['size']) for op in insn['pcode'] for n in [op.get('out'),*op['in']] if n and n['space']=='unique'})
        for lo,hi in intervals:
            if self.groups and lo<self.groups[-1][1]: self.groups[-1]=(self.groups[-1][0],max(hi,self.groups[-1][1]))
            else:self.groups.append((lo,hi))
        for lo,hi in self.groups:assert hi-lo<=16,(lo,hi)
    def group(self,n):
        for idx,(lo,hi) in enumerate(self.groups):
            if lo<=n['offset'] and n['offset']+n['size']<=hi:return idx,lo,hi
        raise ValueError(n)
    def read(self,n):
        sp,off,size=n['space'],n['offset'],n['size']
        assert size in (1,2,4,8)
        if sp=='const':return const(off&nmask(size))
        if sp=='register':
            assert 0<=off and off+size<=2048
            return f"m.reg({off}, {size})"
        if sp=='ram':return f"m.load(0x{off:x}, {size})?"
        if sp=='unique':
            idx,lo,hi=self.group(n);return f"((u{idx} >> {(off-lo)*8}) as u64 & {const(nmask(size))})"
        raise ValueError(sp)
    def write(self,n,value):
        off,size,sp=n['offset'],n['size'],n['space']
        value=f"({value}) & {const(nmask(size))}" if size<8 else value
        if sp=='register':return f"m.set_reg({off}, {size}, {value});"
        if sp=='ram':return f"m.store(0x{off:x}, {size}, {value})?;"
        if sp=='unique':
            idx,lo,hi=self.group(n); shift=(off-lo)*8;typ='u64' if hi-lo<=8 else 'u128'
            if lo==off and hi-lo==size:return f"u{idx} = ({value}) as {typ};"
            bits=nmask(size)<<shift
            return f"u{idx} = (u{idx} & !{bits}{typ}) | ((({value}) as {typ}) << {shift});"
        raise ValueError(n)
    def expr(self,op):
        code=op['op']; a=[self.read(x) for x in op['in']]; s=[n['size'] for n in op['in']]
        size=op.get('out',{}).get('size',0)
        if code in ('COPY','INT_ZEXT'):return a[0]
        if code=='INT_SEXT':return f"signed({a[0]}, {s[0]}) as u64"
        if code=='INT_ADD':return f"({a[0]}).wrapping_add({a[1]})"
        if code=='INT_SUB':return f"({a[0]}).wrapping_sub({a[1]})"
        if code=='INT_MULT':return f"({a[0]}).wrapping_mul({a[1]})"
        if code=='INT_2COMP':return f"({a[0]}).wrapping_neg()"
        if code=='INT_NEGATE':return f"!({a[0]})"
        if code in ('INT_AND','INT_OR','INT_XOR'):return f"({a[0]}) {dict(INT_AND='&',INT_OR='|',INT_XOR='^')[code]} ({a[1]})"
        if code in ('BOOL_AND','BOOL_OR','BOOL_XOR'):return f"(({a[0]} != 0) {dict(BOOL_AND='&',BOOL_OR='|',BOOL_XOR='^')[code]} ({a[1]} != 0)) as u64"
        if code=='BOOL_NEGATE':return f"({a[0]} == 0) as u64"
        if code in ('INT_EQUAL','INT_NOTEQUAL','INT_LESS','INT_LESSEQUAL'):
            return f"(({a[0]}) {dict(INT_EQUAL='==',INT_NOTEQUAL='!=',INT_LESS='<',INT_LESSEQUAL='<=')[code]} ({a[1]})) as u64"
        if code in ('INT_SLESS','INT_SLESSEQUAL'):return f"(signed({a[0]}, {s[0]}) {'<' if code=='INT_SLESS' else '<='} signed({a[1]}, {s[1]})) as u64"
        if code in ('INT_CARRY','INT_SCARRY','INT_SBORROW'):return f"{dict(INT_CARRY='carry',INT_SCARRY='scarry',INT_SBORROW='sborrow')[code]}({a[0]}, {a[1]}, {s[0]})"
        if code in ('INT_LEFT','INT_RIGHT'):return f"{'shl' if code=='INT_LEFT' else 'shr'}({a[0]}, {a[1]})"
        if code=='INT_SRIGHT':return f"sar({a[0]}, {a[1]}, {s[0]})"
        if code=='INT_DIV':return f"udiv({a[0]}, {a[1]})"
        if code=='INT_SDIV':return f"sdiv({a[0]}, {a[1]}, {s[0]})"
        if code=='SUBPIECE':return f"shr({a[0]}, ({a[1]}) * 8)"
        if code=='LZCOUNT':return f"(({a[0]}).leading_zeros() - {64-s[0]*8}) as u64"
        if code=='LOAD':return f"m.load(({a[1]}) as u32, {size})?"
        fp=lambda ix:f"{'f32' if s[ix]==4 else 'f64'}::from_bits(({a[ix]}) as {'u32' if s[ix]==4 else 'u64'})"
        if code.startswith('FLOAT_'):
            if code in ('FLOAT_ADD','FLOAT_SUB','FLOAT_MULT','FLOAT_DIV'):
                return f"({fp(0)} {dict(FLOAT_ADD='+',FLOAT_SUB='-',FLOAT_MULT='*',FLOAT_DIV='/')[code]} {fp(1)}).to_bits() as u64"
            if code in ('FLOAT_EQUAL','FLOAT_NOTEQUAL','FLOAT_LESS','FLOAT_LESSEQUAL'):
                return f"({fp(0)} {dict(FLOAT_EQUAL='==',FLOAT_NOTEQUAL='!=',FLOAT_LESS='<',FLOAT_LESSEQUAL='<=')[code]} {fp(1)}) as u64"
            if code=='FLOAT_NAN':return f"{fp(0)}.is_nan() as u64"
            if code=='FLOAT_NEG':return f"({a[0]}) ^ {const(1<<(s[0]*8-1))}"
            if code=='FLOAT_SQRT':return f"{fp(0)}.sqrt().to_bits() as u64"
        if code=='FLOAT2FLOAT':return f"({fp(0)} as {'f32' if size==4 else 'f64'}).to_bits() as u64"
        if code=='TRUNC':return f"({fp(0)} as {'i32' if size==4 else 'i64'}) as u64"
        if code=='CALLOTHER':
            name=op['userop']
            if name=='VectorSignedToFloat':return f"(signed({a[1]}, {s[1]}) as {'f32' if size==4 else 'f64'}).to_bits() as u64"
            if name=='VectorUnsignedToFloat':return f"({a[1]} as {'f32' if size==4 else 'f64'}).to_bits() as u64"
            if name=='VectorFloatToSignedFixed':return f"float_to_fixed({a[1]}, ({a[2]}) as usize, ({a[3]}) as u32)"
            if name=='hasExclusiveAccess':return f"(m.exclusive == ({a[1]}) as u32) as u64"
            if name=='isCurrentModePrivileged':return '1u64'
            if name=='isIRQinterruptsEnabled':return 'm.irq_enabled as u64'
        raise ValueError((hex(self.i['address']),code,op))
    def code(self,op,index,count,micro):
        code=op['op'];values=op['in']
        if code=='STORE':return [f"m.store(({self.read(values[1])}) as u32, {values[2]['size']}, {self.read(values[2])})?;"]
        if code in ('BRANCH','CALL','CBRANCH','RETURN','BRANCHIND','CALLIND'):
            first=values[0]
            if first['space']=='const' and code in ('BRANCH','CBRANCH'):
                relative=first['offset'];bits=first['size']*8
                if relative&(1<<(bits-1)):relative-=1<<bits
                target=index+relative
                assert 0<=target<=count,(index,target,count)
                action=f"ip = {target}; continue;"
            else:
                target=f"0x{first['offset']:x}u32" if first['space']=='ram' and code in ('BRANCH','CALL','CBRANCH') else f"({self.read(first)}) as u32"
                action=f"return Ok(({target}) & !1);"
            if code=='CBRANCH':return [f"if {self.read(values[1])} != 0 {{ {action} }}"]
            return [action]
        if code=='CALLOTHER' and not op.get('out'):
            name=op['userop']
            if name in ('setISAMode','DataSynchronizationBarrier','InstructionSynchronizationBarrier','DataMemoryBarrier'):return []
            if name=='enableIRQinterrupts':return ['m.irq_enabled = true;']
            if name=='disableIRQinterrupts':return ['m.irq_enabled = false;']
            if name=='ExclusiveAccess':return [f"m.exclusive = ({self.read(values[1])}) as u32;"]
            if name=='setMainStackPointer':return [write_reg('sp',self.read(values[1]))]
            if name in ('software_bkpt','coprocessor_store'):return [f'return Err(m.fault("hardware trap: {name}"));']
        return [self.write(op['out'],self.expr(op))]
    def emit(self):
        specific,kind=specialized(self.i)
        if specific is not None:return ['{',*specific,'}'],kind
        lines=['{']
        for idx,(lo,hi) in enumerate(self.groups):lines.append(f"let mut u{idx} = 0{'u64' if hi-lo<=8 else 'u128'};")
        operations=self.i['pcode']
        micro=any(o['op'] in ('BRANCH','CBRANCH') and o['in'][0]['space']=='const' for o in operations)
        if micro:lines+=['let mut ip = 0usize;','loop { match ip {']
        for ix,op in enumerate(operations):
            statements=self.code(op,ix,len(operations),micro)
            if micro:lines+=[f"{ix} => {{",*statements,f"ip = {ix+1};",'}']
            else:lines+=statements
        if micro:lines+=['_ => break,','} }']
        lines+=['}']
        return lines,None


def main():
    OUT.mkdir(parents=True,exist_ok=True)
    for old in OUT.glob('*.rs'):old.unlink()
    starts={f['entry'] for f in DATA['functions']}
    starts.update([0,0x34,0x108,0x208,0x322c,0x335c,0x33a4])
    order=sorted(INSNS)
    prev=None
    for address in order:
        i=INSNS[address]
        if prev is None or prev['address']+prev['length']!=address or address>>12!=prev['address']>>12:starts.add(address)
        prev=i
        starts.update(i.get('flows',[]))
        for op in i['pcode']:
            if op['op'] in ('BRANCH','CALL','CBRANCH','RETURN','BRANCHIND','CALLIND'):
                starts.add(address+i['length'])
                node=op['in'][0]
                if node['space']=='ram':starts.add(node['offset'])
    for offset in range(0,len(ROM)-3,4):
        val=struct.unpack_from('<I',ROM,offset)[0]
        if val&1 and val&~1 in INSNS:starts.add(val&~1)
    blocks=[];current=[]
    for address in order:
        if current and (address in starts or len(current)>=64):blocks.append(current);current=[]
        current.append(INSNS[address])
    if current:blocks.append(current)
    pages=collections.defaultdict(list)
    for b in blocks:pages[b[0]['address']>>12].append(b)
    fixed=collections.Counter()
    for page,items in sorted(pages.items()):
        lines=['// @generated by tools/lift_rust.py. Do not edit.', '#![allow(unused_assignments,unused_variables,unused_parens,unused_comparisons,unreachable_code,clippy::all)]',
            'use crate::machine::*;','pub(super) fn run(m: &mut Machine, pc: u32) -> Result<u32> {','match pc {']
        for block in items:
            address=block[0]['address'];lines.append(f'0x{address:08x} => b_{address:08x}(m),')
        lines+=['_ => Err(m.fault(format!("missing translated block 0x{pc:08x}"))),','}','}']
        for block in items:
            address=block[0]['address'];lines+=[f'fn b_{address:08x}(m: &mut Machine) -> Result<u32> {{',f'm.ticks += {len(block)};']
            for insn in block:
                lines.append(f"// 0x{insn['address']:08x}: {insn['assembly']}")
                lines.append(f"#[cfg(debug_assertions)] {{ m.pc = 0x{insn['address']:08x}; }}")
                body,kind=Instruction(insn).emit();lines+=body
                if kind:fixed[kind]+=1
            last=block[-1];lines+=[f"Ok(0x{last['address']+last['length']:08x})",'}']
        (OUT/f'page_{page:05x}.rs').write_text('\n'.join(lines)+'\n')
    module=['// @generated by tools/lift_rust.py.','use crate::machine::*;']
    for page in sorted(pages):module.append(f'mod page_{page:05x};')
    module+=['pub fn run_block(m: &mut Machine, pc:u32)->Result<u32> {','match pc>>12 {']
    for page in sorted(pages):module.append(f'0x{page:x} => page_{page:05x}::run(m,pc),')
    module+=['_ => Err(m.fault(format!("unmapped translated code 0x{pc:08x}"))),','}','}']
    module+=['pub const FUNCTIONS: &[(&str,u32)] = &[']
    for f in DATA['functions']:module.append(f'({json.dumps(f["name"])}, 0x{f["entry"]:08x}),')
    module+= ['];']
    (OUT/'mod.rs').write_text('\n'.join(module)+'\n')
    manifest={'firmware_version':'1.05','bin_sha256':hashlib.sha256(ROM).hexdigest(),'instructions':len(INSNS),'function_entries':len(DATA['functions']),
        'basic_blocks':len(blocks),'source_pages':len(pages),'semantic_overrides':dict(fixed),'method':'Static ARM/Thumb P-code lowering to Rust with explicit FPv5 VSEL and fused FMA semantics.'}
    (ROOT/'research/lift-manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
    print(json.dumps(manifest,indent=2))


if __name__=='__main__':main()
