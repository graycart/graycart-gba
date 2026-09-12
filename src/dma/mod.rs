//! DMA channels 0–3: full start modes (P5) + Immediate (P2).
//!
//! Cited: GBATEK -- GBA DMA Transfers
//!   https://problemkaputt.de/gbatek-gba-dma-transfers.htm
//! Cross-check: NanoBoyAdvance DMA notes (secondary) — Immediate ignores Repeat;
//!   address masks on write; SRAM DMA never.
//! Research: Project store `docs/graycart-gba/02-memory-bus-dma.md` §8.
//!
//! **P5 scope:** VBlank / HBlank (Interval Free for OAM) / FIFO Special (DMA1/2) /
//! Video Capture stub (DMA3) / completion IRQs / DMA3 Game Pak yes. SRAM reject
//! stays. Startup delay 2 cycles / mid-insn preempt are P8 stretch.

mod capture;
mod channel;
mod fifo;
mod starts;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_capture;
#[cfg(test)]
mod tests_fifo;
#[cfg(test)]
mod tests_starts;

pub use capture::{capture_window, CAPTURE_VCOUNT_FIRST, CAPTURE_VCOUNT_LAST};
pub use channel::{
    control_word, Channel, ChannelId, DestControl, SrcControl, StartTiming, CONTROL_ENABLE,
    CONTROL_IRQ, CONTROL_REPEAT, CONTROL_TRANSFER_TYPE,
};
pub use fifo::{FIFO_A, FIFO_B};
pub use starts::{dad_in_oam, OAM_REGION_BASE};

use crate::bus::CpuMem;
use crate::irq::Irq;
use channel::Channel as Chan;

/// Game Pak SRAM / Flash-save window (8-bit bus). DMA must never touch this.
/// Includes the mirrored field `0E000000–0FFFFFFF`.
pub const SRAM_WINDOW_START: u32 = 0x0E00_0000;
pub const SRAM_WINDOW_END: u32 = 0x0FFF_FFFF;

/// Why a DMA was refused before touching the bus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DmaReject {
    /// SAD or DAD lands in the SRAM / Flash-save window.
    SramWindow { channel: ChannelId, addr: u32 },
}

/// Outcome of draining DMA work.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DmaRunReport {
    /// Units (halfwords or words) successfully copied this call.
    pub units_transferred: u32,
    /// Channels that finished a burst this call.
    pub completed: Vec<ChannelId>,
    /// Channels that were armed but rejected (e.g. SRAM).
    pub rejected: Vec<DmaReject>,
    /// OR of IRQ_DMAx bits raised this call.
    pub irq_raised: u16,
}

/// Four-channel DMA engine (priority 0 > 1 > 2 > 3).
#[derive(Debug, Clone)]
pub struct Dma {
    channels: [Chan; 4],
}

impl Default for Dma {
    fn default() -> Self {
        Self::new()
    }
}

impl Dma {
    pub fn new() -> Self {
        Self {
            channels: [
                Chan::new(ChannelId::Ch0),
                Chan::new(ChannelId::Ch1),
                Chan::new(ChannelId::Ch2),
                Chan::new(ChannelId::Ch3),
            ],
        }
    }

    #[inline]
    pub fn channel(&self, id: ChannelId) -> &Chan {
        &self.channels[id.index()]
    }

    #[inline]
    pub fn channel_mut(&mut self, id: ChannelId) -> &mut Chan {
        &mut self.channels[id.index()]
    }

    /// True while any channel holds an active transfer.
    ///
    /// The CPU must not execute guest instructions while this is set
    /// (GBATEK: CPU halted during active DMA units).
    #[inline]
    pub fn cpu_halted(&self) -> bool {
        self.channels.iter().any(|c| c.active)
    }

    /// True if any channel is active or has a pending armed burst.
    #[inline]
    pub fn is_busy(&self) -> bool {
        self.channels
            .iter()
            .any(|c| c.active || c.pending_immediate)
    }

