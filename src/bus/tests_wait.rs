//! Unit tests for WAITCNT decoding and waitstate cycle helpers.
//!
//! Cited: GBATEK -- GBA System Control (WAITCNT) / Gamepak waitstates
//!   https://problemkaputt.de/gbatek-gba-system-control.htm

use super::wait::{
    cycles_for_waits, rom_force_nonseq, AccessKind, AccessSize, RomWindow, WaitTables,
    EWRAM_DEFAULT_WAITS,
};
use super::waitcnt::{self, WaitCnt};

#[test]
fn power_on_waitcnt_is_zero() {
    let wc = WaitCnt::power_on();
    assert_eq!(wc.raw(), waitcnt::POWER_ON);
    assert_eq!(wc.sram_waits(), 4);
    assert_eq!(wc.ws0_n_waits(), 4);
    assert_eq!(wc.ws0_s_waits(), 2);
    assert_eq!(wc.ws1_n_waits(), 4);
    assert_eq!(wc.ws1_s_waits(), 4);
    assert_eq!(wc.ws2_n_waits(), 4);
    assert_eq!(wc.ws2_s_waits(), 8);
    assert!(!wc.prefetch_enable());
    assert!(!wc.cart_type_cgb());
}

#[test]
fn power_on_tables_match_default_rom_sram_ewram() {
    let t = WaitTables::power_on();
    // Access time = 1 + waits → ROM 5/5/8, SRAM 5, EWRAM 3/3/6.
    assert_eq!(t.rom_half_cycles(RomWindow::Ws0, AccessKind::Nonseq), 5);
    assert_eq!(t.rom_half_cycles(RomWindow::Ws0, AccessKind::Seq), 3);
    assert_eq!(t.rom_word_cycles(RomWindow::Ws0, AccessKind::Nonseq), 8);
    assert_eq!(t.rom_word_cycles(RomWindow::Ws0, AccessKind::Seq), 6);
    assert_eq!(t.sram_byte_cycles(), 5);
    assert_eq!(t.ewram_waits, EWRAM_DEFAULT_WAITS);
    assert_eq!(t.ewram_cycles(AccessSize::Byte), 3);
    assert_eq!(t.ewram_cycles(AccessSize::Half), 3);
    assert_eq!(t.ewram_cycles(AccessSize::Word), 6);
}

#[test]
fn commercial_4317_decodes_ws0_sram_ws2_prefetch() {
    let wc = WaitCnt::from_u16(waitcnt::COMMERCIAL_COMMON);
    assert_eq!(wc.raw(), 0x4317);
    // SRAM bits 0–1 = 3 → 8 waits.
    assert_eq!(wc.sram_waits(), 8);
    // WS0 N bits 2–3 = 1 → 3 waits; S bit 4 = 1 → 1 wait.
    assert_eq!(wc.ws0_n_waits(), 3);
    assert_eq!(wc.ws0_s_waits(), 1);
    // WS2 N bits 8–9 = 3 → 8; S bit 10 = 0 → 8.
    assert_eq!(wc.ws2_n_waits(), 8);
    assert_eq!(wc.ws2_s_waits(), 8);
    assert!(wc.prefetch_enable());

    let t = wc.to_tables();
    assert_eq!(t.rom_half_cycles(RomWindow::Ws0, AccessKind::Nonseq), 4);
    assert_eq!(t.rom_half_cycles(RomWindow::Ws0, AccessKind::Seq), 2);
    assert_eq!(t.rom_word_cycles(RomWindow::Ws0, AccessKind::Nonseq), 6);
    assert_eq!(t.sram_byte_cycles(), 9);
}

#[test]
fn write_preserves_cart_type_ro_and_clears_unused_bit13() {
    let mut wc = WaitCnt::power_on();
    wc.set_cart_type_cgb(true);
    wc.write(0xFFFF);
    // Writable 0–12 + 14 applied; bit 13 cleared; bit 15 preserved.
    assert_eq!(wc.raw(), 0x8000 | 0x5FFF);
    assert!(wc.cart_type_cgb());
    assert!(wc.prefetch_enable());

    wc.write(0x0000);
    assert_eq!(wc.raw(), 0x8000);
    assert!(wc.cart_type_cgb());
    assert!(!wc.prefetch_enable());
}

#[test]
fn prefetch_enable_bit_roundtrips() {
    let mut wc = WaitCnt::power_on();
    assert!(!wc.prefetch_enable());
    wc.set_prefetch_enable(true);
    assert!(wc.prefetch_enable());
    assert_eq!(wc.raw() & (1 << 14), 1 << 14);
    wc.set_prefetch_enable(false);
    assert!(!wc.prefetch_enable());
}

