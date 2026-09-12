//! G7-sram unit tests (jsmolka save/sram + save/none semantics).
//!
//! Cited: jsmolka/gba-tests `save/sram.asm` / `save/none.asm` (MIT)
//!   https://github.com/jsmolka/gba-tests

use super::sram::{NoSave, Sram, SRAM_CHIP_SIZE};

#[test]
fn uninit_reads_ff() {
    let s = Sram::new(SRAM_CHIP_SIZE);
    assert_eq!(s.read8(0x0E00_0000), 0xFF);
    assert_eq!(NoSave::read8(0x0E00_0000), 0xFF);
}

#[test]
fn mirror_64k_and_16m() {
    let mut s = Sram::default();
    s.write8(0x0E00_0020, 0x42);
    assert_eq!(s.read8(0x0E01_0020), 0x42); // +0x10000 → same chip offset via & 0x7FFF
                                            // +0x01000000 lands in same 64K window indexing.
    assert_eq!(s.read8(0x0F00_0020), 0x42);
}

#[test]
fn chip_mirror_within_64k_field() {
    let mut s = Sram::default();
    s.write8(0x0E00_0001, 0x11);
    // Second half of 64 KiB field mirrors first 32 KiB.
    assert_eq!(s.read8(0x0E00_8001), 0x11);
}

#[test]
fn sav_roundtrip() {
    let mut s = Sram::default();
    s.write8(0x0E00_0100, 0xAB);
    let sav = s.to_sav();
    assert_eq!(sav.len(), SRAM_CHIP_SIZE);
    let s2 = Sram::from_sav(&sav);
    assert_eq!(s2.read8(0x0E00_0100), 0xAB);
}
