//! Single 16-bit GBA timer channel.
//!
//! Cited: GBATEK — GBA Timers
//!   https://problemkaputt.de/gbatek-gba-timers.htm

/// Timer channel index 0–3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TimerId {
    Tm0 = 0,
    Tm1 = 1,
    Tm2 = 2,
    Tm3 = 3,
}

impl TimerId {
    #[inline]
    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }

    #[inline]
    #[must_use]
    pub const fn from_index(i: usize) -> Option<Self> {
        match i {
            0 => Some(Self::Tm0),
            1 => Some(Self::Tm1),
            2 => Some(Self::Tm2),
            3 => Some(Self::Tm3),
            _ => None,
        }
    }

    /// IE/IF bit for this timer's overflow (bits 3–6).
    #[inline]
    #[must_use]
    pub const fn irq_bit(self) -> u16 {
        1 << (3 + self as u16)
    }
}

/// Control bit masks (`TMnCNT_H`).
pub const CTRL_PRESCALE: u16 = 0b11;
pub const CTRL_COUNT_UP: u16 = 1 << 2;
pub const CTRL_IRQ: u16 = 1 << 6;
pub const CTRL_START: u16 = 1 << 7;
/// Writable control bits (unused bits ignored on write).
pub const CTRL_WRITABLE: u16 = CTRL_PRESCALE | CTRL_COUNT_UP | CTRL_IRQ | CTRL_START;

/// Prescaler period in system clocks: F/1, F/64, F/256, F/1024.
#[inline]
#[must_use]
pub const fn prescale_period(sel: u16) -> u32 {
    match sel & CTRL_PRESCALE {
        0 => 1,
        1 => 64,
        2 => 256,
        _ => 1024,
    }
}

/// One hardware timer (counter / reload / control + prescale phase).
#[derive(Debug, Clone)]
pub struct Timer {
    /// Current 16-bit up-counter (returned on `TMnCNT_L` read).
    pub counter: u16,
    /// Reload latch (stored on `TMnCNT_L` write; applied on overflow / start 0→1).
    pub reload: u16,
    /// Control halfword (`TMnCNT_H`), masked to writable bits.
    pub control: u16,
    /// Cycles accumulated toward the next prescaled tick (normal mode only).
    pub(crate) prescale_accum: u32,
}

impl Default for Timer {
    fn default() -> Self {
        Self::new()
    }
}

impl Timer {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            counter: 0,
            reload: 0,
            control: 0,
            prescale_accum: 0,
        }
    }

    #[inline]
    #[must_use]
    pub fn enabled(&self) -> bool {
        self.control & CTRL_START != 0
    }

    #[inline]
    #[must_use]
    pub fn irq_enable(&self) -> bool {
        self.control & CTRL_IRQ != 0
    }

    /// Count-up (cascade) mode. Ignored / unused on TM0 at the Timers layer.
    #[inline]
    #[must_use]
    pub fn count_up(&self) -> bool {
        self.control & CTRL_COUNT_UP != 0
    }

    #[inline]
    #[must_use]
    pub fn prescale_sel(&self) -> u16 {
        self.control & CTRL_PRESCALE
    }

    /// Write reload only — does **not** change the running counter.
    pub fn write_reload(&mut self, value: u16) {
        self.reload = value;
    }

    /// Write control. Start bit 0→1 latches `reload` into `counter` and clears
    /// the prescale phase (functional; enable-latency TBD not invented here).
    pub fn write_control(&mut self, value: u16) {
        let next = value & CTRL_WRITABLE;
        let was = self.control & CTRL_START != 0;
        let now = next & CTRL_START != 0;
        self.control = next;
        if !was && now {
            self.counter = self.reload;
            self.prescale_accum = 0;
        }
        if !now {
            self.prescale_accum = 0;
        }
    }

    /// Advance by `ticks` counter increments (after prescale or cascade).
    ///
    /// Each overflow: counter ← reload, optionally raise IRQ, and count toward
    /// cascade of the next timer.
    pub(crate) fn add_ticks(
        &mut self,
        mut ticks: u64,
        id: TimerId,
        raise: &mut dyn FnMut(u16),
    ) -> u64 {
        let mut overflows = 0u64;
        while ticks > 0 {
            let room = (0x1_0000u64).saturating_sub(u64::from(self.counter));
            if room == 0 {
                // Defensive: counter should never sit at 0x10000.
                self.counter = self.reload;
                overflows += 1;
                if self.irq_enable() {
                    raise(id.irq_bit());
                }
                continue;
            }
            if ticks < room {
                self.counter = self.counter.wrapping_add(ticks as u16);
                break;
            }
            ticks -= room;
            self.counter = self.reload;
            overflows += 1;
            if self.irq_enable() {
                raise(id.irq_bit());
            }
        }
        overflows
    }

    /// Free-running (prescaled) advance by `cycles` system clocks.
    pub(crate) fn step_cycles(
        &mut self,
        cycles: u64,
        id: TimerId,
        raise: &mut dyn FnMut(u16),
    ) -> u64 {
        if cycles == 0 || !self.enabled() {
            return 0;
        }
        let period = u64::from(prescale_period(self.prescale_sel()));
        let total = u64::from(self.prescale_accum).saturating_add(cycles);
        let ticks = total / period;
        self.prescale_accum = (total % period) as u32;
        if ticks == 0 {
            return 0;
        }
        self.add_ticks(ticks, id, raise)
    }
}
