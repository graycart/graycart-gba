//! ARM barrel shifter (immediate + register forms).
//!
//! Cited: GBATEK — ARM CPU Opcode Summary / shifts
//!   https://problemkaputt.de/gbatek.htm
//! Cited: ARM DDI0210C — addressing modes / shifter operands
//! Note: carry-out feeds CPSR.C for logical data-processing when S=1.

/// Shift type encoding (bits 6–5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ShiftType {
    Lsl = 0,
    Lsr = 1,
    Asr = 2,
    Ror = 3,
}

impl ShiftType {
    #[inline]
    pub fn from_bits(bits: u32) -> Self {
        match bits & 3 {
            0 => Self::Lsl,
            1 => Self::Lsr,
            2 => Self::Asr,
            _ => Self::Ror,
        }
    }
}

/// Shifter result: value + carry-out (for logicals / flag updates).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShiftOut {
    pub value: u32,
    pub carry: bool,
}

/// Rotate-right immediate operand2 (I=1): `imm8` ROR `(rot * 2)`.
#[inline]
pub fn shift_imm_ror(imm8: u8, rotate_imm: u8, carry_in: bool) -> ShiftOut {
    let rot = (rotate_imm as u32) * 2;
    if rot == 0 {
        ShiftOut {
            value: imm8 as u32,
            carry: carry_in,
        }
    } else {
        let value = (imm8 as u32).rotate_right(rot);
        ShiftOut {
            value,
            carry: value & (1 << 31) != 0,
        }
    }
}

/// Immediate shift amount form (bit4=0): amount in bits 11–7.
#[inline]
pub fn shift_reg_imm(rm: u32, ty: ShiftType, amount: u8, carry_in: bool) -> ShiftOut {
    let amount = amount as u32;
    match ty {
        ShiftType::Lsl => match amount {
            0 => ShiftOut {
                value: rm,
                carry: carry_in,
            },
            1..=31 => ShiftOut {
                value: rm << amount,
                carry: rm & (1 << (32 - amount)) != 0,
            },
            32 => ShiftOut {
                value: 0,
                carry: rm & 1 != 0,
            },
            _ => ShiftOut {
                value: 0,
                carry: false,
            },
        },
        ShiftType::Lsr => {
            // amount==0 encoding means LSR #32
            let amt = if amount == 0 { 32 } else { amount };
            match amt {
                1..=31 => ShiftOut {
                    value: rm >> amt,
                    carry: rm & (1 << (amt - 1)) != 0,
                },
                32 => ShiftOut {
                    value: 0,
                    carry: rm & (1 << 31) != 0,
                },
                _ => ShiftOut {
                    value: 0,
                    carry: false,
                },
            }
        }
        ShiftType::Asr => {
            let amt = if amount == 0 { 32 } else { amount };
            if amt < 32 {
                ShiftOut {
                    value: ((rm as i32) >> amt) as u32,
                    carry: rm & (1 << (amt - 1)) != 0,
                }
            } else {
                // ASR #32 or more → all sign bits
                let carry = rm & (1 << 31) != 0;
                ShiftOut {
                    value: if carry { 0xFFFF_FFFF } else { 0 },
                    carry,
                }
            }
        }
        ShiftType::Ror => {
            if amount == 0 {
                // RRX
                let carry = rm & 1 != 0;
                let value = (rm >> 1) | (if carry_in { 1 << 31 } else { 0 });
                ShiftOut { value, carry }
            } else {
                let value = rm.rotate_right(amount);
                ShiftOut {
                    value,
                    carry: rm & (1 << (amount - 1)) != 0,
                }
            }
        }
    }
}

/// Register-controlled shift (bit4=1): Rs low 8 bits; Rs==0 leaves Rm, carry=Cin.
#[inline]
pub fn shift_reg_reg(rm: u32, ty: ShiftType, rs_low: u8, carry_in: bool) -> ShiftOut {
    let amount = rs_low as u32;
    if amount == 0 {
        return ShiftOut {
            value: rm,
            carry: carry_in,
        };
    }
    match ty {
        ShiftType::Lsl => match amount {
            1..=31 => ShiftOut {
                value: rm << amount,
                carry: rm & (1 << (32 - amount)) != 0,
            },
            32 => ShiftOut {
                value: 0,
                carry: rm & 1 != 0,
            },
            _ => ShiftOut {
                value: 0,
                carry: false,
            },
        },
        ShiftType::Lsr => match amount {
            1..=31 => ShiftOut {
                value: rm >> amount,
                carry: rm & (1 << (amount - 1)) != 0,
            },
            32 => ShiftOut {
                value: 0,
                carry: rm & (1 << 31) != 0,
            },
            _ => ShiftOut {
                value: 0,
                carry: false,
            },
        },
        ShiftType::Asr => {
            if amount < 32 {
                ShiftOut {
                    value: ((rm as i32) >> amount) as u32,
                    carry: rm & (1 << (amount - 1)) != 0,
                }
            } else {
                let carry = rm & (1 << 31) != 0;
                ShiftOut {
                    value: if carry { 0xFFFF_FFFF } else { 0 },
                    carry,
                }
            }
        }
        ShiftType::Ror => {
            let amt = amount & 31;
            if amt == 0 {
                // ROR by multiple of 32: value unchanged, carry = bit31
                ShiftOut {
                    value: rm,
                    carry: rm & (1 << 31) != 0,
                }
            } else {
                ShiftOut {
                    value: rm.rotate_right(amt),
                    carry: rm & (1 << (amt - 1)) != 0,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lsl_imm() {
        let o = shift_reg_imm(0b1000, ShiftType::Lsl, 1, false);
        assert_eq!(o.value, 0b10000);
        assert!(!o.carry);
    }

    #[test]
    fn lsr_hash_32() {
        let o = shift_reg_imm(0x8000_0000, ShiftType::Lsr, 0, true);
        assert_eq!(o.value, 0);
        assert!(o.carry);
    }

    #[test]
    fn rrx() {
        let o = shift_reg_imm(0b10, ShiftType::Ror, 0, true);
        assert_eq!(o.value, 0x8000_0001);
        assert!(!o.carry);
    }

    #[test]
    fn imm_ror() {
        let o = shift_imm_ror(0xFF, 4, false); // ROR #8
        assert_eq!(o.value, 0xFF00_0000);
        assert!(o.carry);
    }
}
