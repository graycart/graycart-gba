//! ARM7TDMI CPU core (P1 bring-up).
//!
//! Cited: GBATEK -- ARM CPU Overview / Register Set / Flags
//!   https://problemkaputt.de/gbatek.htm
//! Cited: ARM DDI0210C (ARM7TDMI TRM r4p1) -- programmer's model
//! Note: this module currently owns registers, CPSR/SPSR, and modes only;
//! decode/execute, pipeline, and exceptions are sibling P1 streams.
//!
//! Research: Project store `docs/graycart-gba/01-cpu-arm7tdmi.md`.

mod mode;
mod regs;

pub use mode::Mode;
pub use regs::{cpsr, Regs};

/// ARM7TDMI core shell. ISA execute lands in sibling modules on `dev/p1-cpu`.
#[derive(Debug, Default)]
pub struct Cpu {
    pub regs: Regs,
}

impl Cpu {
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }
}
