//! Thumb instruction execute (ARMv4T).
//!
//! Cited: GBATEK — THUMB Instruction Set / memory alignments / cycle notes
//!   https://problemkaputt.de/gbatek.htm
//! Cited: ARM DDI0100 — Thumb instruction set operations
//! Cited: ARM7TDMI TRM (DDI0210C) — PC reads = executing + 4 in Thumb
//! Note: PC-relative ADD/LDR force-align PC (`(pc+4) & !2`). Misaligned LDRH/LDRSH
//! follow GBATEK ARM7 quirks. Cycle accounting is crude (P1); bus waitstates are P2.

use super::alu::{self, set_nz, shift};
use super::decode::{AluImm8Op, AluRegOp, Cond, HiOp, Instr, ShiftKind, SignTransfer};
use super::ThumbCtx;

/// Result of executing one Thumb opcode (for pipeline / exception owners).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecResult {
    /// Advance fetch PC by 2 (normal fall-through).
    Continue,
    /// PC already written (branch / BX / POP PC / ALU→R15).
    Branch,
    /// SWI raised via [`super::ThumbCore::raise_swi`].
    Swi,
    /// Undefined encoding raised via [`super::ThumbCore::raise_undef`].
    Undef,
}

/// Execute a previously decoded Thumb instruction.
///
/// `instr_pc` is the address of the halfword being executed (not the skewed R15).
/// [`super::ThumbCore::reg`](15) must equal `instr_pc.wrapping_add(4)` on entry for
/// non-force-aligned PC reads.
pub fn execute<C: ThumbCtx>(ctx: &mut C, instr_pc: u32, instr: Instr) -> ExecResult {
    match instr {
        Instr::MoveShifted { kind, imm5, rs, rd } => {
            let amount = match kind {
                ShiftKind::Lsl => u32::from(imm5),
                ShiftKind::Lsr | ShiftKind::Asr => {
                    if imm5 == 0 {
                        32
                    } else {
                        u32::from(imm5)
                    }
                }
                ShiftKind::Ror => u32::from(imm5),
            };
            let (r, c) = shift(kind, ctx.reg(rs), amount, ctx.c());
            let (n, z) = set_nz(r);
            ctx.set_nzcv(n, z, c, ctx.v());
            write_rd(ctx, rd, r)
        }
        Instr::AddSub {
            imm,
            sub,
            rn_or_imm3,
            rs,
            rd,
        } => {
            let a = ctx.reg(rs);
            let b = if imm {
                u32::from(rn_or_imm3)
            } else {
                ctx.reg(rn_or_imm3)
            };
            let (r, n, z, c, v) = if sub {
                alu::sub_flags(a, b)
            } else {
                alu::add_flags(a, b)
            };
            ctx.set_nzcv(n, z, c, v);
            write_rd(ctx, rd, r)
        }
        Instr::AluImm8 { op, rd, imm8 } => {
            let imm = u32::from(imm8);
            match op {
                AluImm8Op::Mov => {
                    let (n, z) = set_nz(imm);
                    ctx.set_nzcv(n, z, ctx.c(), ctx.v());
                    write_rd(ctx, rd, imm)
                }
                AluImm8Op::Cmp => {
                    let (r, n, z, c, v) = alu::sub_flags(ctx.reg(rd), imm);
                    let _ = r;
                    ctx.set_nzcv(n, z, c, v);
                    ExecResult::Continue
                }
                AluImm8Op::Add => {
                    let (r, n, z, c, v) = alu::add_flags(ctx.reg(rd), imm);
                    ctx.set_nzcv(n, z, c, v);
                    write_rd(ctx, rd, r)
                }
                AluImm8Op::Sub => {
                    let (r, n, z, c, v) = alu::sub_flags(ctx.reg(rd), imm);
                    ctx.set_nzcv(n, z, c, v);
                    write_rd(ctx, rd, r)
                }
            }
        }
        Instr::AluReg { op, rs, rd } => exec_alu_reg(ctx, op, rs, rd),
        Instr::HiReg { op, rm, rd } => exec_hi(ctx, op, rm, rd),
        Instr::LdrPc { rd, imm8 } => {
            let base = (instr_pc.wrapping_add(4)) & !2;
            let addr = base.wrapping_add(u32::from(imm8) << 2);
            let val = load32_arm7(ctx, addr);
            write_rd(ctx, rd, val)
        }
        Instr::LdrStrReg {
            byte,
            load,
            ro,
            rb,
            rd,
        } => {
            let addr = ctx.reg(rb).wrapping_add(ctx.reg(ro));
            if load {
                let val = if byte {
                    u32::from(ctx.read8(addr))
                } else {
                    load32_arm7(ctx, addr)
                };
                write_rd(ctx, rd, val)
            } else {
                if byte {
                    ctx.write8(addr, ctx.reg(rd) as u8);
                } else {
                    store32_arm7(ctx, addr, ctx.reg(rd));
                }
                ExecResult::Continue
            }
        }
        Instr::LdrStrSign { op, ro, rb, rd } => {
            let addr = ctx.reg(rb).wrapping_add(ctx.reg(ro));
            match op {
                SignTransfer::Strh => {
                    store16_arm7(ctx, addr, ctx.reg(rd) as u16);
                    ExecResult::Continue
                }
                SignTransfer::Ldrh => {
                    let val = load16_zx_arm7(ctx, addr);
                    write_rd(ctx, rd, val)
                }
                SignTransfer::Ldrsb => {
                    let val = ctx.read8(addr) as i8 as i32 as u32;
                    write_rd(ctx, rd, val)
                }
                SignTransfer::Ldrsh => {
                    // Odd LDRSH behaves like LDRSB on ARM7/GBA (GBATEK alignments).
                    let val = if addr & 1 != 0 {
                        ctx.read8(addr) as i8 as i32 as u32
                    } else {
                        ctx.read16(addr) as i16 as i32 as u32
                    };
                    write_rd(ctx, rd, val)
                }
            }
        }
        Instr::LdrStrImm {
            byte,
            load,
            imm5,
            rb,
            rd,
        } => {
            let offset = if byte {
                u32::from(imm5)
            } else {
                u32::from(imm5) << 2
            };
            let addr = ctx.reg(rb).wrapping_add(offset);
            if load {
                let val = if byte {
                    u32::from(ctx.read8(addr))
                } else {
                    load32_arm7(ctx, addr)
                };
                write_rd(ctx, rd, val)
            } else {
                if byte {
                    ctx.write8(addr, ctx.reg(rd) as u8);
                } else {
                    store32_arm7(ctx, addr, ctx.reg(rd));
                }
                ExecResult::Continue
            }
        }
        Instr::LdrStrHalf { load, imm5, rb, rd } => {
            let addr = ctx.reg(rb).wrapping_add(u32::from(imm5) << 1);
            if load {
                let val = load16_zx_arm7(ctx, addr);
                write_rd(ctx, rd, val)
            } else {
                store16_arm7(ctx, addr, ctx.reg(rd) as u16);
                ExecResult::Continue
            }
        }
        Instr::LdrStrSp { load, rd, imm8 } => {
            let addr = ctx.reg(13).wrapping_add(u32::from(imm8) << 2);
            if load {
                let val = load32_arm7(ctx, addr);
                write_rd(ctx, rd, val)
            } else {
                store32_arm7(ctx, addr, ctx.reg(rd));
                ExecResult::Continue
            }
        }
        Instr::AddPcSp { sp, rd, imm8 } => {
            let base = if sp {
                ctx.reg(13)
            } else {
                (instr_pc.wrapping_add(4)) & !2
            };
            write_rd(ctx, rd, base.wrapping_add(u32::from(imm8) << 2))
        }
        Instr::AddSubSp { sub, imm7 } => {
            let imm = u32::from(imm7) << 2;
            let sp = ctx.reg(13);
            let r = if sub {
                sp.wrapping_sub(imm)
            } else {
                sp.wrapping_add(imm)
            };
            ctx.set_reg(13, r);
            ExecResult::Continue
        }
        Instr::PushPop { load, pclr, rlist } => {
            if load {
                exec_pop(ctx, pclr, rlist)
            } else {
                exec_push(ctx, pclr, rlist);
                ExecResult::Continue
            }
        }
        Instr::LdmStm { load, rb, rlist } => exec_ldm_stm(ctx, load, rb, rlist),
        Instr::BCond { cond, imm8 } => {
            if eval_cond(ctx, cond) {
                let off = (i32::from(imm8)) << 1;
                let target = instr_pc.wrapping_add(4).wrapping_add(off as u32);
                set_pc(ctx, target);
                ExecResult::Branch
            } else {
                ExecResult::Continue
            }
        }
        Instr::Swi { imm8 } => {
            ctx.raise_swi(imm8);
            ExecResult::Swi
        }
        Instr::B { imm11 } => {
            let off = (i32::from(imm11)) << 1;
            let target = instr_pc.wrapping_add(4).wrapping_add(off as u32);
            set_pc(ctx, target);
            ExecResult::Branch
        }
        Instr::BlHigh { imm11 } => {
            // Sign-extend 11-bit offset, then << 12; LR = (instr+4) + offset.
            let signed = ((imm11 as i32) << 21) >> 21;
            let lr = instr_pc.wrapping_add(4).wrapping_add((signed << 12) as u32);
            ctx.set_reg(14, lr);
            ExecResult::Continue
        }
        Instr::BlLow { imm11 } => {
            let next = instr_pc.wrapping_add(2);
            let target = ctx.reg(14).wrapping_add(u32::from(imm11) << 1);
            ctx.set_reg(14, next | 1);
            set_pc(ctx, target);
            ExecResult::Branch
        }
        Instr::Undefined => {
            ctx.raise_undef();
            ExecResult::Undef
        }
    }
}

