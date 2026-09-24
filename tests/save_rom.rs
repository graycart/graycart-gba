use std::fs;
use std::path::Path;

use graycart_gba::Machine;

fn run_save_rom(rel: &str, kind: &str) {
    let path = Path::new(rel);
    if !path.exists() {
        eprintln!("skip {}", path.display());
        return;
    }
    let rom = fs::read(path).unwrap();
    let mut machine = Machine::from_rom(rom);
    assert_eq!(
        machine.bus.save_kind().name(),
        kind,
        "save kind for {}",
        path.display()
    );
    machine.run_frames(60);
    assert!(
        machine.idle && machine.cpu.reg(12) == 0 && machine.error.is_none(),
        "idle={} r12={} pc={:#010X} cpsr={:#010X} op={} fault={} err={:?}",
        machine.idle,
        machine.cpu.reg(12),
        machine.cpu.exec_pc,
        machine.cpu.cpsr(),
        machine.cpu.last_op,
        machine.cpu.faults,
        machine.error,
    );
}

#[test]
fn save_none_gba() {
    run_save_rom("tests/fixtures/jsmolka/save/none.gba", "none");
}

#[test]
fn save_sram_gba() {
    run_save_rom("tests/fixtures/jsmolka/save/sram.gba", "sram");
}

#[test]
fn save_flash64_gba() {
    run_save_rom("tests/fixtures/jsmolka/save/flash64.gba", "flash64");
}

#[test]
fn save_flash128_gba() {
    run_save_rom("tests/fixtures/jsmolka/save/flash128.gba", "flash128");
}
