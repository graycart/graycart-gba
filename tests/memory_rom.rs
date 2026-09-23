use std::fs;
use std::path::Path;

use graycart_gba::Machine;

#[test]
fn memory_gba_reaches_idle_with_r12_clear() {
    let path = Path::new("tests/fixtures/jsmolka/memory/memory.gba");
    if !path.exists() {
        eprintln!("skip memory.gba");
        return;
    }
    let rom = fs::read(path).unwrap();
    let mut machine = Machine::from_rom(rom);
    machine.run_frames(30);
    let openbus = machine
        .bus
        .warn_lines
        .iter()
        .filter(|line| line.contains("openbus"))
        .count();
    assert!(
        machine.idle
            && machine.cpu.reg(12) == 0
            && machine.error.is_none()
            && machine.cpu.faults == 0
            && openbus == 0,
        "idle={} r12={} pc={:#010X} cpsr={:#010X} op={} fault={} err={:?} openbus={}",
        machine.idle,
        machine.cpu.reg(12),
        machine.cpu.exec_pc,
        machine.cpu.cpsr(),
        machine.cpu.last_op,
        machine.cpu.faults,
        machine.error.as_ref().map(|e| e.to_string()),
        openbus,
    );
}
