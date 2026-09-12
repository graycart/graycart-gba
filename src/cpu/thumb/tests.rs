//! Thumb unit tests — decode + execute against [`Regs`] + flat RAM.
//!
//! Cited: GBATEK — THUMB Instruction Set
//!   https://problemkaputt.de/gbatek.htm
//! Acceptance: decode tables + ALU/load/BX/BL smoke; jsmolka thumb.gba is the
//! integration gate (pipeline / bus streams).

use super::*;
use crate::cpu::Regs;
use decode::{AluImm8Op, AluRegOp, HiOp, Instr, ShiftKind};

struct FlatMem {
    data: Vec<u8>,
}

impl FlatMem {
    fn new(size: usize) -> Self {
        Self {
            data: vec![0; size],
        }
    }
}

impl ThumbMem for FlatMem {
    fn read8(&mut self, addr: u32) -> u8 {
        self.data.get(addr as usize).copied().unwrap_or(0)
    }
    fn write8(&mut self, addr: u32, val: u8) {
        if let Some(slot) = self.data.get_mut(addr as usize) {
            *slot = val;
        }
    }
    fn read16(&mut self, addr: u32) -> u16 {
        let lo = u16::from(self.read8(addr));
        let hi = u16::from(self.read8(addr.wrapping_add(1)));
        lo | (hi << 8)
    }
    fn write16(&mut self, addr: u32, val: u16) {
        self.write8(addr, val as u8);
        self.write8(addr.wrapping_add(1), (val >> 8) as u8);
    }
    fn read32(&mut self, addr: u32) -> u32 {
        let lo = u32::from(self.read16(addr));
        let hi = u32::from(self.read16(addr.wrapping_add(2)));
        lo | (hi << 16)
    }
    fn write32(&mut self, addr: u32, val: u32) {
        self.write16(addr, val as u16);
        self.write16(addr.wrapping_add(2), (val >> 16) as u16);
    }
}

struct Harness {
    regs: Regs,
    mem: FlatMem,
    last_swi: Option<u8>,
    undef: bool,
}

impl Harness {
    fn new() -> Self {
        let mut regs = Regs::new();
        regs.set_thumb(true);
        Self {
            regs,
            mem: FlatMem::new(0x4000),
            last_swi: None,
            undef: false,
        }
    }

    fn run(&mut self, instr_pc: u32, op: u16) -> ExecResult {
        // Pipeline skew: R15 reads as executing + 4 while this instruction runs.
        self.regs.set(15, instr_pc.wrapping_add(4));
        let instr = decode(op);
        execute(self, instr_pc, instr)
    }
}

impl ThumbCore for Harness {
    fn reg(&self, n: u8) -> u32 {
        self.regs.get(n)
    }
    fn set_reg(&mut self, n: u8, val: u32) {
        self.regs.set(n, val);
    }
    fn n(&self) -> bool {
        self.regs.n()
    }
    fn z(&self) -> bool {
        self.regs.z()
    }
    fn c(&self) -> bool {
        self.regs.c()
    }
    fn v(&self) -> bool {
        self.regs.v()
    }
    fn set_nzcv(&mut self, n: bool, z: bool, c: bool, v: bool) {
        self.regs.set_nzcv(n, z, c, v);
    }
    fn thumb_state(&self) -> bool {
        self.regs.thumb()
    }
    fn set_thumb_state(&mut self, thumb: bool) {
        self.regs.set_thumb(thumb);
    }
    fn raise_swi(&mut self, imm8: u8) {
        self.last_swi = Some(imm8);
    }
    fn raise_undef(&mut self) {
        self.undef = true;
    }
}

impl ThumbMem for Harness {
    fn read8(&mut self, addr: u32) -> u8 {
        self.mem.read8(addr)
    }
    fn write8(&mut self, addr: u32, val: u8) {
        self.mem.write8(addr, val);
    }
    fn read16(&mut self, addr: u32) -> u16 {
        self.mem.read16(addr)
    }
    fn write16(&mut self, addr: u32, val: u16) {
        self.mem.write16(addr, val);
    }
    fn read32(&mut self, addr: u32) -> u32 {
        self.mem.read32(addr)
    }
    fn write32(&mut self, addr: u32, val: u32) {
        self.mem.write32(addr, val);
    }
}

