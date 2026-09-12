//! DMA channels 0–3: register file + Immediate start (P2).
//!
//! Cited: GBATEK -- GBA DMA Transfers
//!   https://problemkaputt.de/gbatek-gba-dma-transfers.htm
//! Cross-check: NanoBoyAdvance DMA notes (secondary) — Immediate ignores Repeat;
//!   address masks on write; SRAM DMA never.
//! Research: Project store `docs/graycart-gba/02-memory-bus-dma.md` §8.
//!
//! **P2 scope:** channels 0–3 SAD/DAD/CNT; Immediate start; CPU halted while a
//! transfer is active; reject Game Pak SRAM window. VBlank / HBlank / Special /
//! FIFO / Video Capture are **P5** stubs only (regs accepted, not started).

mod channel;

#[cfg(test)]
mod tests;

pub use channel::{
    control_word, Channel, ChannelId, DestControl, SrcControl, StartTiming, CONTROL_ENABLE,
    CONTROL_IRQ, CONTROL_REPEAT, CONTROL_TRANSFER_TYPE,
};

use crate::bus::CpuMem;
use channel::Channel as Chan;

/// Game Pak SRAM / Flash-save window (8-bit bus). DMA must never touch this.
/// Includes the mirrored field `0E000000–0FFFFFFF`.
pub const SRAM_WINDOW_START: u32 = 0x0E00_0000;
pub const SRAM_WINDOW_END: u32 = 0x0FFF_FFFF;

/// Why an Immediate DMA was refused before touching the bus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DmaReject {
    /// SAD or DAD lands in the SRAM / Flash-save window.
    SramWindow { channel: ChannelId, addr: u32 },
}

/// Outcome of draining Immediate DMA work.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DmaRunReport {
    /// Units (halfwords or words) successfully copied this call.
    pub units_transferred: u32,
    /// Channels that finished an Immediate burst this call.
    pub completed: Vec<ChannelId>,
    /// Channels that were armed but rejected (e.g. SRAM).
    pub rejected: Vec<DmaReject>,
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

    /// True while any channel holds an active Immediate transfer.
    ///
    /// The CPU must not execute guest instructions while this is set
    /// (GBATEK: CPU halted during active DMA units).
    #[inline]
    pub fn cpu_halted(&self) -> bool {
        self.channels.iter().any(|c| c.active)
    }

    /// True if any channel is active or has a pending Immediate arm.
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
        // Only CNT_H is readable on hardware; others return open-bus (TBD → 0 for P2).
        match decode_mmio_half(offset) {
            Some((id, MmioHalf::CntH)) => self.read_control(id),
            _ => 0,
        }
    }

    /// Drain all pending Immediate transfers in priority order (0→3).
    ///
    /// While any channel is actively transferring, [`Self::cpu_halted`] is true.
    /// Startup delay of 2 cycles is **not** modeled yet (TBD / P8 timing).
    pub fn run_immediate<M: CpuMem>(&mut self, mem: &mut M) -> DmaRunReport {
        let mut report = DmaRunReport::default();

        // Priority: always prefer the lowest channel index with pending/active work.
        loop {
            let Some(idx) = self.next_immediate_index() else {
                break;
            };
            let id = ChannelId::try_from(idx).expect("index 0..=3");

            // Validate SRAM before becoming active.
            let (sad, dad) = {
                let ch = &self.channels[idx];
                (ch.latched_sad, ch.latched_dad)
            };
            if let Some(bad) = sram_touch_addr(sad).or_else(|| sram_touch_addr(dad)) {
                self.channels[idx].finish();
                report.rejected.push(DmaReject::SramWindow {
                    channel: id,
                    addr: bad,
                });
                continue;
            }

            self.channels[idx].begin_active();
            debug_assert!(self.cpu_halted());

            let units = self.transfer_active_channel(idx, mem);
            report.units_transferred += units;
            report.completed.push(id);
        }

        debug_assert!(!self.cpu_halted());
        report
    }

    fn next_immediate_index(&self) -> Option<usize> {
        self.channels
            .iter()
            .position(|c| c.pending_immediate || c.active)
    }

    fn transfer_active_channel<M: CpuMem>(&mut self, idx: usize, mem: &mut M) -> u32 {
        let mut units = 0u32;
        while self.channels[idx].remaining > 0 {
            // Re-check SRAM each unit in case latched addrs walk into the window.
            let (sad, dad, word32) = {
                let ch = &self.channels[idx];
                (ch.latched_sad, ch.latched_dad, ch.transfer32())
            };
            if let Some(_bad) = sram_touch_addr(sad).or_else(|| sram_touch_addr(dad)) {
                // Mid-walk into SRAM: abort remainder, clear enable.
                self.channels[idx].finish();
                return units;
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
        self.channels[idx].finish();
        units
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
    if offset >= 0x30 || offset % 4 != 0 {
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
    if offset >= 0x30 || offset % 2 != 0 {
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
