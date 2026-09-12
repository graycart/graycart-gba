//! Game Pak prefetch buffer FSM (WAITCNT bit 14).
//!
//! Cited: GBATEK — GBA GamePak Prefetch
//!   https://problemkaputt.de/gbatek-gba-gamepak-prefetch.htm
//! Cited: mGBA — Cycle Counting, Prefetch (secondary)
//!   https://mgba.io/2015/06/27/cycle-counting-prefetch/
//! Cross-check: research `docs/graycart-gba/02-memory-bus-dma.md` §7.
//! Note: 8×16 buffer; fill on idle cart cycles; hit → 0 waits. Race penalty
//! (B-02) and mid-DMA interleave are stretch.

use super::wait::{cycles_for_waits, rom_force_nonseq, AccessKind, RomWindow, WaitTables};

/// Prefetch buffer capacity (GBATEK: eight 16-bit halfwords).
pub const PREFETCH_CAPACITY: usize = 8;

/// Game Pak opcode prefetch buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrefetchBuffer {
    enabled: bool,
    /// Ring of buffered halfwords.
    buf: [u16; PREFETCH_CAPACITY],
    /// Index of the oldest buffered halfword.
    head: usize,
    /// Number of halfwords currently buffered.
    count: usize,
    /// Next ROM halfword address the filler will fetch.
    next_addr: u32,
    /// Waitstates remaining before the in-flight halfword commits.
    wait_left: u32,
    /// True while a fill toward [`Self::next_addr`] is in progress.
    filling: bool,
    /// Address of the oldest buffered halfword (head), if `count > 0`.
    head_addr: u32,
    /// True for the first beat after [`Self::restart`] (priced as N).
    first_beat: bool,
}

impl Default for PrefetchBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl PrefetchBuffer {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            enabled: false,
            buf: [0; PREFETCH_CAPACITY],
            head: 0,
            count: 0,
            next_addr: 0,
            wait_left: 0,
            filling: false,
            head_addr: 0,
            first_beat: false,
        }
    }

    #[inline]
    #[must_use]
    pub const fn enabled(&self) -> bool {
        self.enabled
    }

    #[inline]
    #[must_use]
    pub const fn len(&self) -> usize {
        self.count
    }

    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.count == 0
    }

    #[inline]
    #[must_use]
    pub const fn is_filling(&self) -> bool {
        self.filling
    }

    /// Sync enable from WAITCNT bit 14. Disabling drains the buffer.
    pub fn set_enabled(&mut self, enable: bool) {
        if self.enabled == enable {
            return;
        }
        self.enabled = enable;
        if !enable {
            self.drain();
        }
    }

    /// Drop all buffered halfwords and cancel in-flight fill.
    pub fn drain(&mut self) {
        self.head = 0;
        self.count = 0;
        self.wait_left = 0;
        self.filling = false;
        self.head_addr = 0;
        self.next_addr = 0;
        self.first_beat = false;
    }

    /// Restart speculative fill at `addr` (halfword-aligned ROM address).
    pub fn restart(&mut self, addr: u32, tables: WaitTables) {
        self.drain();
        if !self.enabled {
            return;
        }
        let Some(window) = RomWindow::from_addr(addr) else {
            return;
        };
        let addr = addr & !1;
        self.next_addr = addr;
        self.first_beat = true;
        self.filling = true;
        // Cart occupancy = 1 + waitstates (retire one per idle cycle).
        self.wait_left = cycles_for_waits(tables.rom_half_waits(window, AccessKind::Nonseq));
    }

    /// Advance fill by `cycles` eligible idle cart cycles.
    pub fn tick_idle<F>(&mut self, cycles: u32, tables: WaitTables, mut read_half: F)
    where
        F: FnMut(u32) -> u16,
    {
        if !self.enabled || cycles == 0 {
            return;
        }
        if !self.filling && self.count < PREFETCH_CAPACITY {
            if RomWindow::from_addr(self.next_addr).is_some() && self.next_addr != 0 {
                self.start_beat(tables);
            } else {
                return;
            }
        }
        let mut left = cycles;
        while left > 0 && self.filling {
            if self.count >= PREFETCH_CAPACITY {
                self.filling = false;
                self.wait_left = 0;
                break;
            }
            if self.wait_left == 0 {
                self.start_beat(tables);
                if !self.filling {
                    break;
                }
            }
            let step = left.min(self.wait_left);
            self.wait_left -= step;
            left -= step;
            if self.wait_left == 0 {
                let half = read_half(self.next_addr);
                let slot = (self.head + self.count) % PREFETCH_CAPACITY;
                self.buf[slot] = half;
                if self.count == 0 {
                    self.head_addr = self.next_addr;
                }
                self.count += 1;
                self.next_addr = self.next_addr.wrapping_add(2);
                if self.count >= PREFETCH_CAPACITY {
                    self.filling = false;
                } else {
                    self.start_beat(tables);
                }
            }
        }
    }

    fn start_beat(&mut self, tables: WaitTables) {
        let Some(window) = RomWindow::from_addr(self.next_addr) else {
            self.filling = false;
            return;
        };
        let kind = if self.first_beat || rom_force_nonseq(self.next_addr) {
            AccessKind::Nonseq
        } else {
            AccessKind::Seq
        };
        self.first_beat = false;
        self.wait_left = cycles_for_waits(tables.rom_half_waits(window, kind));
        self.filling = true;
    }

    /// Peek the head halfword without draining (pricing helper).
    #[inline]
    #[must_use]
    pub fn peek_half(&self, addr: u32) -> Option<u16> {
        if !self.enabled || self.count == 0 {
            return None;
        }
        let addr = addr & !1;
        if addr != self.head_addr {
            return None;
        }
        Some(self.buf[self.head])
    }

    /// Try to satisfy a ROM opcode halfword fetch from the buffer.
    ///
    /// Returns `Some(half)` on hit (0 waitstates). On miss (wrong address),
    /// drains and returns `None` so the caller pays full ROM access.
    pub fn try_fetch_half(&mut self, addr: u32) -> Option<u16> {
        if !self.enabled || self.count == 0 {
            return None;
        }
        let addr = addr & !1;
        if addr != self.head_addr {
            self.drain();
            return None;
        }
        let half = self.buf[self.head];
        self.head = (self.head + 1) % PREFETCH_CAPACITY;
        self.count -= 1;
        self.head_addr = self.head_addr.wrapping_add(2);
        if self.count == 0 {
            self.head_addr = 0;
        }
        Some(half)
    }

    /// Cycles for a buffer hit (GBATEK: 0 waitstates → 1 cycle beat).
    #[inline]
    #[must_use]
    pub const fn hit_cycles() -> u32 {
        1
    }
}
