//! SRAM / FRAM backup window (direct byte R/W).
//!
//! Cited: GBATEK — GBA Cart Backup SRAM/FRAM
//!   https://problemkaputt.de/gbatek.htm
//! Research: Project store `docs/graycart-gba/06-cart-bios-saves.md` §5.1
//! Note: uninitialized cells read as `0xFF` (jsmolka `save/sram` / `save/none`).

use crate::bus::mirror::sram_chip_offset;

/// Typical retail SRAM size (32 KiB); mirrored inside the 64 KiB field.
pub const SRAM_CHIP_SIZE: usize = 32 * 1024;

/// Direct-mapped SRAM/FRAM image (byte bus).
#[derive(Debug, Clone)]
pub struct Sram {
    pub data: Vec<u8>,
}

impl Default for Sram {
    fn default() -> Self {
        Self::new(SRAM_CHIP_SIZE)
    }
}

impl Sram {
    /// Allocate `size` bytes filled with `0xFF` (erased / uninit).
    #[must_use]
    pub fn new(size: usize) -> Self {
        Self {
            data: vec![0xFF; size],
        }
    }

    #[inline]
    fn index(addr: u32) -> usize {
        // 32 KiB chip mirrored through the 64 KiB window (and beyond via caller).
        sram_chip_offset(addr) % SRAM_CHIP_SIZE.max(1)
    }

    #[must_use]
    pub fn read8(&self, addr: u32) -> u8 {
        let i = Self::index(addr);
        self.data.get(i).copied().unwrap_or(0xFF)
    }

    pub fn write8(&mut self, addr: u32, value: u8) {
        let i = Self::index(addr);
        if let Some(slot) = self.data.get_mut(i) {
            *slot = value;
        }
    }

    /// Raw `.sav` bytes (chip image).
    #[must_use]
    pub fn to_sav(&self) -> Vec<u8> {
        self.data.clone()
    }

    /// Load `.sav` bytes (pads/truncates to chip size with `0xFF`).
    pub fn from_sav(bytes: &[u8]) -> Self {
        let mut s = Self::new(SRAM_CHIP_SIZE);
        let n = bytes.len().min(s.data.len());
        s.data[..n].copy_from_slice(&bytes[..n]);
        s
    }
}

/// No-backup cart: every backup-window read returns `0xFF`; writes ignored.
#[derive(Debug, Clone, Default)]
pub struct NoSave;

impl NoSave {
    #[must_use]
    pub const fn read8(_addr: u32) -> u8 {
        0xFF
    }

    pub const fn write8(_addr: u32, _value: u8) {}
}
