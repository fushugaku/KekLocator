//! Memory and ABI adapters for the statically translated firmware.
//! Instructions themselves are Rust statements in `generated`, not interpreted bytes.
use std::fmt;

pub const RETURN_PC: u32 = 0xffff_fff0;
pub const FLASH_BASE: u32 = 0x6002_0000;
pub const ROM: &[u8] = include_bytes!("../firmware/echolocator-1.05.bin");
pub type Result<T> = std::result::Result<T, Fault>;

#[derive(Debug, Clone)]
pub struct Fault {
    pub pc: u32,
    pub message: String,
}
impl fmt::Display for Fault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{:08x}: {}", self.pc, self.message)
    }
}
impl std::error::Error for Fault {}

pub struct Machine {
    pub registers: [u8; 2048],
    pub itcm: Vec<u8>,
    pub dtcm: Vec<u8>,
    pub ocram: Vec<u8>,
    pub heap: Vec<u8>,
    pub pc: u32,
    pub ticks: u64,
    pub budget: u64,
    pub heap_cursor: usize,
    pub exclusive: u32,
    pub irq_enabled: bool,
}

impl Default for Machine {
    fn default() -> Self {
        Self::new()
    }
}
impl Machine {
    pub fn new() -> Self {
        let mut dtcm = vec![0; 0x20000];
        dtcm[..0x1f60].copy_from_slice(&ROM[0x37398..0x392f8]);
        dtcm[0x1f60..0x2674].copy_from_slice(&ROM[0x3c7f8..0x3cf0c]);
        Self {
            registers: [0; 2048],
            itcm: ROM[0x392f8..0x3c7f8].to_vec(),
            dtcm,
            ocram: vec![0; 0x80000],
            heap: vec![0; 32 * 1024 * 1024],
            pc: 0,
            ticks: 0,
            budget: 0,
            heap_cursor: 0,
            exclusive: u32::MAX,
            irq_enabled: true,
        }
    }
    pub fn fault(&self, message: impl Into<String>) -> Fault {
        Fault {
            pc: self.pc,
            message: message.into(),
        }
    }
    #[inline(always)]
    pub fn reg(&self, off: usize, size: usize) -> u64 {
        assert!(size <= self.registers.len() && off <= self.registers.len() - size);
        // SAFETY: the span is checked above. Generated constant offsets remove this
        // check during optimization, while callers of the public API remain checked.
        unsafe {
            let p = self.registers.as_ptr().add(off);
            match size {
                1 => *p as u64,
                2 => u16::from_le(std::ptr::read_unaligned(p.cast())) as u64,
                4 => u32::from_le(std::ptr::read_unaligned(p.cast())) as u64,
                8 => u64::from_le(std::ptr::read_unaligned(p.cast())),
                _ => unreachable!(),
            }
        }
    }
    #[inline(always)]
    pub fn set_reg(&mut self, off: usize, size: usize, value: u64) {
        assert!(size <= self.registers.len() && off <= self.registers.len() - size);
        // SAFETY: the complete unaligned write is inside the checked register span.
        unsafe {
            let p = self.registers.as_mut_ptr().add(off);
            match size {
                1 => *p = value as u8,
                2 => std::ptr::write_unaligned(p.cast(), (value as u16).to_le()),
                4 => std::ptr::write_unaligned(p.cast(), (value as u32).to_le()),
                8 => std::ptr::write_unaligned(p.cast(), value.to_le()),
                _ => unreachable!(),
            }
        }
    }
    fn region(&self, address: u32, size: usize) -> Result<&[u8]> {
        let (bytes, off): (&[u8], usize) = match address {
            0x00000000..=0x000034ff => (&self.itcm, address as usize),
            0x20000000..=0x2001ffff => (&self.dtcm, (address - 0x20000000) as usize),
            0x20200000..=0x2027ffff => (&self.ocram, (address - 0x20200000) as usize),
            0x60020000..=0x6005cfff => (ROM, (address - FLASH_BASE) as usize),
            0x80000000..=0x81ffffff => (&self.heap, (address - 0x80000000) as usize),
            _ => {
                return Err(self.fault(format!(
                    "read unmapped address 0x{address:08x} ({size} bytes)"
                )))
            }
        };
        bytes
            .get(off..off + size)
            .ok_or_else(|| self.fault(format!("read crosses region at 0x{address:08x}")))
    }
    fn region_mut(&mut self, address: u32, size: usize) -> Result<&mut [u8]> {
        let pc = self.pc;
        let (bytes, off): (&mut [u8], usize) = match address {
            0x00000000..=0x000034ff => (&mut self.itcm, address as usize),
            0x20000000..=0x2001ffff => (&mut self.dtcm, (address - 0x20000000) as usize),
            0x20200000..=0x2027ffff => (&mut self.ocram, (address - 0x20200000) as usize),
            0x80000000..=0x81ffffff => (&mut self.heap, (address - 0x80000000) as usize),
            _ => {
                return Err(Fault {
                    pc,
                    message: format!("write unmapped address 0x{address:08x} ({size} bytes)"),
                })
            }
        };
        bytes.get_mut(off..off + size).ok_or_else(|| Fault {
            pc,
            message: format!("write crosses region at 0x{address:08x}"),
        })
    }
    #[inline(always)]
    pub fn load(&self, address: u32, size: usize) -> Result<u64> {
        let b = self.region(address, size)?;
        Ok(match size {
            1 => b[0] as u64,
            2 => u16::from_le_bytes(b.try_into().unwrap()) as u64,
            4 => u32::from_le_bytes(b.try_into().unwrap()) as u64,
            8 => u64::from_le_bytes(b.try_into().unwrap()),
            _ => return Err(self.fault("invalid load width")),
        })
    }
    #[inline(always)]
    pub fn store(&mut self, address: u32, size: usize, value: u64) -> Result<()> {
        self.region_mut(address, size)?
            .copy_from_slice(&value.to_le_bytes()[..size]);
        Ok(())
    }
    pub fn read_f32(&self, address: u32) -> Result<f32> {
        Ok(f32::from_bits(self.load(address, 4)? as u32))
    }
    pub fn write_f32(&mut self, address: u32, value: f32) -> Result<()> {
        self.store(address, 4, value.to_bits() as u64)
    }
    pub fn allocate(&mut self, bytes: usize) -> Result<u32> {
        let start = (self.heap_cursor + 15) & !15;
        if start
            .checked_add(bytes)
            .is_none_or(|end| end > self.heap.len())
        {
            return Err(self.fault(format!(
                "firmware heap exhausted: allocation {bytes}, used {start}"
            )));
        }
        #[cfg(not(target_arch = "wasm32"))]
        if std::env::var_os("ERIKA_TRACE_ALLOC").is_some() {
            eprintln!("malloc {bytes} at 0x{:08x}", 0x80000000u32 + start as u32);
        }
        self.heap[start..start + bytes].fill(0);
        self.heap_cursor = start + bytes;
        Ok(0x80000000 + start as u32)
    }
    pub fn invoke(
        &mut self,
        entry: u32,
        args: &[u32],
        float_arg: Option<f32>,
        budget: u64,
    ) -> Result<u32> {
        for (i, &v) in args.iter().take(4).enumerate() {
            self.set_reg(0x20 + i * 4, 4, v as u64);
        }
        if let Some(v) = float_arg {
            self.set_reg(0x300, 4, v.to_bits() as u64);
        }
        self.set_reg(0x54, 4, 0x2001fff0);
        self.set_reg(0x58, 4, (RETURN_PC | 1) as u64);
        self.pc = entry & !1;
        self.budget = self.ticks.saturating_add(budget);
        while self.pc != RETURN_PC {
            if self.ticks >= self.budget {
                return Err(self.fault("instruction budget exceeded"));
            }
            if !self.host_call()? {
                self.pc = crate::generated::run_block(self, self.pc)?;
            }
        }
        Ok(self.reg(0x20, 4) as u32)
    }
    fn host_call(&mut self) -> Result<bool> {
        match self.pc {
            // Runtime alloc/free are replaced by a private, zeroed browser heap.
            0x600413e0 => {
                let p = self.allocate(self.reg(0x20, 4) as usize)?;
                self.set_reg(0x20, 4, p as u64);
            }
            0x600413f0 => {}
            // The analogue DAC controls external VCAs; web audio uses digital mixing.
            0x60030af4 => {}
            // These are exact byte operations, independent of ARM alignment paths.
            0x60041868 => {
                let dest = self.reg(0x20, 4) as u32;
                let byte = self.reg(0x24, 4) as u8;
                let len = self.reg(0x28, 4) as usize;
                self.region_mut(dest, len)?.fill(byte);
            }
            0x6002047c => {
                let dest = self.reg(0x20, 4) as u32;
                let source = self.reg(0x24, 4) as u32;
                let len = self.reg(0x28, 4) as usize;
                let temp = self.region(source, len)?.to_vec();
                self.region_mut(dest, len)?.copy_from_slice(&temp);
            }
            _ => return Ok(false),
        }
        self.pc = (self.reg(0x58, 4) as u32) & !1;
        self.ticks += 1;
        Ok(true)
    }
}

