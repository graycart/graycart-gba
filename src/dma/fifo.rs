//! DMA1/2 Special = Sound FIFO refill (P5).
//!
//! Cited: GBATEK -- GBA DMA Transfers / Sound FIFO
//!   https://problemkaputt.de/gbatek-gba-dma-transfers.htm
//! Research: Project store `docs/graycart-gba/02-memory-bus-dma.md` §8.3
//! Note: each request copies 4×32-bit; count + size bit ignored; dest fixed.

use crate::bus::CpuMem;
use crate::irq::Irq;

use super::channel::{ChannelId, StartTiming, CONTROL_ENABLE};
use super::{Dma, DmaRunReport};

/// FIFO A destination (`040000A0`).
pub const FIFO_A: u32 = 0x0400_00A0;
/// FIFO B destination (`040000A4`).
pub const FIFO_B: u32 = 0x0400_00A4;

impl Dma {
    /// Service a FIFO half-empty request on DMA1 and/or DMA2 (Special start).
    ///
    /// `which`: bit0 → try DMA1, bit1 → try DMA2 (both may fire in priority order).
    pub fn on_fifo_request<M: CpuMem>(
        &mut self,
        mem: &mut M,
        irq: &mut Irq,
        which: u8,
    ) -> DmaRunReport {
        let mut report = DmaRunReport::default();
        let candidates = [
            (which & 1 != 0, ChannelId::Ch1),
            (which & 2 != 0, ChannelId::Ch2),
        ];
        for (want, id) in candidates {
            if !want {
                continue;
            }
            {
                let ch = self.channel_mut(id);
                if !ch.enabled() || ch.start_timing() != StartTiming::Special {
                    continue;
                }
                if (ch.control & CONTROL_ENABLE) == 0 {
                    continue;
                }
                // Keep latched SAD walking across requests; DAD stays at FIFO (fixed).
                ch.arm_fifo_burst();
            }
            let part = self.run_one_pending(id, mem, irq);
            report.units_transferred += part.units_transferred;
            report.completed.extend(part.completed);
            report.rejected.extend(part.rejected);
            report.irq_raised |= part.irq_raised;
        }
        report
    }
}
