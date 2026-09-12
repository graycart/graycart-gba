//! Memory bus: region table, mirrors, storage wiring, and waitstate re-exports (P2).
//!
//! Module layout from graycart-gba implementation plan §2.2.
//! Behavior: research `docs/graycart-gba/02-memory-bus-dma.md` §§2–5.
//!
//! Cited: GBATEK — GBA Memory Map
//!   https://problemkaputt.de/gbatek-gba-memory-map.htm
//! Cited: GBATEK — GBA Unpredictable Things (mirrors)
//!   https://problemkaputt.de/gbatek-gba-unpredictable-things.htm
//! Cited: ARM7TDMI TRM (DDI0210C) §1.1.2 — byte / halfword / word data sizes
//!   https://developer.arm.com/documentation/ddi0210/c/
//! Note: [`CpuMem`] for CPU pipeline fetch; video STRB / open-bus are sibling modules.

pub mod mirror;
pub mod openbus;
pub mod region;
pub mod video;
pub mod wait;
pub mod waitcnt;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_openbus;
#[cfg(test)]
mod tests_video;
#[cfg(test)]
mod tests_wait;

pub use openbus::{
    bios_protect_read, empty_cart_rom_halfword, empty_cart_rom_word, pc_in_bios,
    unused_memory_open_bus, OpenBusKind, OpenBusState, BIOS_END,
};
pub use video::{
    classify_vram_offset, expand_strb_byte, obj_vram_base, resolve_strb, resolve_video_write,
    resolve_wide_write, write_size_ok, VideoTarget, VideoWriteAction, OBJ_VRAM_BASE_BITMAP,
    OBJ_VRAM_BASE_TILE,
};
pub use wait::{
    cycles_for_waits, rom_force_nonseq, AccessKind, AccessSize, RomWindow, WaitTables,
    EWRAM_DEFAULT_WAITS, ROM_FORCE_N_BLOCK,
};
pub use waitcnt::{
    WaitCnt, COMMERCIAL_COMMON as WAITCNT_COMMERCIAL_COMMON, POWER_ON as WAITCNT_POWER_ON,
};

use mirror::{
    bios_offset, ewram_offset, io_offset, iwram_offset, oam_offset, palette_offset, rom_offset,
    sram_window_offset, vram_offset,
};
use region::{decode, Region, EWRAM_SIZE, IO_SIZE, IWRAM_SIZE, OAM_SIZE, PALETTE_SIZE, VRAM_SIZE};

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

/// GBA memory bus with region-backed storage.
///
/// Internal RAM (EWRAM / IWRAM / I/O / palette / VRAM / OAM) is always allocated.
/// BIOS, Game Pak ROM, and SRAM start empty until cart/BIOS owners load images —
/// reads of unloaded media return `0` (open-bus / empty-cart patterns are TBD siblings).
#[derive(Debug, Clone)]
pub struct Bus {
    /// Optional BIOS image (16 KiB when loaded).
    pub bios: Vec<u8>,
    /// On-board WRAM (256 KiB).
    pub ewram: Vec<u8>,
    /// On-chip WRAM (32 KiB).
    pub iwram: Vec<u8>,
    /// I/O register file through IMC (`0x800`).
    pub io: Vec<u8>,
    /// Palette RAM (1 KiB).
    pub palette: Vec<u8>,
    /// VRAM (96 KiB physical).
    pub vram: Vec<u8>,
    /// OAM (1 KiB).
    pub oam: Vec<u8>,
    /// Game Pak ROM image (shared across WS0–2 views).
    pub rom: Vec<u8>,
    /// Game Pak SRAM / Flash backup window (≤64 KiB).
    pub sram: Vec<u8>,
}

impl Default for Bus {
    fn default() -> Self {
        Self {
            bios: Vec::new(),
            ewram: vec![0; EWRAM_SIZE],
            iwram: vec![0; IWRAM_SIZE],
            io: vec![0; IO_SIZE],
            palette: vec![0; PALETTE_SIZE],
            vram: vec![0; VRAM_SIZE],
            oam: vec![0; OAM_SIZE],
            rom: Vec::new(),
            sram: Vec::new(),
        }
    }
}

impl Bus {
    /// Power-on bus with zeroed internal RAM and unloaded media.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Decode `addr` to a [`Region`] (public for waitstate / DMA siblings).
    #[must_use]
    pub fn region_of(addr: u32) -> Region {
        decode(addr)
    }

    fn read_slice(buf: &[u8], index: usize) -> u8 {
        buf.get(index).copied().unwrap_or(0)
    }

