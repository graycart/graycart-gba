//! Save-type detection from ROM SDK strings.
//!
//! Cited: GBATEK — no header save field; SDK strings in ROM
//!   https://problemkaputt.de/gbatek.htm
//! Open-emu practice: scan for `EEPROM_V` / `SRAM_V` / `FLASH*` markers.
//! Research: Project store `docs/graycart-gba/06-cart-bios-saves.md` §5
//! Note: heuristics only — override API remains available for hosts.

use super::eeprom::EepromSize;

/// Detected backup memory kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SaveKind {
    None,
    Sram,
    Flash64,
    Flash128,
    Eeprom512,
    Eeprom8K,
}

impl SaveKind {
    #[must_use]
    pub const fn sav_size(self) -> usize {
        match self {
            Self::None => 0,
            Self::Sram => super::sram::SRAM_CHIP_SIZE,
            Self::Flash64 => super::flash::FLASH_64K,
            Self::Flash128 => super::flash::FLASH_128K,
            Self::Eeprom512 => EepromSize::B512.bytes(),
            Self::Eeprom8K => EepromSize::K8.bytes(),
        }
    }
}

/// Scan ROM for Nintendo SDK save-type ID strings (first match wins by priority).
///
/// Priority: Flash1M → Flash512/FLASH_V → EEPROM_V8 → EEPROM_V → SRAM_V → None.
#[must_use]
pub fn detect_save_kind(rom: &[u8]) -> SaveKind {
    if find_ascii(rom, b"FLASH1M_V") || find_ascii(rom, b"FLASH1M") {
        return SaveKind::Flash128;
    }
    if find_ascii(rom, b"FLASH512_V") || find_ascii(rom, b"FLASH512") || find_ascii(rom, b"FLASH_V")
    {
        // jsmolka flash64 embeds `FLASH_V` / `FLASH512_V`.
        return SaveKind::Flash64;
    }
    if find_ascii(rom, b"EEPROM_V122") || find_ascii(rom, b"EEPROM_V12") {
        // Larger EEPROMs often use V12x — treat as 8K when explicitly versioned that way.
        return SaveKind::Eeprom8K;
    }
    if find_ascii(rom, b"EEPROM_V") {
        return SaveKind::Eeprom512;
    }
    if find_ascii(rom, b"SRAM_V") || find_ascii(rom, b"SRAM_F_V") || find_ascii(rom, b"SRAM") {
        return SaveKind::Sram;
    }
    SaveKind::None
}

fn find_ascii(hay: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() || hay.len() < needle.len() {
        return false;
    }
    hay.windows(needle.len()).any(|w| w == needle)
}
