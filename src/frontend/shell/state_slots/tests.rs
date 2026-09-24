use super::*;
use graycart::{Bus, Cpu, apply_fast, capture};
use std::fs;
use std::path::Path;
use tempfile::tempdir;

#[test]
fn slot_path_uses_rom_stem() {
    let p = slot_path(Path::new("/games/pokemon_red.gb"), 3);
    assert_eq!(p, Path::new("/games/pokemon_red.gcs3"));
}

#[test]
fn save_and_load_roundtrip_slot_zero() {
    let dir = tempdir().unwrap();
    let rom_path = dir.path().join("test.gb");
    let rom = graycart::Cartridge::rom_only(vec![0u8; 0x8000]).rom;
    fs::write(&rom_path, &rom).unwrap();

    let mut cpu = Cpu::new();
    let mut bus = Bus::from_rom(rom.clone());
    apply_fast(&mut cpu, &mut bus);
    let state = capture(&cpu, &bus);

    save_slot(&rom, &rom_path, "TESTGAME", 0, &state).unwrap();
    let (loaded, meta) = load_slot(&rom, &rom_path, 0).unwrap();
    assert_eq!(loaded, state);
    assert_eq!(meta.title, "TESTGAME");

    let m = slot_meta(&rom_path, 0).unwrap();
    assert!(m.exists);
    assert_eq!(m.slot, 0);
    assert_eq!(m.title, "TESTGAME");
    assert!(m.timestamp > 0);
}
