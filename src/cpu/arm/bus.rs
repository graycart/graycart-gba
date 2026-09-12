//! Re-export the shared CPU memory surface for ARM load/store.
//!
//! Cited: ARM DDI0210C — memory interface overview
//!   https://developer.arm.com/documentation/ddi0210/c/
//! Note: waitstates / open-bus live with the bus owner (P2). Prefer [`crate::bus::CpuMem`].

pub use crate::bus::CpuMem as ArmBus;
