//! VBlank / HBlank DMA start hooks (P5).
//!
//! Cited: GBATEK -- GBA DMA Transfers / LCD Status
//!   https://problemkaputt.de/gbatek-gba-dma-transfers.htm
//! Research: Project store `docs/graycart-gba/02-memory-bus-dma.md` §8
//! Note: HBlank Interval Free (DISPCNT bit5) required for OAM dest during HBlank.

use crate::bus::CpuMem;
use crate::irq::Irq;

use super::channel::{ChannelId, StartTiming};
use super::{Dma, DmaRunReport};

/// OAM region base (`07000000`); HBlank DMA here needs Interval Free.
pub const OAM_REGION_BASE: u32 = 0x0700_0000;

#[inline]
pub fn dad_in_oam(addr: u32) -> bool {
    (addr >> 24) == 0x07
}

impl Dma {
    /// Fire all enabled VBlank-timed channels (priority 0→3).
    pub fn on_vblank<M: CpuMem>(&mut self, mem: &mut M, irq: &mut Irq) -> DmaRunReport {
        for id in ChannelId::ALL {
            self.channel_mut(id).request_start(StartTiming::VBlank);
        }
        self.run_pending(mem, irq)
    }

    /// Fire HBlank-timed channels. When `hblank_interval_free` is false, channels
    /// whose latched DAD is in OAM are skipped for this edge (Enable kept).
    pub fn on_hblank<M: CpuMem>(
        &mut self,
        mem: &mut M,
        irq: &mut Irq,
        hblank_interval_free: bool,
    ) -> DmaRunReport {
        for id in ChannelId::ALL {
            let ch = self.channel_mut(id);
            if ch.enabled()
                && ch.start_timing() == StartTiming::HBlank
                && !hblank_interval_free
                && dad_in_oam(ch.latched_dad)
            {
                continue;
            }
            ch.request_start(StartTiming::HBlank);
        }
        self.run_pending(mem, irq)
    }
}
