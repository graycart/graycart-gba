//! Unit tests for CPU pipeline + exception entry stubs.

use super::*;
use crate::bus::{CpuMem, FlatRam};
use exception::{CPSR_F, CPSR_I, CPSR_MODE_MASK, CPSR_T};

#[test]
fn pipeline_r15_skew_arm_and_thumb() {
    let mut arm = Pipeline::new(IsaState::Arm, 0x0800_0008);
    assert_eq!(arm.r15_read(), 0x0800_0008);
    assert_eq!(arm.implied_exec_pc(), 0x0800_0000);
    assert_eq!(IsaState::Arm.pc_read_skew(), 8);

    let thumb = Pipeline::new(IsaState::Thumb, 0x0800_0004);
    assert_eq!(thumb.r15_read(), 0x0800_0004);
    assert_eq!(thumb.implied_exec_pc(), 0x0800_0000);
    assert_eq!(IsaState::Thumb.pc_read_skew(), 4);

    arm.redirect(0x0800_0002, IsaState::Arm);
    assert_eq!(arm.fetch_pc, 0x0800_0000, "ARM PC write clears [1:0]");
    assert!(arm.is_flushed());
}

#[test]
fn pipeline_refill_fetches_two_arm_words() {
    let mut mem = FlatRam::new(64);
    // opcodes at 0, 4, 8, …
    mem.write32(0, 0xE1A0_0000); // NOP-ish MOV R0,R0 encoding placeholder
    mem.write32(4, 0xE1A0_1001);
    mem.write32(8, 0xE1A0_2002);

    let mut pipe = Pipeline::new(IsaState::Arm, 0);
    pipe.refill(&mut mem);

    let decode = pipe.decode.expect("decode filled");
    let fetch = pipe.fetch.expect("fetch filled");
    assert_eq!(decode.addr, 0);
    assert_eq!(decode.raw, 0xE1A0_0000);
    assert_eq!(fetch.addr, 4);
    assert_eq!(fetch.raw, 0xE1A0_1001);
    assert_eq!(pipe.fetch_pc, 8);
    assert_eq!(pipe.r15_read(), 8);
}

#[test]
fn pipeline_refill_thumb_halfwords() {
    let mut mem = FlatRam::new(32);
    mem.write16(0, 0x0000);
    mem.write16(2, 0x0001);
    mem.write16(4, 0x0002);

    let mut pipe = Pipeline::new(IsaState::Thumb, 0);
    pipe.refill(&mut mem);
    assert_eq!(pipe.decode.unwrap().raw, 0x0000);
    assert_eq!(pipe.fetch.unwrap().raw, 0x0001);
    assert_eq!(pipe.fetch_pc, 4);
}

#[test]
fn exception_vectors_and_modes() {
    assert_eq!(ExceptionKind::Reset.vector(), 0x00);
    assert_eq!(ExceptionKind::Undefined.vector(), 0x04);
    assert_eq!(ExceptionKind::Swi.vector(), 0x08);
    assert_eq!(ExceptionKind::PrefetchAbort.vector(), 0x0C);
    assert_eq!(ExceptionKind::DataAbort.vector(), 0x10);
    assert_eq!(ExceptionKind::Irq.vector(), 0x18);
    assert_eq!(ExceptionKind::Fiq.vector(), 0x1C);

    assert_eq!(ExceptionKind::Irq.mode_bits(), ExceptionModeBits::Irq);
    assert_eq!(
        ExceptionKind::Swi.mode_bits(),
        ExceptionModeBits::Supervisor
    );
    assert_eq!(GBA_IRQ_HANDLER_PTR, 0x0300_7FFC);
    assert_eq!(EXCEPTION_ENTRY_BUS_HINT, "2S+1N");
}

