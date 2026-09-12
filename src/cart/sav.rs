//! `.sav` serialize helpers (raw sizes compatible with common emulators).
//!
//! Cited: GBATEK — backup sizes; mGBA/VBA raw `.sav` practice (secondary)
//! Research: Project store `docs/graycart-gba/06-cart-bios-saves.md` §5
//! Note: raw dump only — no footer magic.

use super::detect::SaveKind;
use super::eeprom::{Eeprom, EepromSize};
use super::flash::Flash;
use super::sram::Sram;
use super::SaveBackend;

impl SaveBackend {
    /// Raw `.sav` bytes for the active backend (`None` → empty).
    #[must_use]
    pub fn to_sav(&self) -> Vec<u8> {
        match self {
            Self::None => Vec::new(),
            Self::Sram(s) => s.to_sav(),
            Self::Flash(f) => f.to_sav(),
            Self::Eeprom(e) => e.to_sav(),
        }
    }

    /// Replace backend storage from raw `.sav` bytes (size must match kind).
    pub fn load_sav(&mut self, kind: SaveKind, bytes: &[u8]) {
        *self = match kind {
            SaveKind::None => Self::None,
            SaveKind::Sram => Self::Sram(Sram::from_sav(bytes)),
            SaveKind::Flash64 => Self::Flash(Flash::from_sav_64k(bytes)),
            SaveKind::Flash128 => Self::Flash(Flash::from_sav_128k(bytes)),
            SaveKind::Eeprom512 => Self::Eeprom(Eeprom::from_sav(EepromSize::B512, bytes)),
            SaveKind::Eeprom8K => Self::Eeprom(Eeprom::from_sav(EepromSize::K8, bytes)),
        };
    }
}

/// Expected raw `.sav` length for `kind` (0 for [`SaveKind::None`]).
#[must_use]
pub fn sav_len(kind: SaveKind) -> usize {
    kind.sav_size()
}
