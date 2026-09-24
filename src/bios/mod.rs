//! BIOS high-level emulation. No Nintendo BIOS image is committed.
//!
//! Cited: GBATEK BIOS Functions.
//! <https://problemkaputt.de/gbatek.htm>

mod cpuset;
mod decomp;
mod hle;

pub use cpuset::{cpu_fast_set, cpu_set};
pub use decomp::{diff8_vram, diff8_wram, diff16, huff, lz77_vram, lz77_wram, rl_vram, rl_wram};
