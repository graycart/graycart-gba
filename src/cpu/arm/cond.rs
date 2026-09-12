//! ARM condition-field evaluation.
//!
//! Cited: GBATEK -- ARM CPU Flags / condition codes
//!   https://problemkaputt.de/gbatek.htm
//! Cited: ARM DDI0210C §2.2 -- condition field
//! Note: `Regs::cond_passed` is the Cpu-facing API; this pure helper is for
//! decode-adjacent tests without a full register file.

use crate::cpu::cpsr;

/// Returns whether `cond` (bits 31–28 of the opcode) passes given `cpsr_val`.
///
/// ARMv4: `0xF` (NV) never executes.
#[inline]
pub fn cond_passes(cond: u8, cpsr_val: u32) -> bool {
    let n = cpsr_val & cpsr::N != 0;
    let z = cpsr_val & cpsr::Z != 0;
    let c = cpsr_val & cpsr::C != 0;
    let v = cpsr_val & cpsr::V != 0;
    match cond & 0xF {
        0x0 => z,            // EQ
        0x1 => !z,           // NE
        0x2 => c,            // CS/HS
        0x3 => !c,           // CC/LO
        0x4 => n,            // MI
        0x5 => !n,           // PL
        0x6 => v,            // VS
        0x7 => !v,           // VC
        0x8 => c && !z,      // HI
        0x9 => !c || z,      // LS
        0xA => n == v,       // GE
        0xB => n != v,       // LT
        0xC => !z && n == v, // GT
        0xD => z || n != v,  // LE
        0xE => true,         // AL
        0xF => false,        // NV (ARMv4)
        _ => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eq_ne() {
        assert!(cond_passes(0x0, cpsr::Z));
        assert!(!cond_passes(0x0, 0));
        assert!(cond_passes(0x1, 0));
        assert!(!cond_passes(0x1, cpsr::Z));
    }

    #[test]
    fn al_nv() {
        assert!(cond_passes(0xE, 0));
        assert!(!cond_passes(0xF, 0xFFFF_FFFF));
    }

    #[test]
    fn ge_lt_uses_n_xor_v() {
        assert!(cond_passes(0xA, 0));
        assert!(cond_passes(0xA, cpsr::N | cpsr::V));
        assert!(cond_passes(0xB, cpsr::N));
        assert!(cond_passes(0xB, cpsr::V));
    }
}
