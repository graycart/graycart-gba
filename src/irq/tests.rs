//! Unit tests for IE / IF (W1C) / IME and CPU IRQ line (G3-irq-line).
//!
//! Cited: GBATEK — Interrupt Control
//!   https://problemkaputt.de/gbatek.htm#gbainterruptcontrol
//! Cited: GBATEK — ARM CPU Exceptions (vector `0x18`)
//!   https://problemkaputt.de/gbatek.htm#armcpuexceptions

use super::*;
use crate::cpu::{cpsr, Cpu, ExceptionKind, ExceptionModeBits, IsaState, Mode, Pipeline};

#[test]
fn ie_masks_unused_high_bits() {
    let mut irq = Irq::new();
    irq.write_ie(0xFFFF);
    assert_eq!(irq.read_ie(), IRQ_SOURCE_MASK);
    assert_eq!(IE_ADDR, 0x0400_0200);
}

#[test]
fn if_write_one_to_clear() {
    let mut irq = Irq::new();
    irq.raise(IRQ_TIMER0 | IRQ_TIMER1 | IRQ_VBLANK);
    assert_eq!(irq.read_if(), IRQ_TIMER0 | IRQ_TIMER1 | IRQ_VBLANK);

    // Writing 0 must not clear anything.
    irq.write_if_ack(0);
    assert_eq!(irq.read_if(), IRQ_TIMER0 | IRQ_TIMER1 | IRQ_VBLANK);

    // Ack only timer0.
    irq.write_if_ack(IRQ_TIMER0);
    assert_eq!(irq.read_if(), IRQ_TIMER1 | IRQ_VBLANK);
    assert_eq!(IF_ADDR, 0x0400_0202);
}

#[test]
fn ime_bit0_only() {
    let mut irq = Irq::new();
    assert!(!irq.ime());
    irq.write_ime(0xFFFF_FFFE);
    assert!(!irq.ime());
    assert_eq!(irq.read_ime(), 0);

    irq.write_ime(1);
    assert!(irq.ime());
    assert_eq!(irq.read_ime(), 1);
    assert_eq!(IME_ADDR, 0x0400_0208);
}

#[test]
fn raise_sets_if_without_ime() {
    let mut irq = Irq::new();
    assert!(!irq.ime());
    irq.raise(IRQ_KEYPAD);
    assert_eq!(irq.read_if(), IRQ_KEYPAD);
}

#[test]
fn ie_and_if_for_halt_wake_ignores_ime() {
    let mut irq = Irq::new();
    irq.write_ie(IRQ_TIMER0);
    irq.raise(IRQ_TIMER0 | IRQ_VBLANK);
    assert!(!irq.ime());
    assert_eq!(irq.ie_and_if(), IRQ_TIMER0);
    assert!(irq.halt_wake_pending());
}

#[test]
fn cpu_irq_requires_ime_ie_if_and_cpsr_i_clear() {
    let mut irq = Irq::new();
    irq.write_ie(IRQ_VBLANK);
    irq.raise(IRQ_VBLANK);

    // IME=0 → no CPU IRQ even with IE∧IF and I=0.
    assert!(!irq.cpu_irq_asserted(false));

    irq.set_ime(true);
    assert!(irq.cpu_irq_asserted(false));
    // CPSR.I set → masked.
    assert!(!irq.cpu_irq_asserted(true));

    irq.write_ie(0);
    assert!(!irq.cpu_irq_asserted(false));
}

