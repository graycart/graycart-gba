//! Misc system regs placeholder (WAITCNT, POSTFLG, HALTCNT, …).
//!
//! Module layout from graycart-gba implementation plan §2.2.
//! Behavior: see research `docs/graycart-gba/05-io-timers-irq-input.md` (not implemented).

/// Stub system hardware regs.
#[derive(Debug, Default)]
pub struct Hw;
