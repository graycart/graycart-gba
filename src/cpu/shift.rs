//! Barrel shifter. ARM DDI 0210C.
//!
//! Cited: ARM Architecture Reference Manual DDI 0100, barrel shifter.

pub fn rrx(value: u32, carry: bool) -> (u32, bool) {
    let out = ((carry as u32) << 31) | (value >> 1);
    (out, value & 1 != 0)
}

/// `kind`: 0 LSL, 1 LSR, 2 ASR, 3 ROR. Amount 0 means no shift.
/// Immediate LSR/ASR #32 and RRX are handled by the caller.
pub fn barrel(value: u32, kind: u32, amount: u32, carry: bool) -> (u32, bool) {
    match kind {
        0 => lsl(value, amount, carry),
        1 => lsr(value, amount, carry),
        2 => asr(value, amount, carry),
        _ => ror(value, amount, carry),
    }
}

fn lsl(value: u32, amount: u32, carry: bool) -> (u32, bool) {
    if amount == 0 {
        (value, carry)
    } else if amount < 32 {
        (value << amount, value & (1 << (32 - amount)) != 0)
    } else if amount == 32 {
        (0, value & 1 != 0)
    } else {
        (0, false)
    }
}

fn lsr(value: u32, amount: u32, carry: bool) -> (u32, bool) {
    if amount == 0 {
        (value, carry)
    } else if amount < 32 {
        (value >> amount, value & (1 << (amount - 1)) != 0)
    } else if amount == 32 {
        (0, value & 0x8000_0000 != 0)
    } else {
        (0, false)
    }
}

fn asr(value: u32, amount: u32, carry: bool) -> (u32, bool) {
    if amount == 0 {
        (value, carry)
    } else if amount < 32 {
        let out = ((value as i32) >> amount) as u32;
        (out, value & (1 << (amount - 1)) != 0)
    } else {
        let sign = value & 0x8000_0000 != 0;
        (if sign { 0xFFFF_FFFF } else { 0 }, sign)
    }
}

fn ror(value: u32, amount: u32, carry: bool) -> (u32, bool) {
    if amount == 0 {
        (value, carry)
    } else {
        let amount = amount & 31;
        if amount == 0 {
            (value, value & 0x8000_0000 != 0)
        } else {
            (value.rotate_right(amount), value & (1 << (amount - 1)) != 0)
        }
    }
}

pub fn add_carry(a: u32, b: u32, carry: bool) -> (u32, bool, bool) {
    let (sum, c1) = a.overflowing_add(b);
    let (sum, c2) = sum.overflowing_add(u32::from(carry));
    let carry_out = c1 || c2;
    let overflow = (a ^ sum) & (b ^ sum) & 0x8000_0000 != 0;
    (sum, carry_out, overflow)
}
