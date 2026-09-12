//! ARM instruction execute dispatch.
//!
//! Cited: GBATEK -- ARM CPU Instruction Set / memory alignments
//!   https://problemkaputt.de/gbatek.htm
//! Cited: ARM DDI0210C -- ARM instruction execution / shifted operands / LDM·STM
//! Note: uses `Cpu.regs` only (no direct `mode`/`regs` module edits). Crude timing.
//! Note: R15 as Rn/Rm with register-specified shift reads as PC+12 (ARM7TDMI).
//! Note: LDM/STM S-bit force-user bank + empty Rlist ARMv4 quirks (GBATEK).

use crate::cpu::{cpsr, Cpu, ExceptionKind, IsaState, Mode};

use super::alu::{apply_flags, data_process};
use super::bus::ArmBus;
use super::core::{read_reg, read_reg_skew, write_reg, PC_SKEW_ARM_REG_SHIFT};
use super::decode::{decode, Decoded, MsrSrc, Op, Op2, SingleOffset};
use super::shifter::{shift_imm_ror, shift_reg_imm, shift_reg_reg, ShiftOut};

/// Result of attempting to execute one ARM instruction word.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecResult {
    /// Condition failed; still costs 1S fetch advance (pipeline owner).
    CondFailed,
    /// Executed; sequential PC advance (+4) expected.
    Ok,
    /// PC / state changed — pipeline must refill.
    Branched,
    /// SWI / Undefined — caller should `Cpu::take_exception`.
    Exception(ExceptionKind),
}

/// Decode + execute one ARM opcode from `raw`.
pub fn step(cpu: &mut Cpu, bus: &mut impl ArmBus, raw: u32) -> ExecResult {
    execute(cpu, bus, decode(raw))
}

/// Execute an already-decoded ARM instruction.
pub fn execute(cpu: &mut Cpu, bus: &mut impl ArmBus, decoded: Decoded) -> ExecResult {
    if !cpu.regs.cond_passed(decoded.cond) {
        return ExecResult::CondFailed;
    }
    match decoded.op {
        Op::DataProcessing {
            opcode,
            s,
            rn,
            rd,
            op2,
        } => exec_data_processing(cpu, opcode, s, rn, rd, op2),
        Op::Multiply {
            a,
            s,
            rd,
            rn,
            rs,
            rm,
        } => {
            exec_multiply(cpu, a, s, rd, rn, rs, rm);
            ExecResult::Ok
        }
        Op::MultiplyLong {
            signed,
            a,
            s,
            rd_hi,
            rd_lo,
            rs,
            rm,
        } => {
            exec_multiply_long(cpu, signed, a, s, rd_hi, rd_lo, rs, rm);
            ExecResult::Ok
        }
        Op::Swap { byte, rn, rd, rm } => {
            exec_swap(cpu, bus, byte, rn, rd, rm);
            ExecResult::Ok
        }
        Op::Bx { rm } => {
            let target = read_reg(cpu, rm);
            cpu.regs.branch_exchange(target);
            let isa = IsaState::from_cpsr_t(cpu.regs.thumb());
            cpu.pipeline.redirect(cpu.regs.pc(), isa);
            ExecResult::Branched
        }
        Op::HalfwordTransfer {
            load,
            writeback,
            imm,
            up,
            pre,
            s_bit,
            h_bit,
            rn,
            rd,
            offset,
        } => exec_halfword(
            cpu, bus, load, writeback, imm, up, pre, s_bit, h_bit, rn, rd, offset,
        ),
        Op::SingleTransfer {
            load,
            writeback,
            byte,
            up,
            pre,
            rn,
            rd,
            offset,
        } => exec_single(cpu, bus, load, writeback, byte, up, pre, rn, rd, offset),
        Op::BlockTransfer {
            load,
            writeback,
            s_bit,
            up,
            pre,
            rn,
            rlist,
        } => exec_block(cpu, bus, load, writeback, s_bit, up, pre, rn, rlist),
        Op::Branch { link, offset } => exec_branch(cpu, link, offset),
        Op::SoftwareInterrupt { .. } => ExecResult::Exception(ExceptionKind::Swi),
        Op::Mrs { spsr, rd } => {
            let val = if spsr {
                cpu.regs.spsr().unwrap_or(0)
            } else {
                cpu.regs.cpsr()
            };
            write_reg(cpu, rd, val);
            if rd == 15 {
                note_branch(cpu);
                ExecResult::Branched
            } else {
                ExecResult::Ok
            }
        }
        Op::Msr { spsr, fields, src } => {
            exec_msr(cpu, spsr, fields, src);
            ExecResult::Ok
        }
        Op::Undefined => ExecResult::Exception(ExceptionKind::Undefined),
    }
}

