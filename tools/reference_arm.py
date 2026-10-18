#!/usr/bin/env python3
"""Execute the unchanged Echolocator Thumb binary, independently of Ghidra/Rust."""
from pathlib import Path
import struct
import time
import unicorn as uc
from unicorn import arm_const as arm
from capstone import Cs, CS_ARCH_ARM, CS_MODE_THUMB, CS_MODE_MCLASS

ROOT = Path(__file__).resolve().parent.parent
ROM = (ROOT / 'crates/engine/firmware/echolocator-1.05.bin').read_bytes()
STOP = 0x10000000
INPUT, OUTPUT, STATE = 0x20002674, 0x200027f4, 0x20000628

class OriginalArm:
    def __init__(self):
        # Unicorn's M7 model implements SP-only VFP; the firmware also uses f64.
        # MAX executes the shared Thumb-2/DSP/FPv5 instruction subset, including VSEL.
        self.cpu = uc.Uc(uc.UC_ARCH_ARM, uc.UC_MODE_THUMB)
        self.cpu.ctl_set_cpu_model(arm.UC_CPU_ARM_MAX)
        for address, length in [(0,0x4000),(0x20000000,0x20000),(0x20200000,0x80000),
                                (0x60020000,0x3d000),(0x80000000,0x2000000),(STOP,0x1000)]:
            self.cpu.mem_map(address,length)
        self.cpu.mem_write(0x60020000,ROM)
        self.cpu.mem_write(0,ROM[0x392f8:0x3c7f8])
        self.cpu.mem_write(0x20000000,ROM[0x37398:0x392f8])
        self.cpu.mem_write(0x20001f60,ROM[0x3c7f8:0x3cf0c])
        self.cpu.reg_write(arm.UC_ARM_REG_C1_C0_2,0xf00000)
        self.cpu.reg_write(arm.UC_ARM_REG_FPEXC,0x40000000)
        self.cpu.reg_write(arm.UC_ARM_REG_FPSCR,0)
        self.heap_cursor = 0
        self.calls = 0
        self.allocations = []
        self.firmware_controls = False
        self.morph_active = False
        self.disasm = Cs(CS_ARCH_ARM,CS_MODE_THUMB|CS_MODE_MCLASS)
        for address in [0x600413e0,0x600413f0,0x60030af4]:
            self.cpu.hook_add(uc.UC_HOOK_CODE,self.host_call,begin=address,end=address)
        self.cpu.hook_add(uc.UC_HOOK_MEM_INVALID,self.invalid_memory)

    def u32(self,address): return struct.unpack('<I',self.cpu.mem_read(address,4))[0]
    def write_u32(self,address,value): self.cpu.mem_write(address,struct.pack('<I',value))
    def write_f32(self,address,value): self.cpu.mem_write(address,struct.pack('<f',value))
    def host_call(self,cpu,address,size,data):
        if address == 0x600413e0:
            length=cpu.reg_read(arm.UC_ARM_REG_R0)
            offset=(self.heap_cursor+15)&~15
            if offset+length>0x2000000: raise RuntimeError('Original firmware heap overflow')
            self.heap_cursor=offset+length
            self.allocations.append(length)
            cpu.reg_write(arm.UC_ARM_REG_R0,0x80000000+offset)
        cpu.reg_write(arm.UC_ARM_REG_PC,cpu.reg_read(arm.UC_ARM_REG_LR))
    def invalid_memory(self,cpu,access,address,size,value,data):
        print(f'ARM invalid memory access pc={cpu.reg_read(arm.UC_ARM_REG_PC):08x} address={address:08x} size={size}',flush=True)
        return False
    def invoke(self,entry,args=(),float_bits=None,budget=0):
        for reg,value in zip([arm.UC_ARM_REG_R0,arm.UC_ARM_REG_R1,arm.UC_ARM_REG_R2,arm.UC_ARM_REG_R3],args):
            self.cpu.reg_write(reg,value)
        if float_bits is not None: self.cpu.reg_write(arm.UC_ARM_REG_S0,float_bits)
        self.cpu.reg_write(arm.UC_ARM_REG_SP,0x2001fff0)
        self.cpu.reg_write(arm.UC_ARM_REG_LR,STOP|1)
        self.calls+=1
        try: self.cpu.emu_start(entry|1,STOP,count=budget)
        except uc.UcError as error:
            pc=self.cpu.reg_read(arm.UC_ARM_REG_PC)
            code=bytes(self.cpu.mem_read(pc,12))
            instructions='; '.join(f'{i.address:08x}: {i.mnemonic} {i.op_str}' for i in self.disasm.disasm(code,pc))
            raise RuntimeError(f'{error}; entry={entry:08x} pc={pc:08x}; {instructions}') from error
        pc=self.cpu.reg_read(arm.UC_ARM_REG_PC)
        if pc!=STOP: raise RuntimeError(f'Instruction budget exhausted at {pc:08x}, entry={entry:08x}')
        return self.cpu.reg_read(arm.UC_ARM_REG_R0)
    def initialize(self):
        self.write_u32(0x20003414,0x20001234)
        for i in range(36): self.write_u32(0x20001238+i*32,self.u32(0x20001244+i*32))
        self.definitions=[]
        for i in range(20):
            # The definitions are min/max/default at table offset 24.
            low,high,initial=struct.unpack('<3f',self.cpu.mem_read(0x20000088+i*72+24,12))
            self.definitions.append((low,high,initial))
            self.write_f32(0x200000a8+i*72,(initial-low)/(high-low))
        self.invoke(0x6002a77c)
        self.invoke(0x6002aabc,[0])
        zero=bytes(24*2*4)
        for _ in range(128): self.block(zero)
    def parameter(self,index,normalized):
        low,high,_=self.definitions[index]
        if index in (0,13,14,15,17,19): normalized=float(normalized>=0.5)
        if self.firmware_controls:
            bits=struct.unpack('<I',struct.pack('<f',normalized))[0]
            self.invoke(0x6002a7e0,[index,0],bits)
            if not self.morph_active:self.invoke(0x322c)
            return
        # Match separate binary32 multiply and addition at the host parameter boundary.
        f32=lambda v:struct.unpack('<f',struct.pack('<f',v))[0]
        value=f32(f32(low)+f32(f32(high-low)*f32(normalized)))
        self.write_f32(0x200000a8+index*72,normalized)
        self.invoke(self.u32(0x200000c8+index*72),[STATE],struct.unpack('<I',struct.pack('<f',value))[0])
    def use_firmware_controls(self):
        # The browser supplies normalized values rather than reading physical ADC pots.
        # All mapping, change detection and DSP callbacks still execute original code.
        for i in range(20):self.cpu.mem_write(0x20000088+i*72+68,b'\0')
        self.firmware_controls=True
    def install_clock_source(self):
        """Replace only the physical SysTick/millisecond source for MIDI tests."""
        self.clock_ms=0
        def clock(cpu,address,size,data):
            cpu.reg_write(arm.UC_ARM_REG_R0,int(self.clock_ms)&0xffffffff)
            cpu.reg_write(arm.UC_ARM_REG_PC,cpu.reg_read(arm.UC_ARM_REG_LR))
        self.cpu.hook_add(uc.UC_HOOK_CODE,clock,begin=0x60033b48,end=0x60033b48)
        self.cpu.ctl_remove_cache(0x60033b48,0x60033b98)
    def begin_morph(self,values,seconds):
        if not self.firmware_controls:self.use_firmware_controls()
        self.morph_active=False
        for i in range(20):self.cpu.mem_write(0x200000c0+i*72,b'\0')
        self.parameter(19,0)
        self.write_f32(0x200033c4,seconds)
        for i,value in enumerate(values[:19]):
            if i in(0,13,14,15,17):value=float(value>=.5)
            self.invoke(0x6002a7e0,[i,1],struct.unpack('<I',struct.pack('<f',value))[0])
        self.morph_active=any(self.cpu.mem_read(0x200000c0+i*72,1)[0]for i in range(19))
    def block(self,samples):
        assert len(samples)==192
        if self.morph_active:
            self.invoke(0x322c)
            self.morph_active=any(self.cpu.mem_read(0x200000c0+i*72,1)[0]for i in range(20))
        self.cpu.mem_write(INPUT,samples)
        self.invoke(0x208,[INPUT,OUTPUT,24])
        return bytes(self.cpu.mem_read(OUTPUT,192))

if __name__=='__main__':
    started=time.perf_counter(); original=OriginalArm();original.initialize()
    print(f'Original ARM initialized in {time.perf_counter()-started:.3f}s; heap={original.heap_cursor}; allocations={original.allocations}')
    print('Q31 silence:',struct.unpack('<48i',original.block(bytes(192))))
