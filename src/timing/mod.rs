//! Game Pak waitstates and the opcode prefetch buffer.
//!
//! Cited: GBATEK Game Pak Memory Waitstates.
//! <https://problemkaputt.de/gbatek.htm>

pub mod prefetch;
pub mod wait;

pub use prefetch::Prefetch;
pub use wait::{Width, internal_cycles, rom_cycles, sram_cycles, ws0_ns};