#[test]
fn decode_format1_lsl() {
    // LSL r1, r2, #5
    let op = (5 << 6) | (2 << 3) | 1;
    assert_eq!(
        decode(op),
        Instr::MoveShifted {
            kind: ShiftKind::Lsl,
            imm5: 5,
            rs: 2,
            rd: 1,
        }
    );
}

#[test]
fn decode_add_imm3_and_alu_and() {
    // ADD r0, r1, #3 => Format2 I=1 Op=0 imm3=3 rs=1 rd=0
    let op = 0x1C00 | (1 << 10) | (3 << 6) | (1 << 3);
    assert_eq!(
        decode(op),
        Instr::AddSub {
            imm: true,
            sub: false,
            rn_or_imm3: 3,
            rs: 1,
            rd: 0,
        }
    );
    // AND r3, r4 => 010000_0000_100_011
    let and = 0x4000 | (4 << 3) | 3;
    assert_eq!(
        decode(and),
        Instr::AluReg {
            op: AluRegOp::And,
            rs: 4,
            rd: 3,
        }
    );
}

#[test]
fn decode_bx_and_swi_and_bl() {
    // BX r14 => Format5 Op=11 H1=0 H2=1 Rs=6 Rd=0 => 010001_11_0_1_110_000
    let bx = 0x4700 | (1 << 6) | (6 << 3);
    assert_eq!(
        decode(bx),
        Instr::HiReg {
            op: HiOp::Bx,
            rm: 14,
            rd: 0,
        }
    );
    assert_eq!(decode(0xDF42), Instr::Swi { imm8: 0x42 });
    assert_eq!(decode(0xF000), Instr::BlHigh { imm11: 0 });
    assert_eq!(decode(0xF801), Instr::BlLow { imm11: 1 });
}

#[test]
fn decode_hi_reg_both_lo_is_undef() {
    // ADD with H1=H2=0 is undefined (lo/lo belongs to Format 4 / other forms).
    let op = 0x4400; // ADD rd,rs with both H clear
    assert_eq!(decode(op), Instr::Undefined);
}

#[test]
fn exec_mov_imm_sets_flags() {
    let mut h = Harness::new();
    // MOV r2, #0
    let op = 0x2000 | (2 << 8);
    assert_eq!(
        decode(op),
        Instr::AluImm8 {
            op: AluImm8Op::Mov,
            rd: 2,
            imm8: 0,
        }
    );
    assert_eq!(h.run(0x200, op), ExecResult::Continue);
    assert_eq!(h.regs.get(2), 0);
    assert!(h.regs.z());
    assert!(!h.regs.n());
}

#[test]
fn exec_add_sub_and_cmp() {
    let mut h = Harness::new();
    h.regs.set(1, 10);
    // ADD r0, r1, #5
    let add = 0x1C00 | (1 << 10) | (5 << 6) | (1 << 3);
    h.run(0x100, add);
    assert_eq!(h.regs.get(0), 15);
    // CMP r0, #15
    let cmp = 0x2800 | 15;
    h.run(0x102, cmp);
    assert!(h.regs.z());
    assert!(h.regs.c()); // no borrow
}

#[test]
fn exec_lsl_imm_carry() {
    let mut h = Harness::new();
    h.regs.set(2, 0x8000_0001);
    // LSL r1, r2, #1
    let op = (1 << 6) | (2 << 3) | 1;
    h.run(0x100, op as u16);
    assert_eq!(h.regs.get(1), 0x0000_0002);
    assert!(h.regs.c());
    assert!(!h.regs.n());
    assert!(!h.regs.z());
}

#[test]
fn exec_ldr_pc_relative_word_align() {
    let mut h = Harness::new();
    // instr at 0x102 (not word-aligned): base = (0x102+4)&!2 = 0x104
    h.mem.write32(0x104, 0xAABB_CCDD);
    // LDR r3, [PC, #0]
    let op = 0x4800 | (3 << 8);
    h.run(0x102, op);
    assert_eq!(h.regs.get(3), 0xAABB_CCDD);
}

