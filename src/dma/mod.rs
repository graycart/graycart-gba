//! DMA transfers.
//!
//! Cited: GBATEK DMA Transfers.
//! <https://problemkaputt.de/gbatek.htm>

mod immediate;
mod region;
mod timing;

#[cfg(test)]
mod tests;

pub(crate) use immediate::Copy;
use immediate::copy_units;
use timing::Reason;

const CHANNELS: usize = 4;
const REG_BASE: u32 = 0xB0;
const REG_END: u32 = 0xB0 + (CHANNELS as u32) * 12;

/// Four DMA channels and the CPU stall counter for in-flight copies.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Dma {
    channels: [Channel; CHANNELS],
    /// Cycles the CPU must wait after a successful copy (one per unit).
    pub stall: u32,
    /// True while a unit copy is in progress; nested enable/fire must not start another.
    pub(crate) busy: bool,
    last: Option<LastTransfer>,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
struct LastTransfer {
    channel: u8,
    words: u32,
    reason: Reason,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
struct Channel {
    sad: u32,
    dad: u32,
    cnt_l: u16,
    cnt_h: u16,
    latch_src: u32,
    latch_dst: u32,
    latch_count: u32,
}

impl Channel {
    fn new() -> Self {
        Self {
            sad: 0,
            dad: 0,
            cnt_l: 0,
            cnt_h: 0,
            latch_src: 0,
            latch_dst: 0,
            latch_count: 0,
        }
    }

    fn clear_enable(&mut self) {
        self.cnt_h &= !(1 << 15);
    }
}

impl Default for Dma {
    fn default() -> Self {
        Self::new()
    }
}

impl Dma {
    pub fn new() -> Self {
        Self {
            channels: [Channel::new(); CHANNELS],
            stall: 0,
            busy: false,
            last: None,
        }
    }

    /// Live debug line for a finished frame summary.
    pub fn debug_line(&self, frame: u32) -> String {
        match self.last {
            Some(last) => format!(
                "gba-debug: dma frame={frame} ch={} words={} reason={}",
                last.channel,
                last.words,
                reason_name(last.reason)
            ),
            None => format!("gba-debug: dma frame={frame} ch=none words=0 reason=idle"),
        }
    }

    pub fn covers(off: u32, size: u32) -> bool {
        let end = off.saturating_add(size.saturating_sub(1));
        off < REG_END && end >= REG_BASE
    }

    pub fn load(&self, off: u32, size: u32) -> u32 {
        match size {
            2 => u32::from(self.load16(off)),
            4 => {
                let lo = u32::from(self.load16(off));
                let hi = u32::from(self.load16(off.wrapping_add(2)));
                lo | (hi << 16)
            }
            1 => {
                let half = self.load16(off & !1);
                u32::from(if off & 1 != 0 { half >> 8 } else { half & 0xFF })
            }
            _ => 0,
        }
    }

    fn load16(&self, off: u32) -> u16 {
        let Some((channel, local)) = decode(off) else {
            return 0;
        };
        let ch = &self.channels[channel];
        match local {
            0 => ch.sad as u16,
            2 => (ch.sad >> 16) as u16,
            4 => ch.dad as u16,
            6 => (ch.dad >> 16) as u16,
            8 => ch.cnt_l,
            10 => ch.cnt_h,
            _ => 0,
        }
    }

    /// Write DMA I/O. Returns `true` when CNT_H was touched (caller may start a transfer).
    pub fn store(&mut self, off: u32, value: u32, size: u32) -> Option<usize> {
        let mut touched = None;
        match size {
            2 => {
                if self.store16(off, value as u16) {
                    touched = channel_of(off);
                }
            }
            4 => {
                let lo_touch = self.store16(off, value as u16);
                let hi_touch = self.store16(off.wrapping_add(2), (value >> 16) as u16);
                if lo_touch || hi_touch {
                    touched = channel_of(off).or_else(|| channel_of(off.wrapping_add(2)));
                }
            }
            1 => {
                let aligned = off & !1;
                let cur = self.load16(aligned);
                let next = if off & 1 != 0 {
                    (cur & 0x00FF) | (((value as u16) & 0xFF) << 8)
                } else {
                    (cur & 0xFF00) | ((value as u16) & 0xFF)
                };
                if self.store16(aligned, next) {
                    touched = channel_of(aligned);
                }
            }
            _ => {}
        }
        touched
    }

    /// Returns true when this halfword write hit CNT_H.
    fn store16(&mut self, off: u32, value: u16) -> bool {
        let Some((channel, local)) = decode(off) else {
            return false;
        };
        let ch = &mut self.channels[channel];
        match local {
            0 => {
                ch.sad = (ch.sad & 0xFFFF_0000) | u32::from(value);
                false
            }
            2 => {
                ch.sad = (ch.sad & 0x0000_FFFF) | (u32::from(value) << 16);
                false
            }
            4 => {
                ch.dad = (ch.dad & 0xFFFF_0000) | u32::from(value);
                false
            }
            6 => {
                ch.dad = (ch.dad & 0x0000_FFFF) | (u32::from(value) << 16);
                false
            }
            8 => {
                ch.cnt_l = value;
                false
            }
            10 => {
                ch.cnt_h = value;
                true
            }
            _ => false,
        }
    }

    pub fn latch(&mut self, channel: usize) {
        let ch = &mut self.channels[channel];
        ch.latch_src = ch.sad;
        ch.latch_dst = ch.dad;
        ch.latch_count = timing::unit_count(channel, ch.cnt_l);
    }