fn exec_alu_reg<C: ThumbCtx>(ctx: &mut C, op: AluRegOp, rs: u8, rd: u8) -> ExecResult {
    let a = ctx.reg(rd);
    let b = ctx.reg(rs);
    match op {
        AluRegOp::And => {
            let r = a & b;
            let (n, z) = set_nz(r);
            ctx.set_nzcv(n, z, ctx.c(), ctx.v());
            write_rd(ctx, rd, r)
        }
        AluRegOp::Eor => {
            let r = a ^ b;
            let (n, z) = set_nz(r);
            ctx.set_nzcv(n, z, ctx.c(), ctx.v());
            write_rd(ctx, rd, r)
        }
        AluRegOp::Lsl => {
            let (r, c) = alu::lsl(a, b & 0xFF, ctx.c());
            let (n, z) = set_nz(r);
            ctx.set_nzcv(n, z, c, ctx.v());
            write_rd(ctx, rd, r)
        }
        AluRegOp::Lsr => {
            let (r, c) = alu::lsr(a, b & 0xFF, ctx.c());
            let (n, z) = set_nz(r);
            ctx.set_nzcv(n, z, c, ctx.v());
            write_rd(ctx, rd, r)
        }
        AluRegOp::Asr => {
            let (r, c) = alu::asr(a, b & 0xFF, ctx.c());
            let (n, z) = set_nz(r);
            ctx.set_nzcv(n, z, c, ctx.v());
            write_rd(ctx, rd, r)
        }
        AluRegOp::Ror => {
            let (r, c) = alu::ror(a, b & 0xFF, ctx.c());
            let (n, z) = set_nz(r);
            ctx.set_nzcv(n, z, c, ctx.v());
            write_rd(ctx, rd, r)
        }
        AluRegOp::Adc => {
            let (r, c, v) = alu::add_with_carry(a, b, ctx.c());
            let (n, z) = set_nz(r);
            ctx.set_nzcv(n, z, c, v);
            write_rd(ctx, rd, r)
        }
        AluRegOp::Sbc => {
            let (r, c, v) = alu::sub_with_carry(a, b, ctx.c());
            let (n, z) = set_nz(r);
            ctx.set_nzcv(n, z, c, v);
            write_rd(ctx, rd, r)
        }
        AluRegOp::Tst => {
            let r = a & b;
            let (n, z) = set_nz(r);
            ctx.set_nzcv(n, z, ctx.c(), ctx.v());
            ExecResult::Continue
        }
        AluRegOp::Neg => {
            let (r, n, z, c, v) = alu::sub_flags(0, b);
            ctx.set_nzcv(n, z, c, v);
            write_rd(ctx, rd, r)
        }
        AluRegOp::Cmp => {
            let (_r, n, z, c, v) = alu::sub_flags(a, b);
            ctx.set_nzcv(n, z, c, v);
            ExecResult::Continue
        }
        AluRegOp::Cmn => {
            let (_r, n, z, c, v) = alu::add_flags(a, b);
            ctx.set_nzcv(n, z, c, v);
            ExecResult::Continue
        }
        AluRegOp::Orr => {
            let r = a | b;
            let (n, z) = set_nz(r);
            ctx.set_nzcv(n, z, ctx.c(), ctx.v());
            write_rd(ctx, rd, r)
        }
        AluRegOp::Mul => {
            let r = a.wrapping_mul(b);
            let (n, z) = set_nz(r);
            // ARMv4 MUL: C is unpredictable; keep previous C (common emu choice).
            ctx.set_nzcv(n, z, ctx.c(), ctx.v());
            write_rd(ctx, rd, r)
        }
        AluRegOp::Bic => {
            let r = a & !b;
            let (n, z) = set_nz(r);
            ctx.set_nzcv(n, z, ctx.c(), ctx.v());
            write_rd(ctx, rd, r)
        }
        AluRegOp::Mvn => {
            let r = !b;
            let (n, z) = set_nz(r);
            ctx.set_nzcv(n, z, ctx.c(), ctx.v());
            write_rd(ctx, rd, r)
        }
    }
}

