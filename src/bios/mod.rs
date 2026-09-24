//! BIOS high-level emulation. No Nintendo BIOS image is committed.
//!
//! Cited: GBATEK BIOS Functions.
//! <https://problemkaputt.de/gbatek.htm>

mod cpuset;
mod decomp;
mod hle;
mod math;

pub use cpuset::{cpu_fast_set, cpu_set};
pub use decomp::{diff8_vram, diff8_wram, diff16, huff, lz77_vram, lz77_wram, rl_vram, rl_wram};
pub use math::{
    arctan, arctan2, bg_affine_set, bit_unpack, obj_affine_set, register_ram_reset, soft_reset,
    sound_bias, sqrt_u32, warn_swi,
};
