//! GAS1 snapshot encode/decode and restore tests (no frontend).

use super::*;
use crate::cart::{read_sidecar, sidecar_path, write_sidecar};
use crate::hw::Machine;
use std::fs;

fn temp_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "graycart-gba-snapshot-{tag}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn minimal_rom() -> Vec<u8> {
    // Header must be at least 0xC0; no save ID → SaveKind::None.
    vec![0u8; 0x200]
}

fn sram_rom() -> Vec<u8> {
    let mut rom = vec![0u8; 0x200];
    rom.extend_from_slice(b"SRAM_V123");
    rom
}

#[test]
fn gas1_roundtrip_encode_decode() {
    let rom = minimal_rom();
    let machine = Machine::from_rom(rom.clone());
    let state = machine.capture_state();
    let bytes = encode_gas1(&rom, "TESTGAME", &state).expect("encode");
    let (decoded, meta) = decode_gas1(&rom, &bytes).expect("decode");
    assert_eq!(meta.title, "TESTGAME");
    assert_eq!(decoded.cycles, state.cycles);
    assert_eq!(decoded.arm_steps, state.arm_steps);
    assert_eq!(decoded.cpu.cpsr(), state.cpu.cpsr());
    assert!(decoded.bus.rom.is_empty(), "snapshot ROM must stay cleared");
}

#[test]
fn gas1_rejects_wrong_rom_sha() {
    let rom_a = minimal_rom();
    let mut rom_b = minimal_rom();
    rom_b[0] = 1;
    let machine = Machine::from_rom(rom_a.clone());
    let state = machine.capture_state();
    let bytes = encode_gas1(&rom_a, "TEST", &state).expect("encode");
    assert!(matches!(
        decode_gas1(&rom_b, &bytes),
        Err(Gas1Error::RomMismatch)
    ));
}

#[test]
fn restore_state_keeps_rom_and_sidecar_path() {
    let dir = temp_dir("restore");
    let rom_path = dir.join("game.gba");
    let rom = sram_rom();
    fs::write(&rom_path, &rom).unwrap();
    let sav = sidecar_path(&rom_path);
    write_sidecar(&sav, &[0xAA; 64]).unwrap();

    let mut machine = Machine::open(&rom_path).expect("open");
    assert_eq!(
        machine.sidecar_path(),
        Some(sav.as_path()),
        "open must bind the .sav sidecar"
    );
    let live_rom = machine.bus.rom.clone();
    assert!(!live_rom.is_empty());

    let snap = machine.capture_state();
    assert!(snap.bus.rom.is_empty());
    machine.cycles = 123_456;
    // Prove restore does not install the cleared snapshot ROM.
    machine.bus.rom = vec![0xDE, 0xAD, 0xBE, 0xEF];

    machine.restore_state(&snap);
    assert_eq!(
        machine.bus.rom,
        vec![0xDE, 0xAD, 0xBE, 0xEF],
        "restore must keep the live ROM image present at restore time"
    );
    assert!(
        !machine.bus.rom.is_empty(),
        "must not adopt the empty snapshot ROM"
    );
    assert_eq!(
        machine.sidecar_path(),
        Some(sav.as_path()),
        "restore must keep the sidecar path"
    );
    assert_eq!(machine.cycles, snap.cycles);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn corrupt_gas_load_does_not_restore_or_touch_sav() {
    let dir = temp_dir("corrupt-gas");
    let rom_path = dir.join("game.gba");
    let rom = sram_rom();
    fs::write(&rom_path, &rom).unwrap();
    let sav = sidecar_path(&rom_path);
    let sav_payload = vec![0x55; 128];
    write_sidecar(&sav, &sav_payload).unwrap();

    let machine = Machine::open(&rom_path).expect("open");
    let cycles_before = machine.cycles;
    let rom_before = machine.bus.rom.clone();

    let gas_path = rom_path.with_extension("gas0");
    fs::write(&gas_path, b"not-a-gas1-file").unwrap();
    let gas_bytes = fs::read(&gas_path).unwrap();
    assert!(
        decode_gas1(&rom, &gas_bytes).is_err(),
        "corrupt GAS1 must be rejected"
    );
    // Caller must not restore on error — machine and .sav stay as they were.
    assert_eq!(machine.cycles, cycles_before);
    assert_eq!(machine.bus.rom, rom_before);
    assert_eq!(read_sidecar(&sav).unwrap(), sav_payload);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn clean_flush_skips_disk_when_save_not_dirty() {
    let dir = temp_dir("clean-flush");
    let rom_path = dir.join("game.gba");
    let rom = sram_rom();
    fs::write(&rom_path, &rom).unwrap();
    let sav = sidecar_path(&rom_path);
    write_sidecar(&sav, &[0x11; 32]).unwrap();

    let mut machine = Machine::open(&rom_path).expect("open");
    assert!(!machine.bus.save_dirty());
    // Marker: if flush wrote, this would be replaced by chip contents (0xFF fill).
    machine.flush_save().unwrap();
    assert_eq!(
        read_sidecar(&sav).unwrap(),
        vec![0x11; 32],
        "clean flush must not rewrite the sidecar"
    );

    // Dirty path still persists.
    machine.bus.mark_save_dirty();
    machine.flush_save().unwrap();
    assert!(!machine.bus.save_dirty());
    let after = read_sidecar(&sav).unwrap();
    assert_ne!(after, vec![0x11; 32]);
    assert!(!after.is_empty());

    let _ = fs::remove_dir_all(&dir);
}