#[test]
fn waitcnt_mutates_rom_sram_leaves_ewram() {
    let mut tables = WaitTables::power_on();
    assert_eq!(tables.ewram_waits, 2);

    let mut wc = WaitCnt::power_on();
    // SRAM encoding 3 → 8 waits; WS1 N encoding 2 → 2 waits; WS1 S bit → 1 wait.
    wc.write(0x00C3); // bits0–1=3, bits5–6=2, bit7=1
    tables.apply_waitcnt(wc);

    assert_eq!(tables.sram_waits, 8);
    assert_eq!(tables.ws1_n, 2);
    assert_eq!(tables.ws1_s, 1);
    assert_eq!(tables.ewram_waits, 2);
}

#[test]
fn n_and_s_encoding_tables() {
    // N encodings 0..3 → 4,3,2,8 for each window field.
    for (enc, waits) in [(0u16, 4u8), (1, 3), (2, 2), (3, 8)] {
        let wc = WaitCnt::from_u16(enc | (enc << 2) | (enc << 5) | (enc << 8));
        assert_eq!(wc.sram_waits(), waits);
        assert_eq!(wc.ws0_n_waits(), waits);
        assert_eq!(wc.ws1_n_waits(), waits);
        assert_eq!(wc.ws2_n_waits(), waits);
    }

    assert_eq!(WaitCnt::from_u16(0).ws0_s_waits(), 2);
    assert_eq!(WaitCnt::from_u16(1 << 4).ws0_s_waits(), 1);
    assert_eq!(WaitCnt::from_u16(0).ws1_s_waits(), 4);
    assert_eq!(WaitCnt::from_u16(1 << 7).ws1_s_waits(), 1);
    assert_eq!(WaitCnt::from_u16(0).ws2_s_waits(), 8);
    assert_eq!(WaitCnt::from_u16(1 << 10).ws2_s_waits(), 1);
}

#[test]
fn rom_128kib_forced_n_helper() {
    assert!(rom_force_nonseq(0x0800_0000));
    assert!(rom_force_nonseq(0x0802_0000));
    assert!(rom_force_nonseq(0x0A02_0000));
    assert!(rom_force_nonseq(0x0C04_0000));
    assert!(!rom_force_nonseq(0x0800_0002));
    assert!(!rom_force_nonseq(0x0801_FFFE));
    assert!(!rom_force_nonseq(0x0802_0002));
}

#[test]
fn rom_half_cycles_at_forces_n_on_boundary() {
    let t = WaitTables::power_on();
    // Even if caller claims Seq, boundary is priced as N (5 cycles @ power-on).
    assert_eq!(
        t.rom_half_cycles_at(0x0802_0000, RomWindow::Ws0, AccessKind::Seq),
        5
    );
    assert_eq!(
        t.rom_half_cycles_at(0x0802_0002, RomWindow::Ws0, AccessKind::Seq),
        3
    );
}

#[test]
fn classify_access_seq_and_force_n() {
    use super::wait::classify_access;

    assert_eq!(
        classify_access(0x0800_0000, AccessSize::Half, 0x0800_0002, true),
        AccessKind::Seq
    );
    assert_eq!(
        classify_access(0x0801_FFFE, AccessSize::Half, 0x0802_0000, true),
        AccessKind::Nonseq
    );
    assert_eq!(
        classify_access(0x0800_0000, AccessSize::Half, 0x0800_0010, true),
        AccessKind::Nonseq
    );
    assert_eq!(
        classify_access(0x0300_0000, AccessSize::Word, 0x0300_0004, false),
        AccessKind::Nonseq
    );
}

#[test]
fn cycles_for_waits_formula() {
    assert_eq!(cycles_for_waits(0), 1);
    assert_eq!(cycles_for_waits(4), 5);
    assert_eq!(cycles_for_waits(8), 9);
}

#[test]
fn rom_window_from_addr() {
    assert_eq!(RomWindow::from_addr(0x0800_0000), Some(RomWindow::Ws0));
    assert_eq!(RomWindow::from_addr(0x09FF_FFFE), Some(RomWindow::Ws0));
    assert_eq!(RomWindow::from_addr(0x0A00_0000), Some(RomWindow::Ws1));
    assert_eq!(RomWindow::from_addr(0x0C00_0000), Some(RomWindow::Ws2));
    assert_eq!(RomWindow::from_addr(0x0200_0000), None);
}

#[test]
fn fixed_and_video_defaults() {
    assert_eq!(WaitTables::fixed_fast_cycles(AccessSize::Word), 1);
    assert_eq!(WaitTables::video_ram_cycles(AccessSize::Half), 1);
    assert_eq!(WaitTables::video_ram_cycles(AccessSize::Word), 2);
}
