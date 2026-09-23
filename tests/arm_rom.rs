use std::fs;
use std::path::Path;

use graycart_gba::Machine;

#[test]
fn arm_gba_reaches_idle_with_r12_clear() {
    let path = Path::new("tests/fixtures/jsmolka/arm/arm.gba");
    if !path.exists() {
        eprintln!("skip arm.gba");
        return;
    }
    let rom = fs::read(path).unwrap();
    let mut machine = Machine::from_rom(rom);
    machine.run_frames(30);
    assert!(
        machine.idle && machine.cpu.reg(12) == 0,
        "idle={} r12={} pc={:#010X} cpsr={:#010X} op={} fault={}",
        machine.idle,
        machine.cpu.reg(12),
        machine.cpu.exec_pc,
        machine.cpu.cpsr(),
        machine.cpu.last_op,
        machine.cpu.faults,
    );
}
