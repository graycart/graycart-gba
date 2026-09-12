//! Game Pak / saves / GPIO (ROM load surface for harness + host).
//!
//! Module layout from graycart-gba implementation plan §2.2.
//! Behavior: see research `docs/graycart-gba/06-cart-bios-saves.md` (saves TBD).
//!
//! Cited: GBATEK — GBA Cartridge Header / Memory Map
//!   https://problemkaputt.de/gbatek.htm
//! Note: never vendor commercial ROMs or Nintendo BIOS in-tree. Fixture ROMs
//! under `tests/fixtures/` keep upstream licenses.

/// Cartridge image holder (ROM bytes mirrored onto [`crate::bus::Bus::rom`]).
#[derive(Debug, Clone, Default)]
pub struct Cart {
    /// Raw Game Pak image (`.gba` / multiboot). Empty until [`Self::load`].
    pub rom: Vec<u8>,
}

impl Cart {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Replace the cart ROM image (does not touch bus — caller copies to [`crate::bus::Bus`]).
    pub fn load(&mut self, bytes: &[u8]) {
        self.rom = bytes.to_vec();
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rom.is_empty()
    }
}