/// Keep `pipeline.fetch_pc` aligned with architectural PC after a branch/PC write.
fn note_branch(cpu: &mut Cpu) {
    let isa = IsaState::from_cpsr_t(cpu.regs.thumb());
    cpu.pipeline.redirect(cpu.regs.pc(), isa);
}

fn resolve_op2(cpu: &Cpu, op2: Op2) -> ShiftOut {
    let carry_in = cpu.regs.c();
    match op2 {
        Op2::Imm { imm8, rot } => shift_imm_ror(imm8, rot, carry_in),
        Op2::RegImmShift { rm, ty, imm } => shift_reg_imm(read_reg(cpu, rm), ty, imm, carry_in),
        // Rm=R15 with SHIFT(Rs) samples PC+12 (ARM7TDMI pipeline + I-cycle).
        Op2::RegRegShift { rm, ty, rs } => shift_reg_reg(
            read_reg_skew(cpu, rm, PC_SKEW_ARM_REG_SHIFT),
            ty,
            (read_reg(cpu, rs) & 0xFF) as u8,
            carry_in,
        ),
    }
}

fn exec_data_processing(
    cpu: &mut Cpu,
    opcode: u8,
    s: bool,
    rn: u8,
    rd: u8,
    op2: Op2,
) -> ExecResult {
    let shift = resolve_op2(cpu, op2);
    // Rn=R15 with register-specified shift also reads as PC+12 (jsmolka #225).
    let rn_val = match op2 {
        Op2::RegRegShift { .. } => read_reg_skew(cpu, rn, PC_SKEW_ARM_REG_SHIFT),
        _ => read_reg(cpu, rn),
    };
    let alu = data_process(opcode, rn_val, shift.value, shift.carry, cpu.regs.c());

    // Rd=R15 + S=1 → copy SPSR→CPSR (exception-return / ARM7 "bad" test-op form).
    // TST/TEQ/CMP/CMN still restore when Rd encodes 15, but do **not** write R15 or
    // flush the pipeline (jsmolka arm #234/#235). Cited: ARM DDI0210C / GBATEK.
    if s && rd == 15 {
        cpu.regs.restore_cpsr_from_spsr();
        if alu.write_rd {
            write_reg(cpu, 15, alu.result);
            note_branch(cpu);
            return ExecResult::Branched;
        }
        return ExecResult::Ok;
    }

    if s {
        let new_cpsr = apply_flags(cpu.regs.cpsr(), &alu);
        cpu.regs.set_cpsr(new_cpsr);
    }

    if alu.write_rd {
        write_reg(cpu, rd, alu.result);
        if rd == 15 {
            note_branch(cpu);
            return ExecResult::Branched;
        }
    }
    ExecResult::Ok
}

