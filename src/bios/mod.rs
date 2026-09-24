//! BIOS high-level emulation. No Nintendo BIOS image is committed.
//!
//! Cited: GBATEK BIOS Functions.
//! <https://problemkaputt.de/gbatek.htm>

mod cpuset;
mod hle;

pub use cpuset::{cpu_fast_set, cpu_set};
