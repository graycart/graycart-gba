//! ARM data-processing ALU + NZCV flag helpers.
//!
//! Cited: GBATEK — ARM CPU Opcode Summary (ALU)
//!   https://problemkaputt.de/gbatek.htm
//! Cited: ARM DDI0210C — data-processing instructions

use crate::cpu::cpsr;

/// ALU result with optional flag products.
#[derive(Debug, Clone, Copy)]
pub struct AluOut {
    pub result: u32,
    /// Whether this opcode writes Rd (TST/TEQ/CMP/CMN do not).
    pub write_rd: bool,
    pub n: bool,
    pub z: bool,
    pub c: bool,
    pub v: bool,
    /// If false, leave C (and for arith V is always set from ALU when S).
    pub update_c: bool,
    pub update_v: bool,
}

#[inline]
fn nz(result: u32) -> (bool, bool) {
    (result & (1 << 31) != 0, result == 0)
}

/// Logical ops: C from shifter; V unchanged.
#[inline]
fn logical(result: u32, shifter_c: bool) -> AluOut {
    let (n, z) = nz(result);
    AluOut {
        result,
        write_rd: true,
        n,
        z,
        c: shifter_c,
        v: false,
        update_c: true,
        update_v: false,
    }
}

#[inline]
fn logical_test(result: u32, shifter_c: bool) -> AluOut {
    let mut o = logical(result, shifter_c);
    o.write_rd = false;
    o
}

/// ADD-style: C = carry out, V = signed overflow.
#[inline]
pub fn add_with_flags(a: u32, b: u32, carry_in: bool) -> (u32, bool, bool) {
    let cin = u64::from(carry_in);
    let sum = u64::from(a) + u64::from(b) + cin;
    let result = sum as u32;
    let c = sum > u64::from(u32::MAX);
    let v = ((a ^ result) & (b ^ result) & 0x8000_0000) != 0;
    (result, c, v)
}

/// SUB-style (a - b - !carry_in for SBC with carry_in meaning NOT borrow).
#[inline]
pub fn sub_with_flags(a: u32, b: u32, carry_in: bool) -> (u32, bool, bool) {
    // ARM: C=1 means no borrow. For SUB, treat as ADC with ~b and carry=1.
    // SBC uses existing C as carry_in.
    add_with_flags(a, !b, carry_in)
}

/// Execute data-processing opcode `0..=15`.
pub fn data_process(opcode: u8, rn: u32, op2: u32, shifter_c: bool, cpsr_c: bool) -> AluOut {
    match opcode & 0xF {
        0x0 => logical(rn & op2, shifter_c), // AND
        0x1 => logical(rn ^ op2, shifter_c), // EOR
        0x2 => {
            // SUB
            let (r, c, v) = sub_with_flags(rn, op2, true);
            let (n, z) = nz(r);
            AluOut {
                result: r,
                write_rd: true,
                n,
                z,
                c,
                v,
                update_c: true,
                update_v: true,
            }
        }
        0x3 => {
            // RSB
            let (r, c, v) = sub_with_flags(op2, rn, true);
            let (n, z) = nz(r);
            AluOut {
                result: r,
                write_rd: true,
                n,
                z,
                c,
                v,
                update_c: true,
                update_v: true,
            }
        }
        0x4 => {
            // ADD
            let (r, c, v) = add_with_flags(rn, op2, false);
            let (n, z) = nz(r);
            AluOut {
                result: r,
                write_rd: true,
                n,
                z,
                c,
                v,
                update_c: true,
                update_v: true,
            }
        }
        0x5 => {
            // ADC
            let (r, c, v) = add_with_flags(rn, op2, cpsr_c);
            let (n, z) = nz(r);
            AluOut {
                result: r,
                write_rd: true,
                n,
                z,
                c,
                v,
                update_c: true,
                update_v: true,
            }
        }
        0x6 => {
            // SBC
            let (r, c, v) = sub_with_flags(rn, op2, cpsr_c);
            let (n, z) = nz(r);
            AluOut {
                result: r,
                write_rd: true,
                n,
                z,
                c,
                v,
                update_c: true,
                update_v: true,
            }
        }
        0x7 => {
            // RSC
            let (r, c, v) = sub_with_flags(op2, rn, cpsr_c);
            let (n, z) = nz(r);
            AluOut {
                result: r,
                write_rd: true,
                n,
                z,
                c,
                v,
                update_c: true,
                update_v: true,
            }
        }
        0x8 => logical_test(rn & op2, shifter_c), // TST
        0x9 => logical_test(rn ^ op2, shifter_c), // TEQ
        0xA => {
            // CMP
            let (r, c, v) = sub_with_flags(rn, op2, true);
            let (n, z) = nz(r);
            AluOut {
                result: r,
                write_rd: false,
                n,
                z,
                c,
                v,
                update_c: true,
                update_v: true,
            }
        }
        0xB => {
            // CMN
            let (r, c, v) = add_with_flags(rn, op2, false);
            let (n, z) = nz(r);
            AluOut {
                result: r,
                write_rd: false,
                n,
                z,
                c,
                v,
                update_c: true,
                update_v: true,
            }
        }
        0xC => logical(rn | op2, shifter_c),  // ORR
        0xD => logical(op2, shifter_c),       // MOV
        0xE => logical(rn & !op2, shifter_c), // BIC
        0xF => logical(!op2, shifter_c),      // MVN
        _ => unreachable!(),
    }
}

/// Merge NZCV into CPSR from an ALU result when S=1.
#[inline]
pub fn apply_flags(cpsr_val: u32, alu: &AluOut) -> u32 {
    let mut c = cpsr_val & !(cpsr::N | cpsr::Z | cpsr::C | cpsr::V);
    if alu.n {
        c |= cpsr::N;
    }
    if alu.z {
        c |= cpsr::Z;
    }
    if alu.update_c {
        if alu.c {
            c |= cpsr::C;
        }
    } else if cpsr_val & cpsr::C != 0 {
        c |= cpsr::C;
    }
    if alu.update_v {
        if alu.v {
            c |= cpsr::V;
        }
    } else if cpsr_val & cpsr::V != 0 {
        c |= cpsr::V;
    }
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_sets_carry() {
        let (r, c, v) = add_with_flags(0xFFFF_FFFF, 1, false);
        assert_eq!(r, 0);
        assert!(c);
        assert!(!v);
    }

    #[test]
    fn sub_borrow() {
        let (r, c, _v) = sub_with_flags(0, 1, true);
        assert_eq!(r, 0xFFFF_FFFF);
        assert!(!c); // borrow → C clear
    }

    #[test]
    fn mov_logical() {
        let o = data_process(0xD, 0, 0xA5, true, false);
        assert_eq!(o.result, 0xA5);
        assert!(o.write_rd);
        assert!(o.c);
    }
}
