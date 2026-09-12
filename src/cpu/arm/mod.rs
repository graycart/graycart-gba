//! ARM-state decode and execute (ARMv4T subset for GBA).
//!
//! Cited: GBATEK -- ARM CPU Instruction Set / Opcode Summary / memory alignments
//!   https://problemkaputt.de/gbatek.htm
//! Cited: ARM DDI0210C (ARM7TDMI TRM r4p1) -- ARM instruction set
//!   https://developer.arm.com/documentation/ddi0210/c/
//! Note: Thumb decode is a sibling stream. Pipeline refill + `take_exception` are
//! invoked from execute results. Memory waits/open-bus are P2 (`CpuMem`).

pub mod alu;
pub mod bus;
pub mod cond;
pub mod core;
pub mod decode;
pub mod execute;
pub mod shifter;

pub use bus::ArmBus;
pub use decode::{decode, Decoded, Op};
pub use execute::{execute, step, ExecResult};

#[cfg(test)]
mod tests;
