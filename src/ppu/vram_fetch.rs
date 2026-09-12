//! VRAM byte/halfword fetch with GBA mirror map (PPU path).
//!
//! Cited: GBATEK — GBA Unpredictable Things (VRAM mirrors)
//!   https://problemkaputt.de/gbatek-gba-unpredictable-things.htm
//! Cross-check: `crate::bus::mirror::vram_offset` (CPU path).
//! Note: PPU must mirror the 128 KiB window the same way CPU MMIO does.

/// Map a byte offset within `06000000` into the physical 96 KiB store.
#[inline]
#[must_use]
pub fn mirror_off(off: usize) -> usize {
    let mut o = off & 0x1_FFFF;
    if o >= 0x1_0000 {
        o = 0x1_0000 | (o & 0x7_FFF);
    }
    o
}

#[inline]
#[must_use]
pub fn byte(vram: &[u8], off: usize) -> u8 {
    *vram.get(mirror_off(off)).unwrap_or(&0)
}

#[inline]
#[must_use]
pub fn half(vram: &[u8], off: usize) -> u16 {
    let lo = u16::from(byte(vram, off));
    let hi = u16::from(byte(vram, off + 1));
    lo | (hi << 8)
}
