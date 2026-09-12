//! Thumb (ARMv4T) decode / execute — graycart-gba P1.
//!
//! Cited: GBATEK — ARM CPU Overview / instruction summaries
//!   https://problemkaputt.de/gbatek.htm
//! Cited: ARM Architecture Reference Manual (DDI0100) — Thumb instruction set
//! Cited: ARM7TDMI Technical Reference Manual (DDI0210C) — Thumb / pipeline
//! Note: encodings and PC-skew rules for GBA ARMv4T; not a full reprint.
//!
//! Owned exclusively on `dev/p1-cpu` under `src/cpu/thumb/**`. Does not touch
//! `src/cpu/arm/**`. Register banks / CPSR live behind [`ThumbCore`]; bus behind
//! [`ThumbMem`]. Pipeline / exception wiring is owned by sibling P1 streams.

pub mod alu;
pub mod decode;
pub mod execute;

#[cfg(test)]
mod tests;

pub use decode::{decode, Cond, Instr, ShiftKind};
pub use execute::{execute, ExecResult};

/// Architectural register + CPSR surface the Thumb executor needs.
///
/// PC (`R15`) reads must already reflect Thumb pipeline skew (**executing + 4**).
/// Writers clear bit 0 of PC (halfword align); BX/interworking uses
/// [`ThumbCore::set_thumb_state`].
pub trait ThumbCore {
    fn reg(&self, n: u8) -> u32;
    fn set_reg(&mut self, n: u8, val: u32);

    fn n(&self) -> bool;
    fn z(&self) -> bool;
    fn c(&self) -> bool;
    fn v(&self) -> bool;
    fn set_nzcv(&mut self, n: bool, z: bool, c: bool, v: bool);

    /// CPSR.T — `true` while executing Thumb.
    fn thumb_state(&self) -> bool;
    fn set_thumb_state(&mut self, thumb: bool);

    /// Raise SWI; mode / SPSR / vector owned by exception stream.
    fn raise_swi(&mut self, imm8: u8);

    /// Raise Undefined (unused encodings that trap).
    fn raise_undef(&mut self);
}

/// Byte / half / word memory the Thumb executor needs (alignment quirks applied here).
pub trait ThumbMem {
    fn read8(&mut self, addr: u32) -> u8;
    fn write8(&mut self, addr: u32, val: u8);
    fn read16(&mut self, addr: u32) -> u16;
    fn write16(&mut self, addr: u32, val: u16);
    fn read32(&mut self, addr: u32) -> u32;
    fn write32(&mut self, addr: u32, val: u32);
}

/// Combined view for [`execute`].
pub trait ThumbCtx: ThumbCore + ThumbMem {}

impl<T: ThumbCore + ThumbMem> ThumbCtx for T {}
