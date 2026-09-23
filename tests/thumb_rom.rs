use std::fs;
use std::path::Path;

use graycart_gba::Machine;

#[test]
fn thumb_gba_reaches_idle_with_r7_clear() {
    let path = Path::new("tests/fixtures/jsmolka/thumb/thumb.gba");
    if !path.exists() {
        eprintln!("skip thumb.gba");
        return;
    }
    let rom = fs::read(path).unwrap();
    let mut machine = Machine::from_rom(rom);
    machine.run_frames(30);
    assert!(
        machine.idle
            && machine.cpu.reg(7) == 0
            && machine.error.is_none()
            && machine.cpu.faults == 0,
        "idle={} r7={} r12={} pc={:#010X} cpsr={:#010X} op={} fault={} err={:?}",
        machine.idle,
        machine.cpu.reg(7),
        machine.cpu.reg(12),
        machine.cpu.exec_pc,
        machine.cpu.cpsr(),
        machine.cpu.last_op,
        machine.cpu.faults,
        machine.error.as_ref().map(|e| e.to_string()),
    );
}
