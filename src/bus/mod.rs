//! Memory bus / region table placeholder (P2).
//!
//! Module layout from graycart-gba implementation plan §2.2.
//! Behavior: see research `docs/graycart-gba/02-memory-bus-dma.md` (not implemented).

/// Stub bus — sole memory authority once filled in.
#[derive(Debug, Default)]
pub struct Bus;
