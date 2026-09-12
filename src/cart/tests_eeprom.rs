//! G7-eeprom unit tests.
//!
//! Cited: GBATEK — GBA Cart Backup EEPROM
//!   https://problemkaputt.de/gbatek.htm

use super::eeprom::{Eeprom, EepromSize, EEPROM_512, EEPROM_8K};

#[test]
fn sizes() {
    assert_eq!(Eeprom::new(EepromSize::B512).data.len(), EEPROM_512);
    assert_eq!(Eeprom::new(EepromSize::K8).data.len(), EEPROM_8K);
    assert_eq!(EepromSize::B512.addr_bits(), 6);
    assert_eq!(EepromSize::K8.addr_bits(), 14);
}

#[test]
fn block_roundtrip() {
    let mut e = Eeprom::new(EepromSize::B512);
    let data = [0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF];
    e.write_block(3, &data);
    assert_eq!(e.read_block(3), data);
}

#[test]
fn serial_write_then_read_512() {
    let mut e = Eeprom::new(EepromSize::B512);
    // Write cmd 0b10, addr 0 (6 bits), 64 data bits (0x0123456789ABCDEF), stop 0.
    // Stream MSB first: 1 0 | 000000 | <64 bits> | 0
    let mut bits: Vec<u8> = vec![1, 0]; // write
    bits.extend([0; 6]); // addr 0
    let payload: u64 = 0x0123_4567_89AB_CDEF;
    for i in (0..64).rev() {
        bits.push(((payload >> i) & 1) as u8);
    }
    bits.push(0); // stop
    for b in bits {
        e.ingest_bit(b);
    }
    assert_eq!(
        e.read_block(0),
        [0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF]
    );

    // Read cmd 0b11, addr 0, stop 0.
    for b in [1u8, 1, 0, 0, 0, 0, 0, 0, 0] {
        e.ingest_bit(b);
    }
    // 4 dummy + 64 data
    let mut out = 0u64;
    for _ in 0..4 {
        assert_eq!(e.clock_read(), 0);
    }
    for _ in 0..64 {
        out = (out << 1) | u64::from(e.clock_read());
    }
    assert_eq!(out, payload);
}

#[test]
fn sav_roundtrip() {
    let mut e = Eeprom::new(EepromSize::K8);
    e.write_block(1, &[1, 2, 3, 4, 5, 6, 7, 8]);
    let sav = e.to_sav();
    let e2 = Eeprom::from_sav(EepromSize::K8, &sav);
    assert_eq!(e2.read_block(1), [1, 2, 3, 4, 5, 6, 7, 8]);
}
