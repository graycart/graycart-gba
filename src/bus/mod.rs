//! Memory bus / region table placeholder (P2).
//!
//! Module layout from graycart-gba implementation plan §2.2.
//! Behavior: see research `docs/graycart-gba/02-memory-bus-dma.md` (not implemented).
//!
//! Cited: ARM7TDMI TRM (DDI0210C) §1.1.2 — byte / halfword / word data sizes
//!   https://developer.arm.com/documentation/ddi0210/c/
//! Note: thin [`CpuMem`] surface for CPU pipeline fetch only; waitstates/regions are P2.

/// Minimal CPU-facing memory surface (byte / half / word).
///
/// Endianness: little-endian (GBA). Alignment quirks (ROR on misaligned LDR, etc.)
/// belong in the CPU transfer paths, not here.
pub trait CpuMem {
    fn read8(&mut self, addr: u32) -> u8;
    fn write8(&mut self, addr: u32, value: u8);

    fn read16(&mut self, addr: u32) -> u16 {
        let lo = u16::from(self.read8(addr));
        let hi = u16::from(self.read8(addr.wrapping_add(1)));
        lo | (hi << 8)
    }

    fn write16(&mut self, addr: u32, value: u16) {
        self.write8(addr, value as u8);
        self.write8(addr.wrapping_add(1), (value >> 8) as u8);
    }

    fn read32(&mut self, addr: u32) -> u32 {
        let lo = u32::from(self.read16(addr));
        let hi = u32::from(self.read16(addr.wrapping_add(2)));
        lo | (hi << 16)
    }

    fn write32(&mut self, addr: u32, value: u32) {
        self.write16(addr, value as u16);
        self.write16(addr.wrapping_add(2), (value >> 16) as u16);
    }
}

/// Stub bus — sole memory authority once filled in.
#[derive(Debug, Default)]
pub struct Bus;

impl CpuMem for Bus {
    fn read8(&mut self, _addr: u32) -> u8 {
        0
    }

    fn write8(&mut self, _addr: u32, _value: u8) {}
}

/// Contiguous little-endian RAM for CPU unit/integration tests.
#[derive(Debug, Clone)]
pub struct FlatRam {
    pub data: Vec<u8>,
    pub base: u32,
}

impl FlatRam {
    pub fn new(size: usize) -> Self {
        Self {
            data: vec![0; size],
            base: 0,
        }
    }

    pub fn with_base(base: u32, size: usize) -> Self {
        Self {
            data: vec![0; size],
            base,
        }
    }

    fn index(&self, addr: u32) -> Option<usize> {
        let rel = addr.wrapping_sub(self.base) as usize;
        if rel < self.data.len() {
            Some(rel)
        } else {
            None
        }
    }
}

impl CpuMem for FlatRam {
    fn read8(&mut self, addr: u32) -> u8 {
        self.index(addr).map(|i| self.data[i]).unwrap_or(0xFF)
    }

    fn write8(&mut self, addr: u32, value: u8) {
        if let Some(i) = self.index(addr) {
            self.data[i] = value;
        }
    }
}
