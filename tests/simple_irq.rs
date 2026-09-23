use std::fs;
use std::path::Path;

use graycart_gba::Machine;

#[test]
fn simple_irq_rom_when_present() {
    let path = Path::new("tests/fixtures/simple-irq.gba");
    if !path.exists() {
        eprintln!("skip simple-irq.gba (fixture absent)");
        return;
    }
    let rom = fs::read(path).unwrap();
    let mut machine = Machine::from_rom(rom);
    machine.run_frames(30);
    assert!(
        machine.idle && machine.cpu.reg(12) == 0 && machine.error.is_none(),
        "idle={} r12={} error={:?} pc={:#010X} cpsr={:#010X}",
        machine.idle,
        machine.cpu.reg(12),
        machine.error,
        machine.cpu.exec_pc,
        machine.cpu.cpsr(),
    );
}
