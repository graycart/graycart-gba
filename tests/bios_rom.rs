use std::fs;
use std::path::Path;

use graycart_gba::Machine;

#[test]
fn bios_gba_reaches_idle_with_r12_clear() {
    let path = Path::new("tests/fixtures/jsmolka/bios/bios.gba");
    if !path.exists() {
        eprintln!("skip bios.gba");
        return;
    }
    let rom = fs::read(path).unwrap();
    let mut machine = Machine::from_rom(rom);
    machine.run_frames(30);
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