#[test]
fn exec_str_ldr_imm_and_push_pop() {
    let mut h = Harness::new();
    h.regs.set(0, 0x1111_2222);
    h.regs.set(1, 0x1000); // base
    h.regs.set(13, 0x2000);
    h.regs.set(14, 0xDEAD_BEEF);
    // STR r0, [r1, #4]  => B=0 L=0 imm5=1 (word)
    let str = 0x6000 | (1 << 6) | (1 << 3);
    h.run(0x100, str);
    assert_eq!(h.mem.read32(0x1004), 0x1111_2222);
    // PUSH {r0, lr}
    let push = 0xB501; // L=0, R=1, rlist bit0
    h.run(0x102, push);
    assert_eq!(h.regs.get(13), 0x1FF8);
    assert_eq!(h.mem.read32(0x1FF8), 0x1111_2222);
    assert_eq!(h.mem.read32(0x1FFC), 0xDEAD_BEEF);
    // POP {r2, pc}
    let pop = 0xBD04; // L=1 R=1 rlist bit2
    assert_eq!(h.run(0x104, pop), ExecResult::Branch);
    assert_eq!(h.regs.get(2), 0x1111_2222);
    assert_eq!(h.regs.get(15) & !1, 0xDEAD_BEEF & !1);
    assert_eq!(h.regs.get(13), 0x2000);
}

#[test]
fn exec_bx_to_arm_clears_t() {
    let mut h = Harness::new();
    h.regs.set(3, 0x0800_0100); // ARM target (bit0 clear)
                                // BX r3
    let bx = 0x4718; // 010001_11_0_0_011_000 — wait H2 for r3: rs=3 H2=0
    assert_eq!(
        decode(bx),
        Instr::HiReg {
            op: HiOp::Bx,
            rm: 3,
            rd: 0,
        }
    );
    assert_eq!(h.run(0x200, bx), ExecResult::Branch);
    assert!(!h.regs.thumb());
    assert_eq!(h.regs.pc(), 0x0800_0100);
}

#[test]
fn exec_bx_to_thumb_sets_t() {
    let mut h = Harness::new();
    h.regs.set(0, 0x0800_0201);
    let bx = 0x4700; // BX r0
    assert_eq!(h.run(0x200, bx), ExecResult::Branch);
    assert!(h.regs.thumb());
    assert_eq!(h.regs.pc(), 0x0800_0200);
}

#[test]
fn exec_bl_pair() {
    let mut h = Harness::new();
    // At 0x800: BL to 0x800+4+0+2 = 0x806 via high=0 low=1
    h.run(0x800, 0xF000);
    assert_eq!(h.regs.get(14), 0x804); // pc+4 + 0
    assert_eq!(h.run(0x802, 0xF801), ExecResult::Branch);
    assert_eq!(h.regs.pc(), 0x806);
    assert_eq!(h.regs.get(14), 0x804 | 1); // return addr | 1
}

#[test]
fn exec_swi_and_undef() {
    let mut h = Harness::new();
    assert_eq!(h.run(0, 0xDF12), ExecResult::Swi);
    assert_eq!(h.last_swi, Some(0x12));
    assert_eq!(h.run(2, 0x4400), ExecResult::Undef);
    assert!(h.undef);
}

#[test]
fn exec_b_cond_taken_and_not() {
    let mut h = Harness::new();
    h.regs.set_nzcv(false, true, false, false); // Z
                                                // BEQ +2 (imm8=1 → +2 from pc+4 → target instr+6)
    let beq = 0xD001;
    assert_eq!(h.run(0x100, beq), ExecResult::Branch);
    assert_eq!(h.regs.pc(), 0x106);
    h.regs.set_nzcv(false, false, false, false);
    assert_eq!(h.run(0x200, beq), ExecResult::Continue);
}

#[test]
fn misaligned_ldrh_ror8() {
    let mut h = Harness::new();
    h.regs.set(1, 0x1001); // odd
    h.regs.set(2, 0); // offset reg
    h.mem.write16(0x1000, 0xAABB);
    // LDRH r0, [r1, r2] — Format8 H=1 S=0 bit9=1 → 0x5A00 | …
    // bit11=1 bit10=0 bit9=1: LDRH
    let op = 0x5A00 | (2 << 6) | (1 << 3);
    assert_eq!(
        decode(op),
        Instr::LdrStrSign {
            op: decode::SignTransfer::Ldrh,
            ro: 2,
            rb: 1,
            rd: 0,
        }
    );
    h.run(0x100, op);
    // halfword 0xAABB → ROR 8 → 0xBB0000AA (bits in 0–7 and 24–31)
    assert_eq!(h.regs.get(0), 0xBB00_00AA);
}