    fn write_slice(buf: &mut [u8], index: usize, value: u8) {
        if let Some(slot) = buf.get_mut(index) {
            *slot = value;
        }
    }
}

impl CpuMem for Bus {
    fn read8(&mut self, addr: u32) -> u8 {
        match decode(addr) {
            Region::Bios => bios_offset(addr)
                .map(|i| Self::read_slice(&self.bios, i))
                .unwrap_or(0),
            Region::UnusedLow | Region::UnusedHigh => 0, // open-bus TBD (sibling)
            Region::Ewram => Self::read_slice(&self.ewram, ewram_offset(addr)),
            Region::Iwram => Self::read_slice(&self.iwram, iwram_offset(addr)),
            Region::Io => io_offset(addr)
                .map(|i| Self::read_slice(&self.io, i))
                .unwrap_or(0), // unused I/O → open-bus TBD
            Region::Palette => Self::read_slice(&self.palette, palette_offset(addr)),
            Region::Vram => Self::read_slice(&self.vram, vram_offset(addr)),
            Region::Oam => Self::read_slice(&self.oam, oam_offset(addr)),
            Region::GamePakRomWs0 | Region::GamePakRomWs1 | Region::GamePakRomWs2 => {
                Self::read_slice(&self.rom, rom_offset(addr))
            }
            Region::GamePakSram => Self::read_slice(&self.sram, sram_window_offset(addr)),
        }
    }

    fn write8(&mut self, addr: u32, value: u8) {
        match decode(addr) {
            Region::Bios | Region::UnusedLow | Region::UnusedHigh => {}
            Region::Ewram => Self::write_slice(&mut self.ewram, ewram_offset(addr), value),
            Region::Iwram => Self::write_slice(&mut self.iwram, iwram_offset(addr), value),
            Region::Io => {
                if let Some(i) = io_offset(addr) {
                    Self::write_slice(&mut self.io, i, value);
                }
            }
            Region::Palette => Self::write_slice(&mut self.palette, palette_offset(addr), value),
            Region::Vram => Self::write_slice(&mut self.vram, vram_offset(addr), value),
            Region::Oam => Self::write_slice(&mut self.oam, oam_offset(addr), value),
            // ROM writes ignored here (FlashROM commands are cart-owned, P7).
            Region::GamePakRomWs0 | Region::GamePakRomWs1 | Region::GamePakRomWs2 => {}
            Region::GamePakSram => {
                Self::write_slice(&mut self.sram, sram_window_offset(addr), value);
            }
        }
    }

    fn read16(&mut self, addr: u32) -> u16 {
        // 8-bit SRAM bus: hardware duplicates the addressed byte across the halfword.
        if decode(addr) == Region::GamePakSram {
            let b = u16::from(self.read8(addr));
            return b | (b << 8);
        }
        let lo = u16::from(self.read8(addr));
        let hi = u16::from(self.read8(addr.wrapping_add(1)));
        lo | (hi << 8)
    }

    fn write16(&mut self, addr: u32, value: u16) {
        if decode(addr) == Region::GamePakSram {
            // Only the addressed byte is stored; value = LSB of (data ROR (addr*8)).
            let rotated = value.rotate_right((addr & 1) * 8);
            self.write8(addr, rotated as u8);
            return;
        }
        self.write8(addr, value as u8);
        self.write8(addr.wrapping_add(1), (value >> 8) as u8);
    }

    fn read32(&mut self, addr: u32) -> u32 {
        if decode(addr) == Region::GamePakSram {
            let b = u32::from(self.read8(addr));
            return b * 0x0101_0101;
        }
        let lo = u32::from(self.read16(addr));
        let hi = u32::from(self.read16(addr.wrapping_add(2)));
        lo | (hi << 16)
    }

    fn write32(&mut self, addr: u32, value: u32) {
        if decode(addr) == Region::GamePakSram {
            let rotated = value.rotate_right((addr & 3) * 8);
            self.write8(addr, rotated as u8);
            return;
        }
        self.write16(addr, value as u16);
        self.write16(addr.wrapping_add(2), (value >> 16) as u16);
    }
}

/// Contiguous little-endian RAM for CPU unit/integration tests.
#[derive(Debug, Clone)]
pub struct FlatRam {
    pub data: Vec<u8>,
    pub base: u32,
}

impl FlatRam {
    #[must_use]
    pub fn new(size: usize) -> Self {
        Self {
            data: vec![0; size],
            base: 0,
        }
    }

    #[must_use]
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
