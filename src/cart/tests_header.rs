//! G7-header unit tests.
//!
//! Cited: GBATEK — GBA Cartridge Header
//!   https://problemkaputt.de/gbatek.htm#gbacartridgeheader

use super::header::{CartHeader, HEADER_MIN_LEN};

fn minimal_rom_with_fields() -> Vec<u8> {
    let mut rom = vec![0u8; HEADER_MIN_LEN];
    // ARM `B .` (self) at entry.
    rom[0..4].copy_from_slice(&0xEA00_0000u32.to_le_bytes());
    rom[0xA0..0xAC].copy_from_slice(b"TEST TITLE\0\0");
    rom[0xAC..0xB0].copy_from_slice(b"ATSE");
    rom[0xB0..0xB2].copy_from_slice(b"01");
    rom[0xB2] = 0x96;
    rom[0xBC] = 0x01;
    let c = CartHeader::compute_complement(&rom).unwrap();
    rom[0xBD] = c;
    rom
}

#[test]
fn parse_title_code_and_complement() {
    let rom = minimal_rom_with_fields();
    let h = CartHeader::parse(&rom).expect("header");
    assert_eq!(h.fixed_96, 0x96);
    assert_eq!(h.game_code_str(), "ATSE");
    assert!(h.title_str().starts_with("TEST TITLE"));
    assert!(h.checksum_ok(&rom));
    assert_eq!(h.entry, 0xEA00_0000);
}

#[test]
fn short_rom_returns_none() {
    assert!(CartHeader::parse(&[0u8; 0x40]).is_none());
}

#[test]
fn bad_complement_fails_checksum() {
    let mut rom = minimal_rom_with_fields();
    rom[0xBD] ^= 0xFF;
    let h = CartHeader::parse(&rom).unwrap();
    assert!(!h.checksum_ok(&rom));
}
