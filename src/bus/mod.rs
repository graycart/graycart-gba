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
    bios_protect_byte, bios_protect_read, empty_cart_rom_halfword, empty_cart_rom_word, pc_in_bios,
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
/// empty SRAM reads as `0xFF` (uninit / no-save); BIOS outside-PC uses protect latch.
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
    /// Game Pak SRAM / Flash backup window (≤64 KiB) — legacy direct buffer.
    /// Prefer cart [`crate::cart::SaveBackend`] via [`crate::mmio::MachineMem`] for P7+.
    pub sram: Vec<u8>,
    /// BIOS-protect / open-bus latch state.
    pub open_bus: OpenBusState,
    /// Last known CPU PC for BIOS-protect gating (updated by machine step).
    pub cpu_pc: u32,
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
            open_bus: OpenBusState::default(),
            cpu_pc: 0,
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

    /// DISPCNT BG mode bits [2:0] for VRAM BG/OBJ split (video STRB policy).
    #[inline]
    fn dispcnt_bg_mode(&self) -> u8 {
        self.io.first().copied().unwrap_or(0) & 0x7
    }

    /// Apply GBATEK video STRB rules (ignore OBJ/OAM; expand BG/palette).
    fn write8_video(&mut self, target: VideoTarget, offset: usize, value: u8) {
        match resolve_strb(target, value) {
            VideoWriteAction::Ignore | VideoWriteAction::Reject | VideoWriteAction::Store => {}
            VideoWriteAction::ExpandByteToHalfword { halfword } => {
                let aligned = offset & !1;
                match target {
                    VideoTarget::Palette => {
                        Self::write_slice(&mut self.palette, aligned, halfword as u8);
                        Self::write_slice(&mut self.palette, aligned + 1, (halfword >> 8) as u8);
                    }
                    VideoTarget::BgVram => {
                        Self::write_slice(&mut self.vram, aligned, halfword as u8);
                        Self::write_slice(&mut self.vram, aligned + 1, (halfword >> 8) as u8);
                    }
                    VideoTarget::Oam | VideoTarget::ObjVram => {}
                }
            }
        }
    }

    /// Direct halfword store into video RAM (not via [`Self::write8`] — STRB policy differs).
    fn write16_video_raw(buf: &mut [u8], offset: usize, value: u16) {
        let aligned = offset & !1;
        Self::write_slice(buf, aligned, value as u8);
        Self::write_slice(buf, aligned + 1, (value >> 8) as u8);
    }
}

impl Bus {
    /// Read a BIOS byte with PC-gated protect (shared by [`CpuMem`] impl).
    fn read_bios8(&self, addr: u32) -> u8 {
        if let Some(latch) = bios_protect_read(self.cpu_pc, &self.open_bus) {
            return bios_protect_byte(latch, addr);
        }
        // Inside BIOS: normal ROM read; note fetch for ARM word-aligned ops via read32.
        bios_offset(addr)
            .map(|i| Self::read_slice(&self.bios, i))
            .unwrap_or(0)
    }

    /// Read backup window: empty buffer → `0xFF` (uninit / no-save).
    fn read_sram8(&self, addr: u32) -> u8 {
        if self.sram.is_empty() {
            return 0xFF;
        }
        let i = sram_window_offset(addr) % self.sram.len();
        self.sram.get(i).copied().unwrap_or(0xFF)
    }
}

impl CpuMem for Bus {
    fn read8(&mut self, addr: u32) -> u8 {
        match decode(addr) {
            Region::Bios => self.read_bios8(addr),
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
            Region::GamePakSram => self.read_sram8(addr),
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
            Region::Palette => {
                self.write8_video(VideoTarget::Palette, palette_offset(addr), value);
            }
            Region::Vram => {
                let off = vram_offset(addr);
                if let Some(target) = classify_vram_offset(off as u32, self.dispcnt_bg_mode()) {
                    self.write8_video(target, off, value);
                }
            }
            // OAM STRB ignored (GBATEK video byte-store rules).
            Region::Oam => {}
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
        match decode(addr) {
            Region::GamePakSram => {
                // Only the addressed byte is stored; value = LSB of (data ROR (addr*8)).
                // Pass-through unaligned `addr` (STRH must not force-align before this).
                let rotated = value.rotate_right((addr & 1) * 8);
                self.write8(addr, rotated as u8);
            }
            // Video: 16-bit stores are real halfwords — must not go through STRB write8.
            Region::Palette => {
                Self::write16_video_raw(&mut self.palette, palette_offset(addr), value);
            }
            Region::Vram => {
                Self::write16_video_raw(&mut self.vram, vram_offset(addr), value);
            }
            Region::Oam => {
                Self::write16_video_raw(&mut self.oam, oam_offset(addr), value);
            }
            _ => {
                let a = addr & !1;
                self.write8(a, value as u8);
                self.write8(a.wrapping_add(1), (value >> 8) as u8);
            }
        }
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
        // Two halfword stores on aligned base — video path uses write16_video_raw via write16.
        let a = addr & !3;
        self.write16(a, value as u16);
        self.write16(a.wrapping_add(2), (value >> 16) as u16);
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
