//! Unit tests for open-bus / BIOS-protect (`openbus.rs` + bus wiring).
//!
//! Cited: jsmolka/gba-tests `bios/bios.asm` SoftReset residue (MIT)
//!   https://github.com/jsmolka/gba-tests

use super::openbus::{
    bios_protect_byte, bios_protect_read, empty_cart_rom_halfword, empty_cart_rom_word, pc_in_bios,
    unused_memory_open_bus, OpenBusKind, OpenBusState, BIOS_END,
};
use super::{Bus, CpuMem};
use crate::bios::LATCH_SOFT_RESET;

#[test]
fn pc_in_bios_window() {
    assert!(pc_in_bios(0));
    assert!(pc_in_bios(BIOS_END - 1));
    assert!(!pc_in_bios(BIOS_END));
    assert!(!pc_in_bios(0x0200_0000));
}

#[test]
fn bios_protect_returns_latch_outside_bios() {
    let mut state = OpenBusState::new();
    state.note_bios_fetch(0xE1A0_0000);
    assert_eq!(bios_protect_read(0x0800_0000, &state), Some(0xE1A0_0000));
    // Inside BIOS: caller should read ROM; helper declines.
    assert_eq!(bios_protect_read(0x0000_0100, &state), None);
}

#[test]
fn bus_protect_wired_for_outside_pc() {
    let mut bus = Bus::new();
    bus.cpu_pc = 0x0800_0000;
    bus.open_bus.note_bios_fetch(LATCH_SOFT_RESET);
    assert_eq!(bus.read32(0), LATCH_SOFT_RESET);
    assert_eq!(
        bios_protect_byte(LATCH_SOFT_RESET, 0),
        LATCH_SOFT_RESET as u8
    );
    // Inside BIOS PC: raw image (empty → 0).
    bus.cpu_pc = 0x0000_0100;
    assert_eq!(bus.read8(0), 0);
}

#[test]
fn empty_sram_reads_ff() {
    let mut bus = Bus::new();
    assert!(bus.sram.is_empty());
    assert_eq!(bus.read8(0x0E00_0000), 0xFF);
}

#[test]
fn unused_memory_open_bus_is_tbd_placeholder() {
    assert_eq!(
        unused_memory_open_bus(OpenBusKind::UnusedMemory, 0x0800_0000, 0x0000_4000, false),
        None
    );
    assert_eq!(
        unused_memory_open_bus(OpenBusKind::UnusedIo, 0x0800_0000, 0x0400_0400, true),
        None
    );
}

#[test]
fn empty_cart_rom_pattern() {
    assert_eq!(empty_cart_rom_halfword(0x0800_0000), 0x0000);
    assert_eq!(empty_cart_rom_halfword(0x0800_0002), 0x0001);
    assert_eq!(empty_cart_rom_halfword(0x0800_0004), 0x0002);
    assert_eq!(empty_cart_rom_halfword(0x0801_FFFE), 0xFFFF);

    let word = empty_cart_rom_word(0x0800_0000);
    assert_eq!(word & 0xFFFF, 0x0000);
    assert_eq!(word >> 16, 0x0001);
}
