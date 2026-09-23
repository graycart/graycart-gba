use super::Machine;

#[test]
fn one_timer_irq_taken() {
    let mut machine = Machine::from_rom(vec![0; 0xC0]);
    machine.bus.irq.write16(0, 1 << 3);
    machine.bus.irq.write16(8, 1);
    // Reload 0xFFFF, then start|irq|prescaler F/1.
    machine.bus.timers.write16(0, 0xFFFF);
    machine.bus.timers.write16(2, 0x80 | (1 << 6));

    let mut taken = false;
    for _ in 0..64 {
        machine.run_cycles(1);
        if machine.cpu.cpsr() & 0x1F == 0x12 {
            assert_eq!(machine.cpu.fetch_pc, 0x18);
            taken = true;
            break;
        }
    }
    assert!(taken, "IRQ mode never entered");
    assert!(!machine.cpu.idle);
}

#[test]
fn halt_wakes_on_timer_irq() {
    let mut machine = Machine::from_rom(vec![0; 0xC0]);
    machine.bus.irq.write16(0, 1 << 3);
    machine.bus.irq.write16(8, 1);
    machine.bus.timers.write16(0, 0xFFFF);
    machine.bus.timers.write16(2, 0x80 | (1 << 6));
    // HALTCNT byte 0 — same flag as SWI 2.
    machine.bus.write8(0x0400_0301, 0);
    assert!(machine.bus.halted);
    assert_eq!(machine.cpu.cpsr() & 0x80, 0);

    machine.run_cycles(32);
    assert!(!machine.bus.halted, "halt should clear on wake");
    assert_eq!(
        machine.cpu.cpsr() & 0x1F,
        0x12,
        "IRQ should be taken on wake"
    );
}

#[test]
fn halt_via_swi_2() {
    // ARM SWI with comment bits 16..23 = 2 → number 2.
    let mut rom = vec![0u8; 0xC0];
    rom[0..4].copy_from_slice(&0xEF02_0000u32.to_le_bytes());
    let mut machine = Machine::from_rom(rom);
    machine.bus.irq.write16(0, 1 << 3);
    machine.bus.irq.write16(8, 1);

    machine.run_cycles(1);
    assert!(machine.bus.halted, "SWI 2 should halt");

    machine.bus.timers.write16(0, 0xFFFF);
    machine.bus.timers.write16(2, 0x80 | (1 << 6));
    machine.run_cycles(32);
    assert!(!machine.bus.halted);
    assert_eq!(machine.cpu.cpsr() & 0x1F, 0x12);
}

#[test]
fn halt_forever_warns_once() {
    let mut machine = Machine::from_rom(vec![0; 0xC0]);
    machine.bus.write8(0x0400_0301, 0);
    assert!(machine.bus.halted);
    assert!(!machine.bus.irq.can_wake());

    machine.run_cycles(50);
    let warns: Vec<_> = machine
        .bus
        .warn_lines
        .iter()
        .filter(|line| line.contains("halt forever"))
        .collect();
    assert_eq!(warns.len(), 1);
    assert_eq!(warns[0], "gba-debug: warn halt forever");
    assert!(machine.bus.halted);

    machine.run_cycles(50);
    let warns = machine
        .bus
        .warn_lines
        .iter()
        .filter(|line| line.contains("halt forever"))
        .count();
    assert_eq!(warns, 1);
}

#[test]
fn keypad_irq_sets_if_bit_12() {
    let mut machine = Machine::from_rom(vec![0; 0xC0]);
    machine.bus.keypad.write_cnt((1 << 14) | 1);
    machine.bus.keypad.set_pressed(1);
    assert!(machine.bus.keypad.irq_asserted());
    machine.run_cycles(1);
    assert_ne!(machine.bus.irq.iff() & 0x1000, 0);
}

#[test]
fn vblank_irq_rising_edge_sets_if_bit0() {
    let mut machine = Machine::from_rom(vec![0; 0xC0]);
    // DISPSTAT bit 3 = vblank IRQ enable (bits 0–2 stay read-only).
    machine.bus.write16(0x0400_0004, 1 << 3);
    assert_eq!(machine.bus.dispstat_written() & (1 << 3), 1 << 3);

    // Enter the first vblank line (line 160).
    machine.run_cycles(160 * 1232);
    assert!(machine.bus.vblank);
    assert_ne!(
        machine.bus.irq.iff() & 1,
        0,
        "IF bit 0 should rise on vblank entry"
    );

    // Acknowledge; further cycles inside vblank must not re-raise.
    machine.bus.irq.write16(2, 1);
    assert_eq!(machine.bus.irq.iff() & 1, 0);
    machine.run_cycles(64);
    assert!(machine.bus.vblank);
    assert_eq!(
        machine.bus.irq.iff() & 1,
        0,
        "vblank IF must stick clear until the next rising edge"
    );
}

#[test]
fn vcount_io_tracks_scanline_after_run() {
    let mut machine = Machine::from_rom(vec![0; 0xC0]);
    // 1232 cycles per line — same base as CYCLES_PER_LINE in hw/mod.rs.
    machine.run_cycles(159 * 1232);
    assert_eq!(machine.bus.read8(0x0400_0006), 159);
}

#[test]
fn halt_clears_with_i_set_without_taking_irq() {
    // MSR CPSR_c, #0x9F — System mode with I set.
    let mut rom = vec![0u8; 0xC0];
    rom[0..4].copy_from_slice(&0xE321_F09Fu32.to_le_bytes());
    let mut machine = Machine::from_rom(rom);
    machine.run_cycles(1);
    assert_ne!(machine.cpu.cpsr() & 0x80, 0);
    assert_eq!(machine.cpu.cpsr() & 0x1F, 0x1F);

    machine.bus.irq.write16(0, 1 << 3);
    machine.bus.irq.write16(8, 1);
    machine.bus.timers.write16(0, 0xFFFF);
    machine.bus.timers.write16(2, 0x80 | (1 << 6));
    machine.bus.write8(0x0400_0301, 0);
    assert!(machine.bus.halted);

    machine.run_cycles(32);
    assert!(!machine.bus.halted, "pending IRQ must leave halt");
    assert_eq!(machine.cpu.cpsr() & 0x1F, 0x1F, "mode stays System");
    assert_ne!(machine.cpu.fetch_pc, 0x18, "IRQ exception must not run");
    assert!(!machine.cpu.idle);
}
