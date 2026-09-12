//! G7-flash unit tests (jsmolka flash64 / flash128 command shape).
//!
//! Cited: jsmolka/gba-tests `save/flash.asm` (MIT)
//!   https://github.com/jsmolka/gba-tests

use super::flash::Flash;

fn program(f: &mut Flash, addr: u32, value: u8) {
    // AA/55/A0 then data — matches m_flash 0xA0 + strb.
    f.write8(0x0E00_5555, 0xAA);
    f.write8(0x0E00_2AAA, 0x55);
    f.write8(0x0E00_5555, 0xA0);
    f.write8(addr, value);
}

#[test]
fn uninit_ff() {
    let f = Flash::new_64k();
    assert_eq!(f.read8(0x0E00_0000), 0xFF);
}

#[test]
fn program_and_mirror() {
    let mut f = Flash::new_64k();
    program(&mut f, 0x0E00_0020, 0x01);
    assert_eq!(f.read8(0x0E00_0020), 0x01);
    assert_eq!(f.read8(0x0E01_0020), 0x01);
    assert_eq!(f.read8(0x0F00_0020), 0x01);
}

#[test]
fn chip_erase() {
    let mut f = Flash::new_64k();
    for i in 0..0x100u32 {
        program(&mut f, 0x0E00_0000 + i, 0x00);
    }
    assert_eq!(f.read8(0x0E00_0000), 0x00);
    // AA/55/80 + AA/55/10
    f.write8(0x0E00_5555, 0xAA);
    f.write8(0x0E00_2AAA, 0x55);
    f.write8(0x0E00_5555, 0x80);
    f.write8(0x0E00_5555, 0xAA);
    f.write8(0x0E00_2AAA, 0x55);
    f.write8(0x0E00_5555, 0x10);
    assert_eq!(f.read8(0x0E00_0000), 0xFF);
}

#[test]
fn sector_erase_4k() {
    let mut f = Flash::new_64k();
    program(&mut f, 0x0E00_0000, 0x00);
    program(&mut f, 0x0E00_1000, 0x00);
    f.write8(0x0E00_5555, 0xAA);
    f.write8(0x0E00_2AAA, 0x55);
    f.write8(0x0E00_5555, 0x80);
    f.write8(0x0E00_5555, 0xAA);
    f.write8(0x0E00_2AAA, 0x55);
    f.write8(0x0E00_0000, 0x30);
    assert_eq!(f.read8(0x0E00_0000), 0xFF);
    assert_eq!(f.read8(0x0E00_1000), 0x00);
}

#[test]
fn bank_switch_128k() {
    let mut f = Flash::new_128k();
    program(&mut f, 0x0E00_0040, 0x01);
    // Switch to bank 1.
    f.write8(0x0E00_5555, 0xAA);
    f.write8(0x0E00_2AAA, 0x55);
    f.write8(0x0E00_5555, 0xB0);
    f.write8(0x0E00_0000, 0x01);
    assert_ne!(f.read8(0x0E00_0040), 0x01);
    program(&mut f, 0x0E00_0040, 0x02);
    // Back to bank 0.
    f.write8(0x0E00_5555, 0xAA);
    f.write8(0x0E00_2AAA, 0x55);
    f.write8(0x0E00_5555, 0xB0);
    f.write8(0x0E00_0000, 0x00);
    assert_eq!(f.read8(0x0E00_0040), 0x01);
}