#[test]
fn irq_entry_from_arm_forces_arm_disables_i_keeps_f() {
    // User mode, IRQs enabled, FIQ enabled, ARM.
    let cpsr = u32::from(ExceptionModeBits::User as u8);
    let resume = 0x0800_0100;
    let plan = plan_entry(ExceptionKind::Irq, cpsr, resume);

    assert_eq!(plan.lr, resume.wrapping_add(4));
    assert_eq!(plan.spsr, cpsr);
    assert_eq!(plan.pc, 0x18);
    assert!(!plan.thumb_state());
    assert!(plan.irq_disabled());
    assert!(!plan.fiq_disabled());
    assert_eq!(plan.mode_bits(), ExceptionModeBits::Irq as u8);
    assert_eq!(
        plan.new_cpsr & CPSR_MODE_MASK,
        ExceptionModeBits::Irq as u8 as u32
    );
    assert_eq!(plan.new_cpsr & CPSR_I, CPSR_I);
    assert_eq!(plan.new_cpsr & CPSR_F, 0);
    assert_eq!(ExceptionKind::Irq.return_op(), "SUBS PC, R14, #4");
}

#[test]
fn swi_entry_from_thumb_clears_t_and_sets_lr_next() {
    let cpsr = u32::from(ExceptionModeBits::System as u8) | CPSR_T;
    let swi_addr = 0x0800_0200;
    let plan = plan_entry(ExceptionKind::Swi, cpsr, swi_addr);

    assert_eq!(plan.lr, swi_addr + 2, "Thumb SWI resumes at next halfword");
    assert_eq!(plan.spsr & CPSR_T, CPSR_T, "SPSR preserves old Thumb bit");
    assert!(!plan.thumb_state());
    assert!(plan.irq_disabled());
    assert!(!plan.fiq_disabled());
    assert_eq!(plan.mode_bits(), ExceptionModeBits::Supervisor as u8);
    assert_eq!(plan.pc, 0x08);
}

#[test]
fn reset_and_fiq_set_f_bit() {
    let cpsr = u32::from(ExceptionModeBits::System as u8);
    let reset = plan_entry(ExceptionKind::Reset, cpsr, 0);
    assert!(reset.fiq_disabled());
    assert!(reset.irq_disabled());
    assert_eq!(reset.mode_bits(), ExceptionModeBits::Supervisor as u8);

    let fiq = plan_entry(ExceptionKind::Fiq, cpsr, 0x200);
    assert!(fiq.fiq_disabled());
    assert_eq!(fiq.lr, 0x204);
    assert_eq!(fiq.pc, 0x1C);
}

#[test]
fn data_abort_lr_is_plus_eight() {
    let cpsr = 0x1F;
    let plan = plan_entry(ExceptionKind::DataAbort, cpsr, 0x0800_00F0);
    assert_eq!(plan.lr, 0x0800_00F8);
    assert_eq!(ExceptionKind::DataAbort.return_op(), "SUBS PC, R14, #8");
}

#[test]
fn cpu_take_exception_applies_regs_and_flushes_pipe() {
    let mut cpu = Cpu::new();
    // System + Thumb, IRQs enabled.
    cpu.regs
        .set_cpsr(CPSR_T | u32::from(ExceptionModeBits::System as u8));
    cpu.pipeline = Pipeline::new(IsaState::Thumb, 0x0800_0104);
    cpu.pipeline.decode = Some(PipelineSlot {
        addr: 0x0800_0100,
        raw: 0xDF00,
    });

    let resume = 0x0800_0102;
    let plan = cpu.take_exception(ExceptionKind::Irq, resume);
    assert_eq!(plan.pc, 0x18);
    assert_eq!(cpu.pipeline.isa, IsaState::Arm);
    assert_eq!(cpu.pipeline.fetch_pc, 0x18);
    assert!(cpu.pipeline.is_flushed());

    assert_eq!(cpu.regs.mode(), Mode::Irq);
    assert!(!cpu.regs.thumb());
    assert!(cpu.regs.irq_disabled());
    assert_eq!(cpu.regs.get_r14_mode(Mode::Irq), resume.wrapping_add(4));
    assert_eq!(cpu.regs.spsr_of(Mode::Irq), Some(plan.spsr));
    assert_eq!(cpu.regs.pc(), 0x18);
}

#[test]
fn flat_ram_le_word_roundtrip() {
    let mut mem = FlatRam::new(16);
    mem.write32(4, 0xA1B2_C3D4);
    assert_eq!(mem.read8(4), 0xD4);
    assert_eq!(mem.read8(5), 0xC3);
    assert_eq!(mem.read16(4), 0xC3D4);
    assert_eq!(mem.read32(4), 0xA1B2_C3D4);
}