    pub fn clear_enable(&mut self, channel: usize) {
        self.channels[channel].clear_enable();
    }

    pub fn cnt_h(&self, channel: usize) -> u16 {
        self.channels[channel].cnt_h
    }

    pub fn sad(&self, channel: usize) -> u32 {
        self.channels[channel].sad
    }

    pub fn dad(&self, channel: usize) -> u32 {
        self.channels[channel].dad
    }

    pub fn reason(&self, channel: usize) -> Option<Reason> {
        timing::reason(channel, self.channels[channel].cnt_h)
    }

    fn record(&mut self, channel: usize, words: u32, reason: Reason) {
        self.last = Some(LastTransfer {
            channel: channel as u8,
            words,
            reason,
        });
    }

    /// Build the unit-copy job for a normal (non-FIFO) fire.
    pub fn job(&self, channel: usize) -> Copy {
        let ch = &self.channels[channel];
        let cnt_h = ch.cnt_h;
        Copy {
            src: ch.latch_src,
            dst: ch.latch_dst,
            units: ch.latch_count,
            width32: timing::width32(cnt_h),
            src_ctrl: timing::src_ctrl(cnt_h),
            dst_ctrl: timing::dst_ctrl(cnt_h),
        }
    }

    /// Build the FIFO special job (always 32-bit, four words, fixed FIFO dest).
    ///
    /// Channel 1 always writes `0x040000A0`; channel 2 always writes `0x040000A4`.
    /// Destination address control is forced to fixed so the burst does not walk.
    pub fn fifo_job(&self, channel: usize) -> Copy {
        let ch = &self.channels[channel];
        let cnt_h = ch.cnt_h;
        Copy {
            src: ch.latch_src,
            dst: fifo_dest(channel),
            units: timing::FIFO_UNITS,
            width32: true,
            src_ctrl: timing::src_ctrl(cnt_h),
            dst_ctrl: 2, // fixed
        }
    }

    /// After a successful copy: stall, debug, IRQ mask, repeat / enable.
    pub fn finish(
        &mut self,
        channel: usize,
        reason: Reason,
        units: u32,
        width32: bool,
    ) -> Option<u16> {
        let ch = &mut self.channels[channel];
        let cnt_h = ch.cnt_h;
        let src_ctrl = timing::src_ctrl(cnt_h);
        let dst_ctrl = timing::dst_ctrl(cnt_h);
        let unit_size = if width32 { 4 } else { 2 };

        ch.latch_src = step_end(ch.latch_src, src_ctrl, units, unit_size);

        let repeat = timing::repeat(cnt_h);
        if repeat && reason != Reason::Immediate {
            ch.latch_count = if reason == Reason::Fifo {
                timing::FIFO_UNITS
            } else {
                timing::unit_count(channel, ch.cnt_l)
            };
            if reason == Reason::Fifo {
                ch.latch_dst = fifo_dest(channel);
            } else if dst_ctrl == 3 {
                ch.latch_dst = ch.dad;
            } else {
                ch.latch_dst = step_end(ch.latch_dst, dst_ctrl, units, unit_size);
            }
        } else {
            ch.clear_enable();
            if reason == Reason::Fifo {
                ch.latch_dst = fifo_dest(channel);
            } else if reason != Reason::Immediate {
                // keep latch_dst advanced when enable clears; unused until next enable
                ch.latch_dst = step_end(ch.latch_dst, dst_ctrl, units, unit_size);
            }
        }

        self.stall = self.stall.saturating_add(units);
        self.record(channel, units, reason);

        if timing::irq_on_end(cnt_h) {
            Some(timing::irq_mask(channel))
        } else {
            None
        }
    }
}

fn step_end(start: u32, ctrl: u8, units: u32, unit_size: u32) -> u32 {
    let delta = units.wrapping_mul(unit_size);
    match ctrl {
        0 | 3 => start.wrapping_add(delta),
        1 => start.wrapping_sub(delta),
        _ => start,
    }
}

/// Fixed FIFO_A / FIFO_B I/O addresses for DMA channels 1 and 2.
fn fifo_dest(channel: usize) -> u32 {
    match channel {
        1 => 0x0400_00A0,
        2 => 0x0400_00A4,
        _ => 0x0400_00A0,
    }
}

fn reason_name(reason: Reason) -> &'static str {
    match reason {
        Reason::Immediate => "immediate",
        Reason::VBlank => "vblank",
        Reason::HBlank => "hblank",
        Reason::Fifo => "fifo",
        Reason::VideoCapture => "video",
    }
}

fn decode(off: u32) -> Option<(usize, u32)> {
    if !(REG_BASE..REG_END).contains(&off) {
        return None;
    }
    let rel = off - REG_BASE;
    let channel = (rel / 12) as usize;
    let local = rel % 12;
    Some((channel, local))
}

fn channel_of(off: u32) -> Option<usize> {
    decode(off).map(|(ch, _)| ch)
}

/// Re-export copy entry for the bus transfer path.
pub(crate) fn run_copy(
    job: &Copy,
    read: &mut dyn FnMut(u32) -> u32,
    write: &mut dyn FnMut(u32, u32),
) -> u32 {
    copy_units(job, read, write)
}

pub(crate) fn region_access(
    channel: usize,
    src: u32,
    dst: u32,
) -> Result<region::Kind, &'static str> {
    region::access(channel, src, dst)
}

pub(crate) use timing::Reason as StartReason;