fn exec_multiply(cpu: &mut Cpu, a: bool, s: bool, rd: u8, rn: u8, rs: u8, rm: u8) {
    let mut result = read_reg(cpu, rm).wrapping_mul(read_reg(cpu, rs));
    if a {
        result = result.wrapping_add(read_reg(cpu, rn));
    }
    if rd != 15 {
        cpu.regs.set(rd, result);
    }
    if s {
        cpu.regs.set_nzcv(
            result & (1 << 31) != 0,
            result == 0,
            cpu.regs.c(),
            cpu.regs.v(),
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn exec_multiply_long(
    cpu: &mut Cpu,
    signed: bool,
    a: bool,
    s: bool,
    rd_hi: u8,
    rd_lo: u8,
    rs: u8,
    rm: u8,
) {
    let a_rm = read_reg(cpu, rm);
    let a_rs = read_reg(cpu, rs);
    let mut product = if signed {
        (i64::from(a_rm as i32) * i64::from(a_rs as i32)) as u64
    } else {
        u64::from(a_rm) * u64::from(a_rs)
    };
    if a {
        let acc = (u64::from(read_reg(cpu, rd_hi)) << 32) | u64::from(read_reg(cpu, rd_lo));
        product = product.wrapping_add(acc);
    }
    let lo = product as u32;
    let hi = (product >> 32) as u32;
    if rd_lo != 15 {
        cpu.regs.set(rd_lo, lo);
    }
    if rd_hi != 15 {
        cpu.regs.set(rd_hi, hi);
    }
    if s {
        cpu.regs.set_nzcv(
            hi & (1 << 31) != 0,
            hi == 0 && lo == 0,
            cpu.regs.c(),
            cpu.regs.v(),
        );
    }
}

fn exec_branch(cpu: &mut Cpu, link: bool, offset: i32) -> ExecResult {
    // read_reg(15) == architectural PC + 8 (branch base).
    let pc_skewed = read_reg(cpu, 15);
    if link {
        // LR = next instruction = (pc_skewed - 8) + 4 = pc_skewed - 4
        cpu.regs.set(14, pc_skewed.wrapping_sub(4));
    }
    let dest = (pc_skewed as i32).wrapping_add(offset) as u32;
    cpu.regs.set_pc(dest);
    note_branch(cpu);
    ExecResult::Branched
}

fn exec_msr(cpu: &mut Cpu, spsr: bool, fields: u8, src: MsrSrc) {
    let value = match src {
        MsrSrc::Imm { imm8, rot } => shift_imm_ror(imm8, rot, cpu.regs.c()).value,
        MsrSrc::Reg { rm } => read_reg(cpu, rm),
    };

    let mut mask = 0u32;
    if fields & 1 != 0 {
        mask |= cpsr::CONTROL_MASK;
    }
    if fields & 2 != 0 {
        mask |= cpsr::EXTENSION_MASK;
    }
    if fields & 4 != 0 {
        mask |= cpsr::STATUS_MASK;
    }
    if fields & 8 != 0 {
        mask |= cpsr::FLAGS_MASK;
    }

    // User may only write flag field on CPSR.
    if !spsr && cpu.regs.mode() == Mode::User {
        mask &= cpsr::FLAGS_MASK;
    }
    // Do not change T via MSR.
    if !spsr {
        mask &= !cpsr::T;
    }

    if spsr {
        if let Some(old) = cpu.regs.spsr() {
            cpu.regs.set_spsr((old & !mask) | (value & mask));
        }
    } else {
        cpu.regs.msr_cpsr(value, mask);
    }
}

fn apply_offset(base: u32, offset: u32, up: bool) -> u32 {
    if up {
        base.wrapping_add(offset)
    } else {
        base.wrapping_sub(offset)
    }
}

#[allow(clippy::too_many_arguments)]
fn exec_single(
    cpu: &mut Cpu,
    bus: &mut impl ArmBus,
    load: bool,
    writeback: bool,
    byte: bool,
    up: bool,
    pre: bool,
    rn: u8,
    rd: u8,
    offset: SingleOffset,
) -> ExecResult {
    let carry_in = cpu.regs.c();
    let off = match offset {
        SingleOffset::Imm(imm) => u32::from(imm),
        SingleOffset::Reg { rm, ty, imm } => {
            shift_reg_imm(read_reg(cpu, rm), ty, imm, carry_in).value
        }
    };
    let base = read_reg(cpu, rn);
    let addr = if pre {
        apply_offset(base, off, up)
    } else {
        base
    };

    let mut branched = false;
    if load {
        let value = if byte {
            u32::from(bus.read8(addr))
        } else {
            let aligned = addr & !3;
            let data = bus.read32(aligned);
            data.rotate_right((addr & 3) * 8)
        };
        write_reg(cpu, rd, value);
        if rd == 15 {
            branched = true;
        }
    } else {
        let value = read_reg(cpu, rd);
        // ARM7 stores PC+12 when Rd=R15; skewed read is already +8, so +4 more.
        let store_val = if rd == 15 {
            value.wrapping_add(4)
        } else {
            value
        };
        if byte {
            bus.write8(addr, store_val as u8);
        } else {
            // Keep low bits for Game Pak SRAM rotate-write; Bus aligns other regions.
            bus.write32(addr, store_val);
        }
    }

    if writeback || !pre {
        let wb = if pre {
            addr
        } else {
            apply_offset(base, off, up)
        };
        if !(rn == rd && load) && rn != 15 {
            cpu.regs.set(rn, wb);
        }
    }

    if branched {
        note_branch(cpu);
        ExecResult::Branched
    } else {
        ExecResult::Ok
    }
}

#[allow(clippy::too_many_arguments)]
fn exec_halfword(
    cpu: &mut Cpu,
    bus: &mut impl ArmBus,
    load: bool,
    writeback: bool,
    imm: bool,
    up: bool,
    pre: bool,
    s_bit: bool,
    h_bit: bool,
    rn: u8,
    rd: u8,
    offset: u16,
) -> ExecResult {
    let off = if imm {
        u32::from(offset)
    } else {
        read_reg(cpu, (offset & 0xF) as u8)
    };
    let base = read_reg(cpu, rn);
    let addr = if pre {
        apply_offset(base, off, up)
    } else {
        base
    };

    let mut branched = false;
    if load {
        let value = match (s_bit, h_bit) {
            (false, true) => {
                let half = bus.read16(addr & !1);
                let mut v = u32::from(half);
                if addr & 1 != 0 {
                    v = v.rotate_right(8);
                }
                v
            }
            (true, false) => bus.read8(addr) as i8 as i32 as u32,
            (true, true) => {
                if addr & 1 != 0 {
                    bus.read8(addr) as i8 as i32 as u32
                } else {
                    bus.read16(addr) as i16 as i32 as u32
                }
            }
            _ => 0,
        };
        write_reg(cpu, rd, value);
        if rd == 15 {
            branched = true;
        }
    } else if !s_bit && h_bit {
        let value = read_reg(cpu, rd);
        let store_val = if rd == 15 {
            value.wrapping_add(4)
        } else {
            value
        };
        bus.write16(addr, store_val as u16);
    }

    if writeback || !pre {
        let wb = if pre {
            addr
        } else {
            apply_offset(base, off, up)
        };
        if !(rn == rd && load) && rn != 15 {
            cpu.regs.set(rn, wb);
        }
    }

    if branched {
        note_branch(cpu);
        ExecResult::Branched
    } else {
        ExecResult::Ok
    }
}

fn exec_swap(cpu: &mut Cpu, bus: &mut impl ArmBus, byte: bool, rn: u8, rd: u8, rm: u8) {
    let addr = read_reg(cpu, rn);
    if byte {
        let old = bus.read8(addr);
        bus.write8(addr, read_reg(cpu, rm) as u8);
        if rd != 15 {
            cpu.regs.set(rd, u32::from(old));
        }
    } else {
        let aligned = addr & !3;
        let data = bus.read32(aligned);
        let old = data.rotate_right((addr & 3) * 8);
        bus.write32(aligned, read_reg(cpu, rm));
        if rd != 15 {
            cpu.regs.set(rd, old);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn exec_block(
    cpu: &mut Cpu,
    bus: &mut impl ArmBus,
    load: bool,
    writeback: bool,
    s_bit: bool,
    up: bool,
    pre: bool,
    rn: u8,
    rlist: u16,
) -> ExecResult {
    // Empty Rlist (ARMv4): transfer R15 only; writeback base ±0x40.
    // Cited: GBATEK — Block Data Transfer (strange empty Rlist effects).
    if rlist == 0 {
        let base = read_reg(cpu, rn);
        let (addr, wb_value) = if up {
            if pre {
                (base.wrapping_add(4), base.wrapping_add(0x40))
            } else {
                (base, base.wrapping_add(0x40))
            }
        } else if pre {
            let start = base.wrapping_sub(0x40);
            (start, start)
        } else {
            let start = base.wrapping_sub(0x40);
            (start.wrapping_add(4), start)
        };
        let mut branched = false;
        if load {
            let value = bus.read32(addr & !3);
            if s_bit {
                cpu.regs.restore_cpsr_from_spsr();
            }
            write_reg(cpu, 15, value);
            branched = true;
        } else {
            // PC+12 store (skewed read is +8).
            let value = read_reg(cpu, 15).wrapping_add(4);
            bus.write32(addr & !3, value);
        }
        if writeback && rn != 15 {
            cpu.regs.set(rn, wb_value);
        }
        if branched {
            note_branch(cpu);
            ExecResult::Branched
        } else {
            ExecResult::Ok
        }
    } else {
        exec_block_rlist(cpu, bus, load, writeback, s_bit, up, pre, rn, rlist)
    }
}

#[allow(clippy::too_many_arguments)]
fn exec_block_rlist(
    cpu: &mut Cpu,
    bus: &mut impl ArmBus,
    load: bool,
    writeback: bool,
    s_bit: bool,
    up: bool,
    pre: bool,
    rn: u8,
    rlist: u16,
) -> ExecResult {
    let mut count = 0u32;
    for i in 0..16 {
        if rlist & (1 << i) != 0 {
            count += 1;
        }
    }

    let base = read_reg(cpu, rn);
    let (mut addr, wb_value) = if up {
        if pre {
            (base.wrapping_add(4), base.wrapping_add(count * 4))
        } else {
            (base, base.wrapping_add(count * 4))
        }
    } else if pre {
        let start = base.wrapping_sub(count * 4);
        (start, start)
    } else {
        let start = base.wrapping_sub(count * 4);
        (start.wrapping_add(4), start)
    };

    // S-bit: LDM with R15 → SPSR→CPSR; otherwise force User-bank GPR transfer.
    // Cited: GBATEK — Block Data Transfer (PSR & force user bit).
    let user_bank = s_bit && !(load && (rlist & (1 << 15)) != 0);
    let rn_in_list = rlist & (1 << rn) != 0;
    // ARMv4 STM: if Rn is in Rlist, store OLD base when Rn is the lowest set bit,
    // otherwise store the writeback (NEW) base. Cited: GBATEK empty/Rb-in-rlist notes.
    let rn_is_lowest_in_list = rn_in_list && (rlist & ((1u16 << rn) - 1)) == 0;

    let mut branched = false;
    for i in 0..16u8 {
        if rlist & (1 << i) == 0 {
            continue;
        }
        if load {
            let value = bus.read32(addr & !3);
            if i == 15 {
                if s_bit {
                    cpu.regs.restore_cpsr_from_spsr();
                }
                write_reg(cpu, 15, value);
                branched = true;
            } else if user_bank {
                cpu.regs.set_user(i, value);
            } else {
                cpu.regs.set(i, value);
            }
        } else {
            let mut value = if !user_bank && i == rn && rn_in_list && !rn_is_lowest_in_list {
                wb_value
            } else if user_bank {
                cpu.regs.get_user(i)
            } else {
                read_reg(cpu, i)
            };
            if i == 15 {
                // ARM7 STM PC is PC+12; skewed read is already +8.
                value = if user_bank {
                    cpu.regs.pc().wrapping_add(12)
                } else {
                    value.wrapping_add(4)
                };
            }
            bus.write32(addr & !3, value);
        }
        addr = addr.wrapping_add(4);
    }

    if writeback && rn != 15 && !(load && rn_in_list) {
        // Force-user transfers: writeback still updates the *current* mode's Rn
        // when W is set (tests avoid W with ^; keep current-bank writeback).
        cpu.regs.set(rn, wb_value);
    }

    if branched {
        note_branch(cpu);
        ExecResult::Branched
    } else {
        ExecResult::Ok
    }
}
