//! Address mirrors / physical offsets within GBA memory regions.
//!
//! Cited: GBATEK — GBA Unpredictable Things (mirrors)
//!   https://problemkaputt.de/gbatek-gba-unpredictable-things.htm
//! Cross-check: gbadoc — Memory Layout
//!   https://gbadev.net/gbadoc/memory.html
//! Note: region decode lives in [`crate::bus::region`]; open-bus values are sibling TBD.

use crate::bus::region::{
    EWRAM_SIZE, IO_SIZE, IWRAM_SIZE, OAM_SIZE, PALETTE_SIZE, ROM_WINDOW_SIZE, SRAM_WINDOW_SIZE,
    VRAM_SIZE,
};

/// EWRAM offset: 256 KiB mirrored every `0x40000` across `02xxxxxx`.
#[must_use]
pub fn ewram_offset(addr: u32) -> usize {
    (addr as usize) & (EWRAM_SIZE - 1)
}

/// IWRAM offset: 32 KiB mirrored every `0x8000` across `03xxxxxx`.
#[must_use]
pub fn iwram_offset(addr: u32) -> usize {
    (addr as usize) & (IWRAM_SIZE - 1)
}

/// Palette offset: 1 KiB mirrored every `0x400` across `05xxxxxx`.
#[must_use]
pub fn palette_offset(addr: u32) -> usize {
    (addr as usize) & (PALETTE_SIZE - 1)
}

/// OAM offset: 1 KiB mirrored every `0x400` across `07xxxxxx`.
#[must_use]
pub fn oam_offset(addr: u32) -> usize {
    (addr as usize) & (OAM_SIZE - 1)
}

/// VRAM offset into the physical 96 KiB store.
///
/// Hardware presents 96 KiB as a 128 KiB block (`64K+32K+32K` with the two 32K
/// halves mirroring each other), then mirrors that block across `06xxxxxx`.
#[must_use]
pub fn vram_offset(addr: u32) -> usize {
    let mut off = (addr as usize) & 0x1_FFFF; // 128 KiB window
    if off >= 0x1_0000 {
        // Map both `0x10000–0x17FFF` and `0x18000–0x1FFFF` onto `0x10000–0x17FFF`.
        off = 0x1_0000 | (off & 0x7_FFF);
    }
    debug_assert!(off < VRAM_SIZE);
    off
}

/// Game Pak ROM offset within the 32 MiB window (same image for WS0/WS1/WS2).
///
/// ROM waitstate windows are alternate views, not internal-RAM-style mirrors.
#[must_use]
pub fn rom_offset(addr: u32) -> usize {
    (addr as usize) & (ROM_WINDOW_SIZE - 1)
}

/// SRAM / Flash backup offset within the 64 KiB window.
///
/// The 64 KiB field is mirrored across `0E000000–0FFFFFFF`. Typical 32 KiB SRAM
/// chips repeat twice inside that field (`addr & 0x7FFF` for chip-relative).
#[must_use]
pub fn sram_window_offset(addr: u32) -> usize {
    (addr as usize) & (SRAM_WINDOW_SIZE - 1)
}

/// Offset into a 32 KiB SRAM chip image (second half of the 64 KiB window mirrors).
#[must_use]
pub fn sram_chip_offset(addr: u32) -> usize {
    (addr as usize) & 0x7_FFF
}

/// Map an I/O address to a register-file offset, if the location is backed.
///
/// I/O is **not** mirrored across `04xxxxxx`, except the undocumented IMC word
/// at `04000800h`–`04000803h`, which repeats every 64 KiB.
#[must_use]
pub fn io_offset(addr: u32) -> Option<usize> {
    if (addr >> 24) != 0x04 {
        return None;
    }
    let page_off = (addr & 0xFFFF) as usize;
    // Undocumented IMC is a 32-bit word mirrored every 64 KiB.
    if (0x0800..0x0804).contains(&page_off) {
        return Some(page_off);
    }
    // Primary I/O window only (not mirrored).
    if (0x0400_0000..=0x0400_03FF).contains(&addr) {
        let off = (addr - 0x0400_0000) as usize;
        if off < IO_SIZE {
            return Some(off);
        }
    }
    None
}

/// BIOS offset, or `None` if outside the 16 KiB System ROM.
#[must_use]
pub fn bios_offset(addr: u32) -> Option<usize> {
    if addr < 0x0000_4000 {
        Some(addr as usize)
    } else {
        None
    }
}
