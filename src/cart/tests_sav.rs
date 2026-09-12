//! G7-sav unit tests.
//!
//! Cited: Project store `docs/graycart-gba/06-cart-bios-saves.md` §5

use super::detect::SaveKind;
use super::sav::sav_len;
use super::sram::SRAM_CHIP_SIZE;
use super::SaveBackend;

#[test]
fn sav_sizes() {
    assert_eq!(sav_len(SaveKind::None), 0);
    assert_eq!(sav_len(SaveKind::Sram), SRAM_CHIP_SIZE);
    assert_eq!(sav_len(SaveKind::Flash64), 64 * 1024);
    assert_eq!(sav_len(SaveKind::Flash128), 128 * 1024);
    assert_eq!(sav_len(SaveKind::Eeprom512), 512);
    assert_eq!(sav_len(SaveKind::Eeprom8K), 8 * 1024);
}

#[test]
fn backend_sav_roundtrip_sram() {
    let mut b = SaveBackend::for_kind(SaveKind::Sram);
    b.write8(0x0E00_0010, 0x5A);
    let sav = b.to_sav();
    assert_eq!(sav.len(), SRAM_CHIP_SIZE);
    let mut b2 = SaveBackend::None;
    b2.load_sav(SaveKind::Sram, &sav);
    assert_eq!(b2.read8(0x0E00_0010), 0x5A);
}