    pub fn write_sad(&mut self, id: ChannelId, value: u32) {
        self.channel_mut(id).write_sad(value);
    }

    pub fn write_dad(&mut self, id: ChannelId, value: u32) {
        self.channel_mut(id).write_dad(value);
    }

    pub fn write_count(&mut self, id: ChannelId, value: u16) {
        self.channel_mut(id).write_count(value);
    }

    /// Write DMAxCNT_H. Rising Enable + Immediate arms a pending transfer.
    pub fn write_control(&mut self, id: ChannelId, value: u16) {
        self.channel_mut(id).write_control(value);
    }

    pub fn read_control(&self, id: ChannelId) -> u16 {
        self.channel(id).control
    }

    /// MMIO helpers: map `040000B0`–`040000DF` writes (byte offset within DMA block).
    ///
    /// `offset` is relative to `0x0400_00B0` (0..=0x2F). Unknown offsets are ignored.
    pub fn write_mmio32(&mut self, offset: u32, value: u32) {
        let Some((id, kind)) = decode_mmio_word(offset) else {
            return;
        };
        match kind {
            MmioWord::Sad => self.write_sad(id, value),
            MmioWord::Dad => self.write_dad(id, value),
            MmioWord::Cnt => {
                self.write_count(id, value as u16);
                self.write_control(id, (value >> 16) as u16);
            }
        }
    }

    pub fn write_mmio16(&mut self, offset: u32, value: u16) {
        let Some((id, kind)) = decode_mmio_half(offset) else {
            return;
        };
        match kind {
            MmioHalf::CntL => self.write_count(id, value),
            MmioHalf::CntH => self.write_control(id, value),
            MmioHalf::SadLo => {
                let ch = self.channel_mut(id);
                let hi = ch.sad & 0xFFFF_0000;
                ch.write_sad(hi | u32::from(value));
            }
            MmioHalf::SadHi => {
                let ch = self.channel_mut(id);
                let lo = ch.sad & 0x0000_FFFF;
                ch.write_sad((u32::from(value) << 16) | lo);
            }
            MmioHalf::DadLo => {
                let ch = self.channel_mut(id);
                let hi = ch.dad & 0xFFFF_0000;
                ch.write_dad(hi | u32::from(value));
            }
            MmioHalf::DadHi => {
                let ch = self.channel_mut(id);
                let lo = ch.dad & 0x0000_FFFF;
                ch.write_dad((u32::from(value) << 16) | lo);
            }
        }
    }

    pub fn read_mmio16(&self, offset: u32) -> u16 {
        // Only CNT_H is readable on hardware; others return open-bus (TBD → 0).
        match decode_mmio_half(offset) {
            Some((id, MmioHalf::CntH)) => self.read_control(id),
            _ => 0,
        }
    }

    /// Drain all pending Immediate transfers in priority order (0→3).
    ///
    /// Convenience for P2 call sites; raises IRQs when CNT_H.IRQ is set.
    pub fn run_immediate<M: CpuMem>(&mut self, mem: &mut M) -> DmaRunReport {
        let mut irq = Irq::new();
        self.run_pending(mem, &mut irq)
    }

    /// Drain all pending bursts in priority order (0→3), raising DMA IRQs.
    pub fn run_pending<M: CpuMem>(&mut self, mem: &mut M, irq: &mut Irq) -> DmaRunReport {
        let mut report = DmaRunReport::default();
        while let Some(idx) = self.next_pending_index() {
            let id = ChannelId::try_from(idx).expect("index 0..=3");
            let part = self.run_one_pending(id, mem, irq);
            report.units_transferred += part.units_transferred;
            report.completed.extend(part.completed);
            report.rejected.extend(part.rejected);
            report.irq_raised |= part.irq_raised;
        }
        debug_assert!(!self.cpu_halted());
        report
    }

