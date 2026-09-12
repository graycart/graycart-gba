//! ARM instruction decode (ARMv4T classic encodings).
//!
//! Cited: GBATEK — CPU Instruction Set / Opcode Summary
//!   https://problemkaputt.de/gbatek.htm
//! Cited: ARM DDI0100 / DDI0210C — ARM instruction encodings
//! Note: prefer known tables; Undefined only where ARM specifies.

use super::shifter::ShiftType;

/// Decoded ARM instruction (condition already stripped for dispatch; stored too).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Decoded {
    pub cond: u8,
    pub op: Op,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    DataProcessing {
        opcode: u8,
        s: bool,
        rn: u8,
        rd: u8,
        op2: Op2,
    },
    /// Multiply / multiply-accumulate (32-bit).
    Multiply {
        a: bool,
        s: bool,
        rd: u8,
        rn: u8,
        rs: u8,
        rm: u8,
    },
    /// Multiply long (signed/unsigned, accumulate).
    MultiplyLong {
        signed: bool,
        a: bool,
        s: bool,
        rd_hi: u8,
        rd_lo: u8,
        rs: u8,
        rm: u8,
    },
    /// Single data swap.
    Swap {
        byte: bool,
        rn: u8,
        rd: u8,
        rm: u8,
    },
    /// Branch and exchange.
    Bx {
        rm: u8,
    },
    /// Halfword / signed byte transfers (LDRH/STRH/LDRSB/LDRSH).
    HalfwordTransfer {
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
    },
    /// Single data transfer (LDR/STR +B).
    SingleTransfer {
        load: bool,
        writeback: bool,
        byte: bool,
        up: bool,
        pre: bool,
        rn: u8,
        rd: u8,
        offset: SingleOffset,
    },
    /// Block data transfer (LDM/STM).
    BlockTransfer {
        load: bool,
        writeback: bool,
        s_bit: bool,
        up: bool,
        pre: bool,
        rn: u8,
        rlist: u16,
    },
    Branch {
        link: bool,
        offset: i32,
    },
    SoftwareInterrupt {
        comment: u32,
    },
    Mrs {
        spsr: bool,
        rd: u8,
    },
    Msr {
        spsr: bool,
        /// Field mask bits 19–16 (c/x/s/f).
        fields: u8,
        src: MsrSrc,
    },
    Undefined,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op2 {
    Imm { imm8: u8, rot: u8 },
    RegImmShift { rm: u8, ty: ShiftType, imm: u8 },
    RegRegShift { rm: u8, ty: ShiftType, rs: u8 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SingleOffset {
    Imm(u16),
    Reg { rm: u8, ty: ShiftType, imm: u8 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MsrSrc {
    Imm { imm8: u8, rot: u8 },
    Reg { rm: u8 },
}

/// Decode a 32-bit ARM opcode word into a structured `Decoded`.
pub fn decode(raw: u32) -> Decoded {
    let cond = ((raw >> 28) & 0xF) as u8;
    let op = decode_op(raw);
    Decoded { cond, op }
}

fn decode_op(raw: u32) -> Op {
    // BX: 0001 0010 1111 1111 1111 0001 xxxx
    if (raw & 0x0FFF_FFF0) == 0x012F_FF10 {
        return Op::Bx {
            rm: (raw & 0xF) as u8,
        };
    }

    // SWP / SWPB: 0001 0 x 00 Rn Rd 0000 1001 Rm
    if (raw & 0x0FB0_0FF0) == 0x0100_0090 {
        return Op::Swap {
            byte: (raw & (1 << 22)) != 0,
            rn: ((raw >> 16) & 0xF) as u8,
            rd: ((raw >> 12) & 0xF) as u8,
            rm: (raw & 0xF) as u8,
        };
    }

    // Multiply: 0000 00 A S Rd Rn Rs 1001 Rm
    if (raw & 0x0FC0_00F0) == 0x0000_0090 {
        return Op::Multiply {
            a: (raw & (1 << 21)) != 0,
            s: (raw & (1 << 20)) != 0,
            rd: ((raw >> 16) & 0xF) as u8,
            rn: ((raw >> 12) & 0xF) as u8,
            rs: ((raw >> 8) & 0xF) as u8,
            rm: (raw & 0xF) as u8,
        };
    }

    // Multiply long: 0000 1 U A S RdHi RdLo Rs 1001 Rm
    if (raw & 0x0F80_00F0) == 0x0080_0090 {
        return Op::MultiplyLong {
            signed: (raw & (1 << 22)) != 0,
            a: (raw & (1 << 21)) != 0,
            s: (raw & (1 << 20)) != 0,
            rd_hi: ((raw >> 16) & 0xF) as u8,
            rd_lo: ((raw >> 12) & 0xF) as u8,
            rs: ((raw >> 8) & 0xF) as u8,
            rm: (raw & 0xF) as u8,
        };
    }

    // Halfword / signed transfers: bit27-25=000, bit7=1, bit4=1, and not multiply
    // Encoding: 000 P U I W L Rn Rd OffsetH 1 S H 1 OffsetL  (imm)
    // or register form with I=0
    if (raw & 0x0E00_0090) == 0x0000_0090 && (raw & (1 << 22) != 0 || (raw & 0xF00) == 0) {
        // Exclude already-handled multiply/swap (they have specific patterns).
        // Halfword: bits 7:4 = 1SH1 with SH != 00 for useful ops; SH=00 is SWP-ish already matched.
        let sh = ((raw >> 5) & 3) as u8;
        if sh != 0 {
            let imm = (raw & (1 << 22)) != 0;
            let offset = if imm {
                ((((raw >> 8) & 0xF) << 4) | (raw & 0xF)) as u16
            } else {
                (raw & 0xF) as u16 // Rm in low nibble; high unused
            };
            return Op::HalfwordTransfer {
                load: (raw & (1 << 20)) != 0,
                writeback: (raw & (1 << 21)) != 0,
                imm,
                up: (raw & (1 << 23)) != 0,
                pre: (raw & (1 << 24)) != 0,
                s_bit: (raw & (1 << 6)) != 0,
                h_bit: (raw & (1 << 5)) != 0,
                rn: ((raw >> 16) & 0xF) as u8,
                rd: ((raw >> 12) & 0xF) as u8,
                offset,
            };
        }
    }

    // MRS: 0001 0 R 00 1111 Rd 0000 0000 0000
    if (raw & 0x0FBF_0FFF) == 0x010F_0000 {
        return Op::Mrs {
            spsr: (raw & (1 << 22)) != 0,
            rd: ((raw >> 12) & 0xF) as u8,
        };
    }

    // MSR register: 0001 0 R 10 field_mask 1111 0000 0000 Rm
    if (raw & 0x0FB0_FFF0) == 0x0120_F000 {
        return Op::Msr {
            spsr: (raw & (1 << 22)) != 0,
            fields: ((raw >> 16) & 0xF) as u8,
            src: MsrSrc::Reg {
                rm: (raw & 0xF) as u8,
            },
        };
    }

    // MSR immediate: 0011 0 R 10 field_mask 1111 rot imm8
    if (raw & 0x0FB0_F000) == 0x0320_F000 {
        return Op::Msr {
            spsr: (raw & (1 << 22)) != 0,
            fields: ((raw >> 16) & 0xF) as u8,
            src: MsrSrc::Imm {
                imm8: (raw & 0xFF) as u8,
                rot: ((raw >> 8) & 0xF) as u8,
            },
        };
    }

    let top = (raw >> 25) & 0x7;
    match top {
        // Data processing (incl. imm) — bits 27-26 = 00
        0 | 1 => {
            // bit4=1 and bit7=1 already handled for mul/extra loads; remaining with
            // bit25=0 and bit4=1 is reg-reg shift data processing.
            let i = (raw & (1 << 25)) != 0;
            let opcode = ((raw >> 21) & 0xF) as u8;
            let s = (raw & (1 << 20)) != 0;
            let rn = ((raw >> 16) & 0xF) as u8;
            let rd = ((raw >> 12) & 0xF) as u8;
            let op2 = if i {
                Op2::Imm {
                    imm8: (raw & 0xFF) as u8,
                    rot: ((raw >> 8) & 0xF) as u8,
                }
            } else if (raw & (1 << 4)) != 0 {
                // Must not be the multiply/extra load patterns (bit7=1).
                if (raw & (1 << 7)) != 0 {
                    return Op::Undefined;
                }
                Op2::RegRegShift {
                    rm: (raw & 0xF) as u8,
                    ty: ShiftType::from_bits((raw >> 5) & 3),
                    rs: ((raw >> 8) & 0xF) as u8,
                }
            } else {
                Op2::RegImmShift {
                    rm: (raw & 0xF) as u8,
                    ty: ShiftType::from_bits((raw >> 5) & 3),
                    imm: ((raw >> 7) & 0x1F) as u8,
                }
            };
            Op::DataProcessing {
                opcode,
                s,
                rn,
                rd,
                op2,
            }
        }
        // Single data transfer
        2 | 3 => {
            let i = (raw & (1 << 25)) != 0;
            // When I=1 and bit4=1 → undefined (not a valid shifted-reg LDR/STR on ARMv4)
            if i && (raw & (1 << 4)) != 0 {
                return Op::Undefined;
            }
            let offset = if i {
                SingleOffset::Reg {
                    rm: (raw & 0xF) as u8,
                    ty: ShiftType::from_bits((raw >> 5) & 3),
                    imm: ((raw >> 7) & 0x1F) as u8,
                }
            } else {
                SingleOffset::Imm((raw & 0xFFF) as u16)
            };
            Op::SingleTransfer {
                load: (raw & (1 << 20)) != 0,
                writeback: (raw & (1 << 21)) != 0,
                byte: (raw & (1 << 22)) != 0,
                up: (raw & (1 << 23)) != 0,
                pre: (raw & (1 << 24)) != 0,
                rn: ((raw >> 16) & 0xF) as u8,
                rd: ((raw >> 12) & 0xF) as u8,
                offset,
            }
        }
        // Block transfer
        4 => Op::BlockTransfer {
            load: (raw & (1 << 20)) != 0,
            writeback: (raw & (1 << 21)) != 0,
            s_bit: (raw & (1 << 22)) != 0,
            up: (raw & (1 << 23)) != 0,
            pre: (raw & (1 << 24)) != 0,
            rn: ((raw >> 16) & 0xF) as u8,
            rlist: (raw & 0xFFFF) as u16,
        },
        // Branch
        5 => {
            let mut off = (raw & 0x00FF_FFFF) as i32;
            // sign-extend 24→32, then <<2
            if off & 0x0080_0000 != 0 {
                off |= !0x00FF_FFFF;
            }
            Op::Branch {
                link: (raw & (1 << 24)) != 0,
                offset: off << 2,
            }
        }
        // Coprocessor — Undefined on GBA (no CP15)
        6 | 7 => {
            // SWI: 1111 ....
            if (raw >> 24) & 0xF == 0xF {
                Op::SoftwareInterrupt {
                    comment: raw & 0x00FF_FFFF,
                }
            } else {
                Op::Undefined
            }
        }
        _ => Op::Undefined,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_mov_imm() {
        // MOV r0, #1  => E3A00001
        let d = decode(0xE3A0_0001);
        assert_eq!(d.cond, 0xE);
        match d.op {
            Op::DataProcessing {
                opcode: 0xD,
                s: false,
                rd: 0,
                op2: Op2::Imm { imm8: 1, rot: 0 },
                ..
            } => {}
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn decode_b() {
        // B .+8 encoded as offset 0 relative to PC+8 → imm24=0
        let d = decode(0xEA00_0000);
        match d.op {
            Op::Branch {
                link: false,
                offset: 0,
            } => {}
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn decode_bx() {
        let d = decode(0xE12F_FF1E); // BX LR
        match d.op {
            Op::Bx { rm: 14 } => {}
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn decode_swi() {
        let d = decode(0xEF00_0006);
        match d.op {
            Op::SoftwareInterrupt { comment: 6 } => {}
            other => panic!("unexpected {other:?}"),
        }
    }
}
