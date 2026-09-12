//! Thumb ALU helpers (flags + barrel shifter).
//!
//! Cited: GBATEK — ARM CPU Flags / THUMB ALU
//!   https://problemkaputt.de/gbatek.htm
//! Cited: ARM DDI0100 — Data-processing / shifter operands
//! Note: carry-out from shifts feeds CPSR.C for logicals; arithmetics use ALU C/V.

use super::decode::ShiftKind;

#[inline]
pub fn set_nz(result: u32) -> (bool, bool) {
    ((result as i32) < 0, result == 0)
}

/// Logical left: carry = last bit shifted out; imm=0 → result=Rm, C unchanged (caller).
pub fn lsl(val: u32, amount: u32, old_c: bool) -> (u32, bool) {
    if amount == 0 {
        (val, old_c)
    } else if amount < 32 {
        (val << amount, ((val >> (32 - amount)) & 1) != 0)
    } else if amount == 32 {
        (0, (val & 1) != 0)
    } else {
        (0, false)
    }
}

pub fn lsr(val: u32, amount: u32, old_c: bool) -> (u32, bool) {
    if amount == 0 {
        // Encoding imm5=0 means LSR #32 for Format 1; caller passes 32.
        (val, old_c)
    } else if amount < 32 {
        (val >> amount, ((val >> (amount - 1)) & 1) != 0)
    } else if amount == 32 {
        (0, (val as i32) < 0)
    } else {
        (0, false)
    }
}

pub fn asr(val: u32, amount: u32, old_c: bool) -> (u32, bool) {
    if amount == 0 {
        (val, old_c)
    } else if amount < 32 {
        let r = ((val as i32) >> amount) as u32;
        let c = ((val >> (amount - 1)) & 1) != 0;
        (r, c)
    } else {
        let c = (val as i32) < 0;
        let r = if c { 0xFFFF_FFFF } else { 0 };
        (r, c)
    }
}

pub fn ror(val: u32, amount: u32, old_c: bool) -> (u32, bool) {
    if amount == 0 {
        (val, old_c)
    } else {
        let a = amount & 31;
        if a == 0 {
            // ROR by a multiple of 32: result unchanged; C = bit 31.
            (val, (val as i32) < 0)
        } else {
            let r = val.rotate_right(a);
            (r, (r as i32) < 0)
        }
    }
}

pub fn shift(kind: ShiftKind, val: u32, amount: u32, old_c: bool) -> (u32, bool) {
    match kind {
        ShiftKind::Lsl => lsl(val, amount, old_c),
        ShiftKind::Lsr => lsr(val, amount, old_c),
        ShiftKind::Asr => asr(val, amount, old_c),
        ShiftKind::Ror => ror(val, amount, old_c),
    }
}

pub fn add_with_carry(a: u32, b: u32, carry_in: bool) -> (u32, bool, bool) {
    let (s1, c1) = a.overflowing_add(b);
    let (result, c2) = s1.overflowing_add(u32::from(carry_in));
    let carry = c1 | c2;
    let overflow = (!(a ^ b) & (a ^ result)) & 0x8000_0000 != 0;
    (result, carry, overflow)
}

pub fn sub_with_carry(a: u32, b: u32, carry_in: bool) -> (u32, bool, bool) {
    // ARM SBC/SUB: carry_in true means no borrow.
    add_with_carry(a, !b, carry_in)
}

pub fn add_flags(a: u32, b: u32) -> (u32, bool, bool, bool, bool) {
    let (r, c, v) = add_with_carry(a, b, false);
    let (n, z) = set_nz(r);
    (r, n, z, c, v)
}

pub fn sub_flags(a: u32, b: u32) -> (u32, bool, bool, bool, bool) {
    let (r, c, v) = sub_with_carry(a, b, true);
    let (n, z) = set_nz(r);
    (r, n, z, c, v)
}
