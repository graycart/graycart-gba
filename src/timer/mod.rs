//! Hardware timers 0–3 (P3) — reload, enable edge, prescale, cascade, overflow IRQ.
//!
//! Cited: GBATEK — GBA Timers
//!   https://problemkaputt.de/gbatek-gba-timers.htm
//! Research: Project store `docs/graycart-gba/05-io-timers-irq-input.md` §3
//! Cross-check: mGBA `timer.c` (secondary — functional first; no enable-latency TBD
//!   treated as hardware truth).
//!
//! **IRQ coupling:** overflow with local IRQ enable sets IF bits 3–6 via
//! [`IrqRaise`] → [`crate::irq::Irq::raise`] (IME/IE not required to *set* IF).

mod unit;

#[cfg(test)]
mod tests;

pub use unit::{
    prescale_period, Timer, TimerId, CTRL_COUNT_UP, CTRL_IRQ, CTRL_PRESCALE, CTRL_START,
    CTRL_WRITABLE,
};

/// Timer overflow IF bits — same values as [`crate::irq`].
pub use crate::irq::{IRQ_TIMER0, IRQ_TIMER1, IRQ_TIMER2, IRQ_TIMER3};

/// Absolute MMIO base for `TM0CNT_L`.
pub const TM0CNT_L_ADDR: u32 = 0x0400_0100;

/// Sink for timer overflow IF bits (3–6).
///
/// Prefer stepping with `&mut crate::irq::Irq` (implements this trait). Closures
/// and [`RecordingIrq`] remain for isolated unit tests.
pub trait IrqRaise {
    fn raise(&mut self, bit: u16);
}

impl IrqRaise for crate::irq::Irq {
    #[inline]
    fn raise(&mut self, bit: u16) {
        crate::irq::Irq::raise(self, bit);
    }
}

impl<F: FnMut(u16)> IrqRaise for F {
    #[inline]
    fn raise(&mut self, bit: u16) {
        (self)(bit);
    }
}

/// Lightweight IF latch for timer-only unit tests (does not own IE/IME).
#[derive(Debug, Default, Clone)]
pub struct RecordingIrq {
    /// Bits ORed by [`IrqRaise::raise`] (IF-shaped).
    pub if_bits: u16,
}

impl IrqRaise for RecordingIrq {
    #[inline]
    fn raise(&mut self, bit: u16) {
        self.if_bits |= bit;
    }
}

/// Four-channel timer block.
#[derive(Debug, Clone)]
pub struct Timers {
    channels: [Timer; 4],
}

impl Default for Timers {
    fn default() -> Self {
        Self::new()
    }
}

impl Timers {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            channels: [Timer::new(), Timer::new(), Timer::new(), Timer::new()],
        }
    }

    #[inline]
    #[must_use]
    pub fn channel(&self, id: TimerId) -> &Timer {
        &self.channels[id.index()]
    }

    #[inline]
    pub fn channel_mut(&mut self, id: TimerId) -> &mut Timer {
        &mut self.channels[id.index()]
    }

    /// Write `TMnCNT_L` — stores **reload** only (counter unchanged).
    pub fn write_reload(&mut self, id: TimerId, value: u16) {
        self.channels[id.index()].write_reload(value);
    }

    /// Read `TMnCNT_L` — returns **current counter** (frozen if stopped).
    #[must_use]
    pub fn read_counter(&self, id: TimerId) -> u16 {
        self.channels[id.index()].counter
    }

    /// Write `TMnCNT_H`. Start 0→1 loads reload into the counter.
    pub fn write_control(&mut self, id: TimerId, value: u16) {
        self.channels[id.index()].write_control(value);
    }

    /// Read `TMnCNT_H` (writable bits only).
    #[must_use]
    pub fn read_control(&self, id: TimerId) -> u16 {
        self.channels[id.index()].control
    }

    /// 32-bit write to `TMnCNT` (low = reload, high = control).
    ///
    /// GBATEK: when start goes 0→1 in the same 32-bit store, the **newly written
    /// reload** is latched into the counter — not two independent halfword writes
    /// with a stale reload.
    pub fn write_cnt32(&mut self, id: TimerId, value: u32) {
        let reload = value as u16;
        let control = (value >> 16) as u16;
        self.write_reload(id, reload);
        self.write_control(id, control);
    }

    /// Byte offset of `TMnCNT_L` within the timer MMIO window (`0` = TM0).
    #[inline]
    #[must_use]
    pub const fn mmio_offset(id: TimerId) -> usize {
        id.index() * 4
    }

    /// MMIO halfword write relative to `TM0CNT_L` (`offset` 0..=14).
    pub fn write_mmio16(&mut self, offset: usize, value: u16) {
        let id = match TimerId::from_index(offset / 4) {
            Some(id) => id,
            None => return,
        };
        if offset % 4 == 0 {
            self.write_reload(id, value);
        } else if offset % 4 == 2 {
            self.write_control(id, value);
        }
    }

    /// MMIO halfword read relative to `TM0CNT_L`.
    #[must_use]
    pub fn read_mmio16(&self, offset: usize) -> u16 {
        let id = match TimerId::from_index(offset / 4) {
            Some(id) => id,
            None => return 0,
        };
        if offset % 4 == 0 {
            self.read_counter(id)
        } else if offset % 4 == 2 {
            self.read_control(id)
        } else {
            0
        }
    }

    /// MMIO 32-bit write at `TMnCNT_L` aligned offset (`0`, `4`, `8`, `12`).
    pub fn write_mmio32(&mut self, offset: usize, value: u32) {
        let id = match TimerId::from_index(offset / 4) {
            Some(id) if offset % 4 == 0 => id,
            _ => return,
        };
        self.write_cnt32(id, value);
    }

    /// Advance all timers by `cycles` system clocks (~16.78 MHz).
    ///
    /// Cascade (count-up) timers ignore their prescaler and tick once per
    /// previous-timer overflow. TM0 never cascades.
    pub fn step(&mut self, cycles: u64, irq: &mut impl IrqRaise) {
        if cycles == 0 {
            return;
        }
        let mut overflows = [0u64; 4];
        for i in 0..4 {
            let id = TimerId::from_index(i).expect("0..4");
            let cascade = i > 0 && self.channels[i].count_up();
            if cascade {
                let ticks = overflows[i - 1];
                if ticks == 0 || !self.channels[i].enabled() {
                    continue;
                }
                let mut raise = |bit: u16| irq.raise(bit);
                overflows[i] = self.channels[i].add_ticks(ticks, id, &mut raise);
            } else if self.channels[i].enabled() {
                // TM0: count-up bit is unused — always free-running when started.
                let mut raise = |bit: u16| irq.raise(bit);
                overflows[i] = self.channels[i].step_cycles(cycles, id, &mut raise);
            }
        }
    }
}
