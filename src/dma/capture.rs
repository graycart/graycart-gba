//! DMA3 Special = Video Capture stub (P5).
//!
//! Cited: GBATEK -- GBA DMA Transfers (Video Capture)
//!   https://problemkaputt.de/gbatek-gba-dma-transfers.htm
//! Research: Project store `docs/graycart-gba/02-memory-bus-dma.md` §8.4
//! Note: functional window VCOUNT 2..=161 inclusive; stop at 162. Timing stretch.

use crate::bus::CpuMem;
use crate::irq::Irq;

use super::channel::{ChannelId, StartTiming};
use super::{Dma, DmaRunReport};

/// First VCOUNT line where capture DMA may run (inclusive).
pub const CAPTURE_VCOUNT_FIRST: u16 = 2;
/// Last VCOUNT line where capture DMA may run (inclusive).
pub const CAPTURE_VCOUNT_LAST: u16 = 161;

#[inline]
pub fn capture_window(vcount: u16) -> bool {
    (CAPTURE_VCOUNT_FIRST..=CAPTURE_VCOUNT_LAST).contains(&vcount)
}

impl Dma {
    /// On HBlank during the capture window, fire DMA3 if Start=Special.
    ///
    /// Outside the window (including VCOUNT ≥ 162), no-op. Auto-clear delay TBD.
    pub fn on_capture_hblank<M: CpuMem>(
        &mut self,
        mem: &mut M,
        irq: &mut Irq,
        vcount: u16,
    ) -> DmaRunReport {
        if !capture_window(vcount) {
            return DmaRunReport::default();
        }
        let ch = self.channel_mut(ChannelId::Ch3);
        if ch.enabled() && ch.start_timing() == StartTiming::Special {
            let _ = ch.request_start(StartTiming::Special);
        }
        self.run_one_pending(ChannelId::Ch3, mem, irq)
    }
}
