//! Thumb opcode decode (ARMv4T).
//!
//! Cited: GBATEK — THUMB Instruction Set
//!   https://problemkaputt.de/gbatek.htm
//! Cited: ARM DDI0100 — Thumb instruction set encodings
//! Note: format numbers follow the classic ARM ARM Thumb chapter; GBA = ARMv4T only.

/// Logical shift used by Format 1 / ALU shifts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShiftKind {
    Lsl,
    Lsr,
    Asr,
    Ror,
}

/// Thumb condition codes (Format 16). Same mnemonic order as ARM `cond`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Cond {
    Eq = 0x0,
    Ne = 0x1,
    Cs = 0x2,
    Cc = 0x3,
    Mi = 0x4,
    Pl = 0x5,
    Vs = 0x6,
    Vc = 0x7,
    Hi = 0x8,
    Ls = 0x9,
    Ge = 0xA,
    Lt = 0xB,
    Gt = 0xC,
    Le = 0xD,
    /// Encoding 0xE is undefined in Thumb conditional branch (ARMv4T).
    Al = 0xE,
}

impl Cond {
    pub fn from_u4(v: u8) -> Self {
        match v & 0xF {
            0x0 => Self::Eq,
            0x1 => Self::Ne,
            0x2 => Self::Cs,
            0x3 => Self::Cc,
            0x4 => Self::Mi,
            0x5 => Self::Pl,
            0x6 => Self::Vs,
            0x7 => Self::Vc,
            0x8 => Self::Hi,
            0x9 => Self::Ls,
            0xA => Self::Ge,
            0xB => Self::Lt,
            0xC => Self::Gt,
            0xD => Self::Le,
            _ => Self::Al,
        }
    }
}