fn exec_hi<C: ThumbCtx>(ctx: &mut C, op: HiOp, rm: u8, rd: u8) -> ExecResult {
    match op {
        HiOp::Add => {
            let r = ctx.reg(rd).wrapping_add(ctx.reg(rm));
            write_rd(ctx, rd, r)
        }
        HiOp::Cmp => {
            let (_r, n, z, c, v) = alu::sub_flags(ctx.reg(rd), ctx.reg(rm));
            ctx.set_nzcv(n, z, c, v);
            ExecResult::Continue
        }
        HiOp::Mov => write_rd(ctx, rd, ctx.reg(rm)),
        HiOp::Bx => {
            let target = ctx.reg(rm);
            let thumb = (target & 1) != 0;
            ctx.set_thumb_state(thumb);
            if thumb {
                set_pc(ctx, target & !1);
            } else {
                set_pc(ctx, target & !3);
            }
            ExecResult::Branch
        }
    }
}

fn exec_push<C: ThumbCtx>(ctx: &mut C, lr: bool, rlist: u8) {
    let mut sp = ctx.reg(13);
    let count = rlist.count_ones() + u32::from(lr);
    sp = sp.wrapping_sub(count * 4);
    let mut addr = sp;
    for i in 0..8u8 {
        if rlist & (1 << i) != 0 {
            store32_arm7(ctx, addr, ctx.reg(i));
            addr = addr.wrapping_add(4);
        }
    }
    if lr {
        store32_arm7(ctx, addr, ctx.reg(14));
    }
    ctx.set_reg(13, sp);
}