#[inline(always)]
pub fn mask(size: usize) -> u64 {
    if size == 8 {
        u64::MAX
    } else {
        (1u64 << (size * 8)) - 1
    }
}
#[inline(always)]
pub fn signed(value: u64, size: usize) -> i64 {
    ((value << (64 - size * 8)) as i64) >> (64 - size * 8)
}
#[inline(always)]
pub fn shl(value: u64, amount: u64) -> u64 {
    if amount >= 64 {
        0
    } else {
        value << amount
    }
}
#[inline(always)]
pub fn shr(value: u64, amount: u64) -> u64 {
    if amount >= 64 {
        0
    } else {
        value >> amount
    }
}
#[inline(always)]
pub fn sar(value: u64, amount: u64, size: usize) -> u64 {
    (signed(value, size) >> (amount.min(63))) as u64
}
#[inline(always)]
pub fn carry(a: u64, b: u64, size: usize) -> u64 {
    ((a as u128 + b as u128) > (mask(size) as u128)) as u64
}
#[inline(always)]
pub fn scarry(a: u64, b: u64, size: usize) -> u64 {
    let a = signed(a, size) as i128;
    let b = signed(b, size) as i128;
    let v = a + b;
    (v < -(1i128 << (size * 8 - 1)) || v > (1i128 << (size * 8 - 1)) - 1) as u64
}
#[inline(always)]
pub fn sborrow(a: u64, b: u64, size: usize) -> u64 {
    let a = signed(a, size) as i128;
    let b = signed(b, size) as i128;
    let v = a - b;
    (v < -(1i128 << (size * 8 - 1)) || v > (1i128 << (size * 8 - 1)) - 1) as u64
}
#[inline(always)]
pub fn udiv(a: u64, b: u64) -> u64 {
    if b == 0 {
        0
    } else {
        a / b
    }
}
#[inline(always)]
pub fn sdiv(a: u64, b: u64, size: usize) -> u64 {
    let a = signed(a, size);
    let b = signed(b, size);
    if b == 0 {
        0
    } else {
        a.wrapping_div(b) as u64
    }
}
#[inline(always)]
pub fn float_to_fixed(bits: u64, width: usize, fraction: u32) -> u64 {
    let v = (f32::from_bits(bits as u32) as f64) * 2f64.powi(fraction as i32);
    if width == 32 {
        (v as i32) as u32 as u64
    } else {
        (v as i16) as u16 as u64
    }
}
