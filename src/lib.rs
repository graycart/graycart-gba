//! Graycart Game Boy Advance core.
//!
//! The library is the machine. It does not open a window or an audio device.

pub mod apu;
pub mod bios;
pub mod bus;
pub mod cart;
pub mod compat;
pub mod cpu;
pub mod debug;
pub mod dma;
pub mod frontend;
pub mod hw;
pub mod input;
pub mod irq;
pub mod ppu;
pub mod timer;
pub mod timing;

pub use bus::Bus;
pub use cpu::{Cpu, StepError};
pub use debug::{
    cpu_result_line, format_trace_line, frame_hash, frame_nonzero, live_cpu_line, MachineDebug,
};
pub use hw::Machine;