fn exec_pop<C: ThumbCtx>(ctx: &mut C, pc: bool, rlist: u8) -> ExecResult {
    let mut addr = ctx.reg(13);
    for i in 0..8u8 {
        if rlist & (1 << i) != 0 {
            let v = load32_arm7(ctx, addr);
            ctx.set_reg(i, v);
            addr = addr.wrapping_add(4);
        }
    }
    let mut branched = false;
    if pc {
        let v = load32_arm7(ctx, addr);
        addr = addr.wrapping_add(4);
        // ARMv4T POP PC does not interwork via bit0 (BX does). Stay in Thumb; align.
        set_pc(ctx, v & !1);
        branched = true;
    }
    ctx.set_reg(13, addr);
    if branched {
        ExecResult::Branch
    } else {
        ExecResult::Continue
    }
}

fn exec_ldm_stm<C: ThumbCtx>(ctx: &mut C, load: bool, rb: u8, rlist: u8) -> ExecResult {
    // Empty rlist is unpredictable on ARM7; treat as no-op with base unchanged (TBD).
    let mut addr = ctx.reg(rb);
    let start = addr;
    if load {
        for i in 0..8u8 {
            if rlist & (1 << i) != 0 {
                let v = load32_arm7(ctx, addr);
                ctx.set_reg(i, v);
                addr = addr.wrapping_add(4);
            }
        }
        // Writeback unless Rb in list
        if rlist & (1 << rb) == 0 {
            ctx.set_reg(rb, addr);
        } else {
            // base in list: writeback suppressed (ARM ARM); leave as loaded value
            let _ = start;
        }
        ExecResult::Continue
    } else {
        for i in 0..8u8 {
            if rlist & (1 << i) != 0 {
                // If store of base and base in list: store original base (ARM)
                let val = if i == rb { start } else { ctx.reg(i) };
                store32_arm7(ctx, addr, val);
                addr = addr.wrapping_add(4);
            }
        }
        ctx.set_reg(rb, addr);
        ExecResult::Continue
    }
}