/// Decoded Thumb instruction (architectural effect, not encoding bits).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Instr {
    /// Format 1: `LSL`/`LSR`/`ASR` Rd, Rs, #imm5
    MoveShifted {
        kind: ShiftKind,
        imm5: u8,
        rs: u8,
        rd: u8,
    },
    /// Format 2: ADD/SUB Rd, Rs, Rn|#imm3
    AddSub {
        imm: bool,
        sub: bool,
        rn_or_imm3: u8,
        rs: u8,
        rd: u8,
    },
    /// Format 3: MOV/CMP/ADD/SUB Rd, #imm8
    AluImm8 { op: AluImm8Op, rd: u8, imm8: u8 },
    /// Format 4: ALU Rd, Rs
    AluReg { op: AluRegOp, rs: u8, rd: u8 },
    /// Format 5: Hi-reg ADD/CMP/MOV or BX
    HiReg { op: HiOp, rm: u8, rd: u8 },
    /// Format 6: LDR Rd, [PC, #imm8*4]
    LdrPc { rd: u8, imm8: u8 },
    /// Format 7: LDR/STR(+B) Rd, [Rb, Ro]
    LdrStrReg {
        byte: bool,
        load: bool,
        ro: u8,
        rb: u8,
        rd: u8,
    },
    /// Format 8: LDRH/STRH/LDRSB/LDRSH Rd, [Rb, Ro]
    LdrStrSign {
        op: SignTransfer,
        ro: u8,
        rb: u8,
        rd: u8,
    },
    /// Format 9: LDR/STR(+B) Rd, [Rb, #imm5]
    LdrStrImm {
        byte: bool,
        load: bool,
        imm5: u8,
        rb: u8,
        rd: u8,
    },
    /// Format 10: LDRH/STRH Rd, [Rb, #imm5*2]
    LdrStrHalf {
        load: bool,
        imm5: u8,
        rb: u8,
        rd: u8,
    },
    /// Format 11: LDR/STR Rd, [SP, #imm8*4]
    LdrStrSp { load: bool, rd: u8, imm8: u8 },
    /// Format 12: ADD Rd, PC|SP, #imm8*4
    AddPcSp { sp: bool, rd: u8, imm8: u8 },
    /// Format 13: ADD/SUB SP, #imm7*4
    AddSubSp { sub: bool, imm7: u8 },
    /// Format 14: PUSH/POP {Rlist}[, LR|PC]
    PushPop { load: bool, pclr: bool, rlist: u8 },
    /// Format 15: LDMIA/STMIA Rb!, {Rlist}
    LdmStm { load: bool, rb: u8, rlist: u8 },
    /// Format 16: B<cond> label
    BCond { cond: Cond, imm8: i8 },
    /// Format 17: SWI
    Swi { imm8: u8 },
    /// Format 18: B label
    B { imm11: i16 },
    /// Format 19 high: BL prefix (LR = PC + off<<12)
    BlHigh { imm11: u16 },
    /// Format 19 low: BL suffix (branch + LR link)
    BlLow { imm11: u16 },
    /// Unused / undefined Thumb encoding (ARMv4T).
    Undefined,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AluImm8Op {
    Mov,
    Cmp,
    Add,
    Sub,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AluRegOp {
    And,
    Eor,
    Lsl,
    Lsr,
    Asr,
    Adc,
    Sbc,
    Ror,
    Tst,
    Neg,
    Cmp,
    Cmn,
    Orr,
    Mul,
    Bic,
    Mvn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HiOp {
    Add,
    Cmp,
    Mov,
    Bx,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignTransfer {
    Strh,
    Ldrh,
    Ldrsb,
    Ldrsh,
}

/// Decode one 16-bit Thumb opcode into an [`Instr`].
pub fn decode(op: u16) -> Instr {
    let b15_13 = (op >> 13) & 0b111;
    match b15_13 {
        0b000 => decode_shift_addsub(op),
        0b001 => {
            let opc = ((op >> 11) & 0b11) as u8;
            let rd = ((op >> 8) & 0b111) as u8;
            let imm8 = (op & 0xFF) as u8;
            let op = match opc {
                0 => AluImm8Op::Mov,
                1 => AluImm8Op::Cmp,
                2 => AluImm8Op::Add,
                _ => AluImm8Op::Sub,
            };
            Instr::AluImm8 { op, rd, imm8 }
        }
        0b010 => decode_010(op),
        0b011 => {
            let byte = (op & (1 << 12)) != 0;
            let load = (op & (1 << 11)) != 0;
            let imm5 = ((op >> 6) & 0x1F) as u8;
            let rb = ((op >> 3) & 0b111) as u8;
            let rd = (op & 0b111) as u8;
            Instr::LdrStrImm {
                byte,
                load,
                imm5,
                rb,
                rd,
            }
        }
        0b100 => decode_100(op),
        0b101 => decode_101(op),
        0b110 => decode_110(op),
        0b111 => decode_111(op),
        _ => Instr::Undefined,
    }
}

fn decode_shift_addsub(op: u16) -> Instr {
    // Format 2 occupies 00011xx…
    if (op & 0xF800) == 0x1800 {
        let imm = (op & (1 << 10)) != 0;
        let sub = (op & (1 << 9)) != 0;
        let rn_or_imm3 = ((op >> 6) & 0b111) as u8;
        let rs = ((op >> 3) & 0b111) as u8;
        let rd = (op & 0b111) as u8;
        return Instr::AddSub {
            imm,
            sub,
            rn_or_imm3,
            rs,
            rd,
        };
    }
    let kind = match (op >> 11) & 0b11 {
        0 => ShiftKind::Lsl,
        1 => ShiftKind::Lsr,
        2 => ShiftKind::Asr,
        _ => return Instr::Undefined,
    };
    let imm5 = ((op >> 6) & 0x1F) as u8;
    let rs = ((op >> 3) & 0b111) as u8;
    let rd = (op & 0b111) as u8;
    Instr::MoveShifted { kind, imm5, rs, rd }
}

fn decode_010(op: u16) -> Instr {
    if (op & 0xFC00) == 0x4000 {
        // Format 4 ALU
        let opc = ((op >> 6) & 0xF) as u8;
        let rs = ((op >> 3) & 0b111) as u8;
        let rd = (op & 0b111) as u8;
        let op = match opc {
            0x0 => AluRegOp::And,
            0x1 => AluRegOp::Eor,
            0x2 => AluRegOp::Lsl,
            0x3 => AluRegOp::Lsr,
            0x4 => AluRegOp::Asr,
            0x5 => AluRegOp::Adc,
            0x6 => AluRegOp::Sbc,
            0x7 => AluRegOp::Ror,
            0x8 => AluRegOp::Tst,
            0x9 => AluRegOp::Neg,
            0xA => AluRegOp::Cmp,
            0xB => AluRegOp::Cmn,
            0xC => AluRegOp::Orr,
            0xD => AluRegOp::Mul,
            0xE => AluRegOp::Bic,
            _ => AluRegOp::Mvn,
        };
        return Instr::AluReg { op, rs, rd };
    }
    if (op & 0xFC00) == 0x4400 {
        // Format 5 Hi / BX
        let opc = ((op >> 8) & 0b11) as u8;
        let h1 = (op & (1 << 7)) != 0;
        let h2 = (op & (1 << 6)) != 0;
        let rs = ((op >> 3) & 0b111) as u8;
        let rd = (op & 0b111) as u8;
        let rm = rs | (u8::from(h2) << 3);
        let rd = rd | (u8::from(h1) << 3);
        let op = match opc {
            0 => HiOp::Add,
            1 => HiOp::Cmp,
            2 => HiOp::Mov,
            _ => HiOp::Bx,
        };
        // BX ignores H1 / Rd; ADD/CMP/MOV with both H bits clear is undefined.
        if matches!(op, HiOp::Add | HiOp::Cmp | HiOp::Mov) && !h1 && !h2 {
            return Instr::Undefined;
        }
        return Instr::HiReg { op, rm, rd };
    }
    if (op & 0xF800) == 0x4800 {
        let rd = ((op >> 8) & 0b111) as u8;
        let imm8 = (op & 0xFF) as u8;
        return Instr::LdrPc { rd, imm8 };
    }
    // Formats 7 / 8: 0101…
    if (op & 0xF000) == 0x5000 {
        let bit11 = (op & (1 << 11)) != 0;
        let bit10 = (op & (1 << 10)) != 0;
        let bit9 = (op & (1 << 9)) != 0;
        let ro = ((op >> 6) & 0b111) as u8;
        let rb = ((op >> 3) & 0b111) as u8;
        let rd = (op & 0b111) as u8;
        if !bit9 {
            // Format 7
            return Instr::LdrStrReg {
                byte: bit10,
                load: bit11,
                ro,
                rb,
                rd,
            };
        }
        // Format 8
        let op = match (bit11, bit10) {
            (false, false) => SignTransfer::Strh,
            (false, true) => SignTransfer::Ldrsb,
            (true, false) => SignTransfer::Ldrh,
            (true, true) => SignTransfer::Ldrsh,
        };
        return Instr::LdrStrSign { op, ro, rb, rd };
    }
    Instr::Undefined
}

fn decode_100(op: u16) -> Instr {
    if (op & 0xF000) == 0x8000 {
        let load = (op & (1 << 11)) != 0;
        let imm5 = ((op >> 6) & 0x1F) as u8;
        let rb = ((op >> 3) & 0b111) as u8;
        let rd = (op & 0b111) as u8;
        return Instr::LdrStrHalf { load, imm5, rb, rd };
    }
    // Format 11 SP-relative
    let load = (op & (1 << 11)) != 0;
    let rd = ((op >> 8) & 0b111) as u8;
    let imm8 = (op & 0xFF) as u8;
    Instr::LdrStrSp { load, rd, imm8 }
}

fn decode_101(op: u16) -> Instr {
    if (op & 0xF000) == 0xA000 {
        let sp = (op & (1 << 11)) != 0;
        let rd = ((op >> 8) & 0b111) as u8;
        let imm8 = (op & 0xFF) as u8;
        return Instr::AddPcSp { sp, rd, imm8 };
    }
    // 1011….
    if (op & 0xFF00) == 0xB000 {
        let sub = (op & (1 << 7)) != 0;
        let imm7 = (op & 0x7F) as u8;
        return Instr::AddSubSp { sub, imm7 };
    }
    if (op & 0xF600) == 0xB400 {
        // PUSH/POP: 1011 L 10 R …
        let load = (op & (1 << 11)) != 0;
        let pclr = (op & (1 << 8)) != 0;
        let rlist = (op & 0xFF) as u8;
        return Instr::PushPop { load, pclr, rlist };
    }
    Instr::Undefined
}

fn decode_110(op: u16) -> Instr {
    if (op & 0xF000) == 0xC000 {
        let load = (op & (1 << 11)) != 0;
        let rb = ((op >> 8) & 0b111) as u8;
        let rlist = (op & 0xFF) as u8;
        return Instr::LdmStm { load, rb, rlist };
    }
    // Conditional branch / SWI
    let cond = ((op >> 8) & 0xF) as u8;
    if cond == 0xF {
        return Instr::Swi {
            imm8: (op & 0xFF) as u8,
        };
    }
    if cond == 0xE {
        return Instr::Undefined;
    }
    Instr::BCond {
        cond: Cond::from_u4(cond),
        imm8: (op & 0xFF) as i8,
    }
}

fn decode_111(op: u16) -> Instr {
    match (op >> 11) & 0b11 {
        0b00 => {
            // Sign-extend 11-bit offset << 1 later in execute
            let raw = op & 0x7FF;
            let imm11 = if raw & 0x400 != 0 {
                (raw | 0xF800) as i16
            } else {
                raw as i16
            };
            Instr::B { imm11 }
        }
        0b10 => Instr::BlHigh { imm11: op & 0x7FF },
        0b11 => Instr::BlLow { imm11: op & 0x7FF },
        // 01 = undefined on ARMv4T (BLX imm is v5)
        _ => Instr::Undefined,
    }
}
