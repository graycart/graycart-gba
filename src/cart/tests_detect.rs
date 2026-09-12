//! G7-detect unit tests.
//!
//! Cited: jsmolka/gba-tests save-type ID strings (MIT)
//!   https://github.com/jsmolka/gba-tests

use super::detect::{detect_save_kind, SaveKind};

#[test]
fn detect_sram_string() {
    let mut rom = vec![0u8; 0x200];
    rom[0x100..0x108].copy_from_slice(b"SRAM_V  ");
    assert_eq!(detect_save_kind(&rom), SaveKind::Sram);
}

#[test]
fn detect_flash64_and_128() {
    let mut rom = vec![0u8; 0x200];
    rom[0x100..0x108].copy_from_slice(b"FLASH_V ");
    assert_eq!(detect_save_kind(&rom), SaveKind::Flash64);
    rom[0x100..0x109].copy_from_slice(b"FLASH1M_V");
    assert_eq!(detect_save_kind(&rom), SaveKind::Flash128);
}

#[test]
fn detect_none() {
    assert_eq!(detect_save_kind(&[0u8; 64]), SaveKind::None);
}

#[test]
fn detect_eeprom() {
    let mut rom = vec![0u8; 0x100];
    rom[0x40..0x48].copy_from_slice(b"EEPROM_V");
    assert_eq!(detect_save_kind(&rom), SaveKind::Eeprom512);
}
