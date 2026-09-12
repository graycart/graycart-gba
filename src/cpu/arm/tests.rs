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

/// jsmolka arm #224: `mov r0, pc, lsl r0` with Rs-shift samples PC+12.
#[test]
fn mov_pc_lsl_rs_uses_plus_12() {
    let mut cpu = cpu_at(0x0800_0000);
    let mut mem = FlatRam::new(4);
    cpu.regs.set(0, 0);
    // mov r0, pc, lsl r0
    let r = step(&mut cpu, &mut mem, 0xE1A0_001F);
    assert_eq!(r, ExecResult::Ok);
    assert_eq!(cpu.regs.get(0), 0x0800_000C);
}

/// jsmolka arm #225: `add r0, pc, r0, lsl r0` — Rn=PC also PC+12 under Rs-shift.
#[test]
fn add_pc_rm_lsl_rs_uses_plus_12() {
    let mut cpu = cpu_at(0x0800_0000);
    let mut mem = FlatRam::new(4);
    cpu.regs.set(0, 0);
    // add r0, pc, r0, lsl r0
    let r = step(&mut cpu, &mut mem, 0xE08F_0010);
    assert_eq!(r, ExecResult::Ok);
    assert_eq!(cpu.regs.get(0), 0x0800_000C);
}

/// jsmolka arm #234: CMP with Rd=15 restores SPSR→CPSR (no Rd write).
#[test]
fn cmp_rd15_restores_spsr_without_branch() {
    let mut cpu = cpu_at(0x0800_0000);
    let mut mem = FlatRam::new(4);
    cpu.regs.set(8, 32); // User/System R8
    cpu.regs.set_mode(Mode::Fiq);
    cpu.regs.set(8, 64); // FIQ R8
    cpu.regs.set_spsr(Mode::System.bits());
    // cmp pc, r0 with Rd=15 encoding (0xE15FF000)
    let r = step(&mut cpu, &mut mem, 0xE15F_F000);
    assert_eq!(r, ExecResult::Ok);
    assert_eq!(cpu.regs.mode(), Mode::System);
    assert_eq!(cpu.regs.get(8), 32);
}

/// jsmolka arm #511: STM^ stores User-bank R8 while in FIQ.
#[test]
fn stm_user_bank_from_fiq() {
    let mut cpu = cpu_at(0x0800_0000);
    let mut mem = FlatRam::new(0x100);
    let base = 0x40u32;
    cpu.regs.set(0, base);
    cpu.regs.set(8, 32); // User R8
    cpu.regs.set_mode(Mode::Fiq);
    cpu.regs.set(8, 64); // FIQ R8
                         // stmfd r0, {r8, r9}^  (S=1, no writeback)
    let r = step(&mut cpu, &mut mem, 0xE940_0300);
    assert_eq!(r, ExecResult::Ok);
    // Fully descending, 2 regs: stores at base-8, base-4
    assert_eq!(mem.read32(base - 8), 32);
}