fn write_rd<C: ThumbCtx>(ctx: &mut C, rd: u8, val: u32) -> ExecResult {
    if rd == 15 {
        set_pc(ctx, val & !1);
        ExecResult::Branch
    } else {
        ctx.set_reg(rd, val);
        ExecResult::Continue
    }
}

fn set_pc<C: ThumbCtx>(ctx: &mut C, pc: u32) {
    ctx.set_reg(15, pc & !1);
}

fn eval_cond<C: ThumbCtx>(ctx: &C, cond: Cond) -> bool {
    let n = ctx.n();
    let z = ctx.z();
    let c = ctx.c();
    let v = ctx.v();
    match cond {
        Cond::Eq => z,
        Cond::Ne => !z,
        Cond::Cs => c,
        Cond::Cc => !c,
        Cond::Mi => n,
        Cond::Pl => !n,
        Cond::Vs => v,
        Cond::Vc => !v,
        Cond::Hi => c && !z,
        Cond::Ls => !c || z,
        Cond::Ge => n == v,
        Cond::Lt => n != v,
        Cond::Gt => !z && n == v,
        Cond::Le => z || n != v,
        Cond::Al => true,
    }
}

/// ARM7 LDR: align down, ROR by (addr&3)*8.
fn load32_arm7<C: ThumbCtx>(ctx: &mut C, addr: u32) -> u32 {
    let aligned = addr & !3;
    let data = ctx.read32(aligned);
    let rot = (addr & 3) * 8;
    if rot == 0 {
        data
    } else {
        data.rotate_right(rot)
    }
}

fn store32_arm7<C: ThumbCtx>(ctx: &mut C, addr: u32, val: u32) {
    ctx.write32(addr & !3, val);
}

/// ARM7 LDRH: aligned = zero-extend halfword; odd = `[addr&~1]` then **ROR 8**
/// (bits land in 0–7 and 24–31 per GBATEK).
fn load16_zx_arm7<C: ThumbCtx>(ctx: &mut C, addr: u32) -> u32 {
    let half = u32::from(ctx.read16(addr & !1));
    if addr & 1 != 0 {
        half.rotate_right(8)
    } else {
        half
    }
}

fn store16_arm7<C: ThumbCtx>(ctx: &mut C, addr: u32, val: u16) {
    ctx.write16(addr & !1, val);
}