    /// Run a single channel if it is pending (used by FIFO / capture).
    pub fn run_one_pending<M: CpuMem>(
        &mut self,
        id: ChannelId,
        mem: &mut M,
        irq: &mut Irq,
    ) -> DmaRunReport {
        let mut report = DmaRunReport::default();
        let idx = id.index();
        if !self.channels[idx].pending_immediate && !self.channels[idx].active {
            return report;
        }

        let (sad, dad) = {
            let ch = &self.channels[idx];
            (ch.latched_sad, ch.latched_dad)
        };
        if let Some(bad) = sram_touch_addr(sad).or_else(|| sram_touch_addr(dad)) {
            self.finish_channel(idx, irq, &mut report);
            report.rejected.push(DmaReject::SramWindow {
                channel: id,
                addr: bad,
            });
            return report;
        }

        self.channels[idx].begin_active();
        debug_assert!(self.cpu_halted());

        let mut units = 0u32;
        let mut aborted = false;
        while self.channels[idx].remaining > 0 {
            let (sad, dad, word32) = {
                let ch = &self.channels[idx];
                (ch.latched_sad, ch.latched_dad, ch.transfer32())
            };
            if let Some(bad) = sram_touch_addr(sad).or_else(|| sram_touch_addr(dad)) {
                self.finish_channel(idx, irq, &mut report);
                report.rejected.push(DmaReject::SramWindow {
                    channel: id,
                    addr: bad,
                });
                aborted = true;
                break;
            }

            if word32 {
                let v = mem.read32(sad & !3);
                mem.write32(dad & !3, v);
            } else {
                let v = mem.read16(sad & !1);
                mem.write16(dad & !1, v);
            }

            let ch = &mut self.channels[idx];
            ch.step_addrs();
            ch.remaining -= 1;
            units += 1;
        }

        if !aborted {
            self.finish_channel(idx, irq, &mut report);
            report.completed.push(id);
        }
        report.units_transferred += units;
        report
    }

    fn finish_channel(&mut self, idx: usize, irq: &mut Irq, report: &mut DmaRunReport) {
        let id = ChannelId::try_from(idx).expect("index");
        if self.channels[idx].finish() {
            let bit = id.irq_bit();
            irq.raise(bit);
            report.irq_raised |= bit;
        }
    }

    fn next_pending_index(&self) -> Option<usize> {
        self.channels
            .iter()
            .position(|c| c.pending_immediate || c.active)
    }
}

#[inline]
fn sram_touch_addr(addr: u32) -> Option<u32> {
    if (SRAM_WINDOW_START..=SRAM_WINDOW_END).contains(&addr) {
        Some(addr)
    } else {
        None
    }
}

#[derive(Clone, Copy)]
enum MmioWord {
    Sad,
    Dad,
    Cnt,
}

#[derive(Clone, Copy)]
enum MmioHalf {
    SadLo,
    SadHi,
    DadLo,
    DadHi,
    CntL,
    CntH,
}

fn decode_mmio_word(offset: u32) -> Option<(ChannelId, MmioWord)> {
    // Each channel occupies 12 bytes: SAD, DAD, CNT_L|CNT_H.
    if offset >= 0x30 || (offset & 3) != 0 {
        return None;
    }
    let ch = (offset / 12) as usize;
    let id = ChannelId::try_from(ch).ok()?;
    let within = offset % 12;
    let kind = match within {
        0 => MmioWord::Sad,
        4 => MmioWord::Dad,
        8 => MmioWord::Cnt,
        _ => return None,
    };
    Some((id, kind))
}

fn decode_mmio_half(offset: u32) -> Option<(ChannelId, MmioHalf)> {
    if offset >= 0x30 || (offset & 1) != 0 {
        return None;
    }
    let ch = (offset / 12) as usize;
    let id = ChannelId::try_from(ch).ok()?;
    let within = offset % 12;
    let kind = match within {
        0 => MmioHalf::SadLo,
        2 => MmioHalf::SadHi,
        4 => MmioHalf::DadLo,
        6 => MmioHalf::DadHi,
        8 => MmioHalf::CntL,
        10 => MmioHalf::CntH,
        _ => return None,
    };
    Some((id, kind))
}
