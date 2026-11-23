use super::{Cpu, StepError};
use crate::bus::Bus;

#[test]
fn unimplemented_names_the_opcode() {
    let err = StepError::Unimplemented {
        pc: 0x0800_0000,
        mnemonic: "arm 0xE7F000F0".into(),
    };
    assert_eq!(
        err.to_string(),
        "unimplemented pc=0x08000000 mnemonic=arm 0xE7F000F0"
    );
}

#[test]
fn reset_is_system_mode_at_rom() {
    let cpu = Cpu::new();
    assert_eq!(cpu.cpsr(), 0x1F);
    assert_eq!(cpu.exec_pc, 0x0800_0000);
    assert_eq!(cpu.reg(13), 0x0300_7F00);
    assert!(!cpu.idle);
}

#[test]
fn irq_system_mode_roundtrip_keeps_irq_stack() {
    let mut bus = Bus::new(vec![0; 0x200]);
    bus.write32(0x0300_7FFC, 0x0800_0200);
    let mut cpu = Cpu::new();
    // Pretend game state in System Thumb.
    cpu.write_cpsr_for_test(0x6000_003F);
    cpu.set_reg_for_test(13, 0x0300_7E50);
    cpu.set_reg_for_test(14, 0x0800_1001);
    cpu.fetch_pc = 0x0800_075A;

    cpu.raise_irq_from(&mut bus, true);
    assert_eq!(cpu.cpsr() & 0x1F, 0x12);
    let irq_sp_after_push = cpu.reg(13);
    assert_eq!(cpu.reg(14), 0x138);

    // Metroid: stmfd {r0-r3,lr} (5 words)
    let sp = cpu.reg(13).wrapping_sub(20);
    cpu.set_reg_for_test(13, sp);
    bus.write32(sp, 0x6000_003F); // spsr mirror in r0
    bus.write32(sp + 4, 0);
    bus.write32(sp + 8, 0);
    bus.write32(sp + 12, 0x0400_0200);
    bus.write32(sp + 16, 0x138);

    // Switch to System like Metroid (cpsr_fc).
    let mut r3 = cpu.cpsr();
    r3 &= !0xDF;
    r3 |= 0x1F;
    cpu.write_cpsr_for_test_masked(r3, 0xFF00_00FF);
    assert_eq!(cpu.cpsr() & 0x1F, 0x1F);
    assert_eq!(cpu.reg(13), 0x0300_7E50, "System SP restored");

    // Switch back to IRQ.
    let mut r3 = cpu.cpsr();
    r3 &= !0xDF;
    r3 |= 0x92;
    cpu.write_cpsr_for_test_masked(r3, 0xFF00_00FF);
    assert_eq!(cpu.cpsr() & 0x1F, 0x12);
    assert_eq!(cpu.reg(13), sp, "IRQ SP restored after System");
    assert_eq!(cpu.reg(14), 0x138);

    // Pop Metroid frame
    cpu.set_reg_for_test(13, sp + 20);
    assert_eq!(cpu.reg(13), irq_sp_after_push);

    // HLE return
    cpu.fetch_pc = 0x138;
    cpu.step(&mut bus).unwrap();
    assert_eq!(cpu.fetch_pc & !1, 0x0800_075A, "resume Halt PC");
    assert_eq!(cpu.cpsr() & 0x1F, 0x1F);
}

#[test]
fn halt_entry_early_irq_stacks_bios_strb_link() {
    use super::halt::{WRITE_BOUNDARY, halt_entry};
    // Timer overflow inside the SWI body: stacked LR 0xB8, entered at the boundary.
    let (lr, owed) = halt_entry(WRITE_BOUNDARY, true, false);
    assert_eq!((lr, owed), (0xB8, 41));
    let (lr, owed) = halt_entry(WRITE_BOUNDARY - 1, true, false);
    assert_eq!((lr, owed), (0xB8, 42));
}

#[test]
fn halt_entry_settled_irq_stacks_halt_link() {
    use super::halt::{SETTLE, halt_entry};
    // First halted cycle cannot wake early: wait until SETTLE.
    let (lr, owed) = halt_entry(SETTLE - 1, true, false);
    assert_eq!((lr, owed), (0x74, 43));
    let (lr, owed) = halt_entry(94, true, false);
    assert_eq!((lr, owed), (0x74, 42));
}

#[test]
fn halt_entry_raw_haltcnt_and_video_wake() {
    use super::halt::halt_entry;
    // No SWI body behind a raw HALTCNT write.
    assert_eq!(halt_entry(1, false, false), (0x74, 42));
    // Video-only wakes reach the handler sooner than timer / serial wakes.
    let (lr, owed) = halt_entry(2_000, true, true);
    assert_eq!(lr, 0x74);
    assert!(owed < 42);
}

#[test]
fn swi_2_enters_halt_via_swi() {
    let mut bus = Bus::new(vec![0; 0x200]);
    let mut cpu = Cpu::new();
    cpu.swi_number_for_test(&mut bus, 2);
    assert!(bus.halted);
    assert!(bus.halt_via_swi);
    bus.write8(0x0400_0301, 0);
    assert!(!bus.halt_via_swi, "raw HALTCNT has no BIOS body");
}
