//! Unit tests for ARM decode / ALU / a few execute paths.
//!
//! Cited: GBATEK -- ARM CPU Opcode Summary (fixture expectations)
//!   https://problemkaputt.de/gbatek.htm

use super::*;
use crate::bus::{CpuMem, FlatRam};
use crate::cpu::{cpsr, Cpu, ExceptionKind, Mode};

fn cpu_at(pc: u32) -> Cpu {
    let mut cpu = Cpu::new();
    cpu.regs.set_mode(Mode::System);
    cpu.regs.set_pc(pc);
    cpu.pipeline.redirect(pc, crate::cpu::IsaState::Arm);
    cpu
}

#[test]
fn mov_imm_writes_r0() {
    let mut cpu = cpu_at(0x0800_0000);
    let mut mem = FlatRam::new(4);
    let r = step(&mut cpu, &mut mem, 0xE3A0_00A5);
    assert_eq!(r, ExecResult::Ok);
    assert_eq!(cpu.regs.get(0), 0xA5);
}

#[test]
fn adds_sets_flags() {
    let mut cpu = cpu_at(0x0800_0000);
    let mut mem = FlatRam::new(4);
    cpu.regs.set(0, 0xFFFF_FFFF);
    let r = step(&mut cpu, &mut mem, 0xE290_0001);
    assert_eq!(r, ExecResult::Ok);
    assert_eq!(cpu.regs.get(0), 0);
    assert!(cpu.regs.z());
    assert!(cpu.regs.c());
    assert!(!cpu.regs.n());
}

#[test]
fn cond_skip_leaves_regs() {
    let mut cpu = cpu_at(0x0800_0000);
    let mut mem = FlatRam::new(4);
    cpu.regs.set_nzcv(false, false, false, false);
    // MOVEQ r0, #1
    let r = step(&mut cpu, &mut mem, 0x03A0_0001);
    assert_eq!(r, ExecResult::CondFailed);
    assert_eq!(cpu.regs.get(0), 0);
}

#[test]
fn branch_links_and_targets() {
    let mut cpu = cpu_at(0x0800_0000);
    let mut mem = FlatRam::new(4);
    let r = step(&mut cpu, &mut mem, 0xEB00_0000);
    assert_eq!(r, ExecResult::Branched);
    assert_eq!(cpu.regs.pc(), 0x0800_0008);
    assert_eq!(cpu.regs.get(14), 0x0800_0004);
    assert!(cpu.pipeline.is_flushed());
}

#[test]
fn bx_to_thumb() {
    let mut cpu = cpu_at(0x0800_0000);
    let mut mem = FlatRam::new(4);
    cpu.regs.set(0, 0x0800_0101);
    let r = step(&mut cpu, &mut mem, 0xE12F_FF10);
    assert_eq!(r, ExecResult::Branched);
    assert!(cpu.regs.thumb());
    assert_eq!(cpu.regs.pc(), 0x0800_0100);
}

#[test]
fn ldr_word_and_writeback() {
    let mut cpu = cpu_at(0x0800_0000);
    let mut mem = FlatRam::new(0x100);
    mem.write32(0x40, 0x1122_3344);
    cpu.regs.set(1, 0x40);
    let r = step(&mut cpu, &mut mem, 0xE491_0004);
    assert_eq!(r, ExecResult::Ok);
    assert_eq!(cpu.regs.get(0), 0x1122_3344);
    assert_eq!(cpu.regs.get(1), 0x44);
}

#[test]
fn misaligned_ldr_rors() {
    let mut cpu = cpu_at(0x0800_0000);
    let mut mem = FlatRam::new(0x100);
    mem.write32(0x40, 0x1122_3344);
    cpu.regs.set(1, 0x41);
    let r = step(&mut cpu, &mut mem, 0xE591_0000);
    assert_eq!(r, ExecResult::Ok);
    assert_eq!(cpu.regs.get(0), 0x4411_2233);
}

#[test]
fn mul_accumulate() {
    let mut cpu = cpu_at(0x0800_0000);
    let mut mem = FlatRam::new(4);
    cpu.regs.set(1, 3);
    cpu.regs.set(2, 5);
    cpu.regs.set(3, 7);
    let r = step(&mut cpu, &mut mem, 0xE020_3291);
    assert_eq!(r, ExecResult::Ok);
    assert_eq!(cpu.regs.get(0), 22);
}

#[test]
fn swi_signals_exception() {
    let mut cpu = cpu_at(0x0800_0000);
    let mut mem = FlatRam::new(4);
    let r = step(&mut cpu, &mut mem, 0xEF00_0006);
    assert_eq!(r, ExecResult::Exception(ExceptionKind::Swi));
}

#[test]
fn mrs_cpsr() {
    let mut cpu = cpu_at(0x0800_0000);
    let mut mem = FlatRam::new(4);
    cpu.regs.set_nzcv(true, false, true, false);
    let r = step(&mut cpu, &mut mem, 0xE10F_0000);
    assert_eq!(r, ExecResult::Ok);
    assert_eq!(cpu.regs.get(0) & cpsr::N, cpsr::N);
    assert_eq!(cpu.regs.get(0) & cpsr::C, cpsr::C);
}

#[test]
fn pc_relative_add_uses_plus_8() {
    let mut cpu = cpu_at(0x0800_0000);
    let mut mem = FlatRam::new(4);
    let r = step(&mut cpu, &mut mem, 0xE28F_0000);
    assert_eq!(r, ExecResult::Ok);
    assert_eq!(cpu.regs.get(0), 0x0800_0008);
}
