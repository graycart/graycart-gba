use std::fs;
use std::path::Path;

use graycart_gba::{Machine, frame_hash};

#[test]
fn shades_gba_pixels_then_hash() {
    let path = Path::new("tests/fixtures/jsmolka/ppu/shades.gba");
    if !path.exists() {
        eprintln!("skip shades.gba");
        return;
    }
    let rom = fs::read(path).unwrap();
    let mut machine = Machine::from_rom(rom);
    machine.run_frames(5);
    assert!(machine.idle, "shades.gba should reach idle");
    assert!(machine.error.is_none(), "error={:?}", machine.error);
    assert_eq!(machine.cpu.faults, 0, "faults should be 0");

    let p00 = machine.ppu.pixels[0];
    let p160 = machine.ppu.pixels[16];
    assert_eq!(p00, 0, "pixel (0,0) must be tile 0 / color 0");
    assert_eq!(p160, 0x0800, "pixel (16,0) must be tile 1 / color 1");

    let hash = frame_hash(&machine.ppu.pixels);
    eprintln!("shades.gba frame hash={hash}");
    assert_eq!(
        hash,
        "da5336215e6c56d30146c42d81ad828a819214c307682903a5bcc87be8c0538e"
    );
}

#[test]
fn stripes_gba_pixels_then_hash() {
    let path = Path::new("tests/fixtures/jsmolka/ppu/stripes.gba");
    if !path.exists() {
        eprintln!("skip stripes.gba");
        return;
    }
    let rom = fs::read(path).unwrap();
    let mut machine = Machine::from_rom(rom);
    machine.run_frames(5);
    assert!(machine.idle, "stripes.gba should reach idle");
    assert!(machine.error.is_none(), "error={:?}", machine.error);
    assert_eq!(machine.cpu.faults, 0, "faults should be 0");

    let p00 = machine.ppu.pixels[0];
    let p80 = machine.ppu.pixels[8];
    assert_eq!(p00, 0x560B, "pixel (0,0) must be backdrop (empty tile 1)");
    assert_eq!(p80, 0x6290, "pixel (8,0) must be tile 0 color 1");

    let hash = frame_hash(&machine.ppu.pixels);
    eprintln!("stripes.gba frame hash={hash}");
    assert_eq!(
        hash,
        "4600c4f2749300b40f34dea969165d4d08d652543620076fb5cae249426f238d"
    );
}
