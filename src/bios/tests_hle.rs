//! G7-hle unit tests.
//!
//! Cited: GBATEK — BIOS Div / Sqrt
//!   https://problemkaputt.de/gbatek.htm

use super::hle::{self, swi};
use crate::bus::{CpuMem, FlatRam};
use crate::cpu::{soft_boot, Cpu};

#[test]
fn soft_boot_latch_constant() {
    let mut cpu = Cpu::new();
    let latch = hle::soft_boot_cart(&mut cpu);
    assert_eq!(latch, super::LATCH_SOFT_RESET);
    assert_eq!(cpu.regs.pc(), soft_boot::CART_ENTRY);
}

#[test]
fn sqrt_swi() {
    let mut cpu = Cpu::new();
    let mut mem = FlatRam::new(16);
    cpu.regs.set(0, 144);
    assert!(matches!(
        hle::try_swi(&mut cpu, &mut mem, swi::SQRT),
        hle::SwiHleResult::Done
    ));
    assert_eq!(cpu.regs.get(0), 12);
    assert_eq!(
        hle::latch_after_handled_swi(swi::SQRT),
        super::LATCH_AFTER_SWI
    );
}

#[test]
fn div_swi() {
    let mut cpu = Cpu::new();
    let mut mem = FlatRam::new(16);
    cpu.regs.set(0, 100);
    cpu.regs.set(1, 7);
    assert!(matches!(
        hle::try_swi(&mut cpu, &mut mem, swi::DIV),
        hle::SwiHleResult::Done
    ));
    assert_eq!(cpu.regs.get(0), 14);
    assert_eq!(cpu.regs.get(1), 2);
}

#[test]
fn cpuset_rejects_bios_source() {
    let mut cpu = Cpu::new();
    let mut mem = FlatRam::with_base(0, 0x100);
    cpu.regs.set(0, 0x100); // BIOS-range source
    cpu.regs.set(1, 0x80);
    cpu.regs.set(2, 4);
    mem.write32(0x80, 0xDEAD_BEEF);
    assert!(matches!(
        hle::try_swi(&mut cpu, &mut mem, swi::CPU_SET),
        hle::SwiHleResult::Done
    ));
    // Destination unchanged because src < 0x4000.
    assert_eq!(mem.read32(0x80), 0xDEAD_BEEF);
}