#[test]
fn try_take_cpu_irq_vectors_to_0x18() {
    let mut irq = Irq::new();
    irq.write_ie(IRQ_TIMER0);
    irq.raise(IRQ_TIMER0);
    irq.set_ime(true);

    let mut cpu = Cpu::new();
    // System mode, IRQs enabled (I=0), ARM.
    cpu.regs
        .set_cpsr(u32::from(ExceptionModeBits::System as u8));
    let resume = 0x0800_1000;
    let plan = irq
        .try_take_cpu_irq(&mut cpu, resume)
        .expect("CPU IRQ should fire");

    assert_eq!(plan.kind, ExceptionKind::Irq);
    assert_eq!(plan.pc, 0x18);
    assert_eq!(cpu.regs.pc(), 0x18);
    assert_eq!(cpu.regs.mode(), Mode::Irq);
    assert!(cpu.regs.irq_disabled());
    assert_eq!(cpu.regs.get_r14_mode(Mode::Irq), resume.wrapping_add(4));
}

#[test]
fn try_take_cpu_irq_noop_when_masked() {
    let mut irq = Irq::new();
    irq.write_ie(IRQ_DMA0);
    irq.raise(IRQ_DMA0);
    // IME still false.
    let mut cpu = Cpu::new();
    cpu.regs
        .set_cpsr(u32::from(ExceptionModeBits::System as u8));
    assert!(irq.try_take_cpu_irq(&mut cpu, 0x0800_0000).is_none());
    assert_eq!(cpu.regs.mode(), Mode::System);
}

#[test]
fn try_take_cpu_irq_noop_when_cpsr_i_set() {
    let mut irq = Irq::new();
    irq.write_ie(IRQ_SERIAL);
    irq.raise(IRQ_SERIAL);
    irq.set_ime(true);

    let mut cpu = Cpu::new();
    cpu.regs
        .set_cpsr(u32::from(ExceptionModeBits::System as u8) | cpsr::I);
    assert!(cpu.regs.irq_disabled());
    assert!(irq.try_take_cpu_irq(&mut cpu, 0x0800_0000).is_none());
}

#[test]
fn try_service_cpu_uses_decode_pc() {
    let mut irq = Irq::new();
    irq.write_ie(IRQ_VCOUNT);
    irq.raise(IRQ_VCOUNT);
    irq.set_ime(true);

    let mut cpu = Cpu::new();
    cpu.regs.set_cpsr(u32::from(ExceptionModeBits::User as u8));
    cpu.pipeline = Pipeline::new(IsaState::Arm, 0x0800_0108);
    cpu.pipeline.decode = Some(crate::cpu::PipelineSlot {
        addr: 0x0800_0100,
        raw: 0xE1A0_0000,
    });

    let plan = irq.try_service_cpu(&mut cpu).expect("IRQ");
    assert_eq!(plan.lr, 0x0800_0100u32.wrapping_add(4));
    assert_eq!(plan.pc, 0x18);
}

#[test]
fn intr_wait_flags_helpers_roundtrip() {
    let mut iwram = vec![0u8; 0x8000];
    assert_eq!(read_intr_wait_flags(&iwram), 0);
    assert_eq!(INTR_WAIT_FLAGS_ADDR, 0x0300_7FF8);
    assert_eq!(IRQ_HANDLER_PTR_ADDR, 0x0300_7FFC);
    assert_eq!(intr_wait_flags_iwram_offset(), 0x7FF8);

    or_intr_wait_flags(&mut iwram, IRQ_VBLANK | IRQ_TIMER0);
    assert_eq!(read_intr_wait_flags(&iwram), IRQ_VBLANK | IRQ_TIMER0);

    clear_intr_wait_flags(&mut iwram, IRQ_VBLANK);
    assert_eq!(read_intr_wait_flags(&iwram), IRQ_TIMER0);
}

#[test]
fn raise_if_trait_bridges_to_raise() {
    use crate::input::RaiseIf;
    let mut irq = Irq::new();
    irq.raise_if(IRQ_KEYPAD);
    assert_eq!(irq.read_if(), IRQ_KEYPAD);
}

#[test]
fn irq_delay_constant_documented_but_unused() {
    // Guard against silent deletion of the TBD marker.
    assert_eq!(IRQ_DELAY_CYCLES_TBD, 7);
}
