use std::fs;
use std::path::Path;

use graycart_gba::{frame_hash, Machine};

#[test]
fn hello_gba_settled_frame_hash() {
    let path = Path::new("tests/fixtures/jsmolka/ppu/hello.gba");
    if !path.exists() {
        eprintln!("skip hello.gba");
        return;
    }
    let rom = fs::read(path).unwrap();
    let mut machine = Machine::from_rom(rom);
    machine.run_frames(5);
    assert!(machine.idle, "hello.gba should reach idle");
    assert!(machine.error.is_none(), "error={:?}", machine.error);
    assert_eq!(machine.cpu.faults, 0, "faults should be 0");
    assert!(
        machine.ppu.nonzero() > 0,
        "blank frame must not pass; nonzero={}",
        machine.ppu.nonzero()
    );
    let hash = frame_hash(&machine.ppu.pixels);
    eprintln!("hello.gba frame hash={hash}");
    assert_eq!(
        hash,
        "56cd131fb3915fe7e410be228a8c09e99132064799f148583636ca75745bedf7"
    );
}
