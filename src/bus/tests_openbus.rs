//! Unit tests for open-bus / BIOS-protect placeholders (`openbus.rs`).

use super::openbus::{
    bios_protect_read, empty_cart_rom_halfword, empty_cart_rom_word, pc_in_bios,
    unused_memory_open_bus, OpenBusKind, OpenBusState, BIOS_END,
};

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
    // (addr/2) & 0xFFFF
    assert_eq!(empty_cart_rom_halfword(0x0800_0000), 0x0000);
    assert_eq!(empty_cart_rom_halfword(0x0800_0002), 0x0001);
    assert_eq!(empty_cart_rom_halfword(0x0800_0004), 0x0002);
    assert_eq!(empty_cart_rom_halfword(0x0801_FFFE), 0xFFFF);

    let word = empty_cart_rom_word(0x0800_0000);
    assert_eq!(word & 0xFFFF, 0x0000);
    assert_eq!(word >> 16, 0x0001);
}
