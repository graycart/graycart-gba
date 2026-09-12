//! Game Pak / saves / GPIO (ROM load + backup backends for P7).
//!
//! Module layout from graycart-gba implementation plan §2.2.
//! Behavior: see research `docs/graycart-gba/06-cart-bios-saves.md`.
//!
//! Cited: GBATEK — GBA Cartridge Header / Memory Map / Cart Backup
//!   https://problemkaputt.de/gbatek.htm
//! Note: never vendor commercial ROMs or Nintendo BIOS in-tree. Fixture ROMs
//! under `tests/fixtures/` keep upstream licenses.

pub mod detect;
pub mod eeprom;
pub mod flash;
pub mod header;
pub mod sav;
pub mod sram;

#[cfg(test)]
mod tests_detect;
#[cfg(test)]
mod tests_eeprom;
#[cfg(test)]
mod tests_flash;
#[cfg(test)]
mod tests_header;
#[cfg(test)]
mod tests_sav;
#[cfg(test)]
mod tests_sram;

use detect::{detect_save_kind, SaveKind};
use eeprom::{Eeprom, EepromSize};
use flash::Flash;
use header::CartHeader;
use sram::{NoSave, Sram};

/// Active backup backend for the `0x0E000000` window (and EEPROM image).
#[derive(Debug, Clone, Default)]
pub enum SaveBackend {
    #[default]
    None,
    Sram(Sram),
    Flash(Flash),
    Eeprom(Eeprom),
}

impl SaveBackend {
    /// Build a backend for `kind` with erased (`0xFF`) storage.
    #[must_use]
    pub fn for_kind(kind: SaveKind) -> Self {
        match kind {
            SaveKind::None => Self::None,
            SaveKind::Sram => Self::Sram(Sram::default()),
            SaveKind::Flash64 => Self::Flash(Flash::new_64k()),
            SaveKind::Flash128 => Self::Flash(Flash::new_128k()),
            SaveKind::Eeprom512 => Self::Eeprom(Eeprom::new(EepromSize::B512)),
            SaveKind::Eeprom8K => Self::Eeprom(Eeprom::new(EepromSize::K8)),
        }
    }

    #[must_use]
    pub fn kind(&self) -> SaveKind {
        match self {
            Self::None => SaveKind::None,
            Self::Sram(_) => SaveKind::Sram,
            Self::Flash(f) if f.size > flash::FLASH_64K => SaveKind::Flash128,
            Self::Flash(_) => SaveKind::Flash64,
            Self::Eeprom(e) => match e.size {
                EepromSize::B512 => SaveKind::Eeprom512,
                EepromSize::K8 => SaveKind::Eeprom8K,
            },
        }
    }

    #[must_use]
    pub fn read8(&self, addr: u32) -> u8 {
        match self {
            Self::None => NoSave::read8(addr),
            Self::Sram(s) => s.read8(addr),
            Self::Flash(f) => f.read8(addr),
            // EEPROM is not on the SRAM window — open/high.
            Self::Eeprom(_) => 0xFF,
        }
    }

    pub fn write8(&mut self, addr: u32, value: u8) {
        match self {
            Self::None => NoSave::write8(addr, value),
            Self::Sram(s) => s.write8(addr, value),
            Self::Flash(f) => f.write8(addr, value),
            Self::Eeprom(_) => {}
        }
    }
}

/// Cartridge image holder (ROM bytes mirrored onto [`crate::bus::Bus::rom`]).
#[derive(Debug, Clone, Default)]
pub struct Cart {
    /// Raw Game Pak image (`.gba` / multiboot). Empty until [`Self::load`].
    pub rom: Vec<u8>,
    /// Detected / configured backup backend.
    pub save: SaveBackend,
    /// Last parsed header (if ROM was long enough).
    pub header: Option<CartHeader>,
}

impl Cart {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Replace the cart ROM image (does not touch bus — caller copies to [`crate::bus::Bus`]).
    pub fn load(&mut self, bytes: &[u8]) {
        self.rom = bytes.to_vec();
        self.header = CartHeader::parse(&self.rom);
        let kind = detect_save_kind(&self.rom);
        self.save = SaveBackend::for_kind(kind);
    }

    /// Force a save kind (host override / tests).
    pub fn set_save_kind(&mut self, kind: SaveKind) {
        self.save = SaveBackend::for_kind(kind);
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rom.is_empty()
    }

    #[must_use]
    pub fn save_kind(&self) -> SaveKind {
        self.save.kind()
    }
}
