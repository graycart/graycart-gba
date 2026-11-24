//! Four GBA hardware timers.
//!
//! Cited: GBATEK Timers. https://problemkaputt.de/gbatek.htm
//! Shared free-running divider: [NBA timers](https://nba-emu.github.io/hw-docs/timers.html),
//! mGBA `GBATimerWriteTMCNT_HI` absolute align, jgenesis `(cycles >> shift)` edges.
//! Post-HLE phase `37` matches alyosha `timer.gba` (power-on /64 residual).
//!
//! DMA writes to TMxCNT_H use ares-style latch (`write16`). CPU writes apply
//! immediately (`write16_immediate`) so pause-timing Imm/HBlank samples keep
//! the HEAD phase under GBATEK I/O Word=2 setup timing.

#[cfg(test)]
mod tests;

/// TIM0 counter left by the BIOS boot sequence (only the low byte is pinned by hardware tests).
const POST_BOOT_TIM0: u16 = 0x008B;

const PRESCALE_SHIFT: [u8; 4] = [0, 6, 8, 10];

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
struct Timer {
    reload: u16,
    counter: u16,
    control: u16,
    control_latch: u16,
    enable_reload: u16,
    control_dirty: bool,
    start_latency: u8,
    /// Opcode-fetch cycles already charged before a CPU enable write.
    #[serde(default)]
    skip: u8,
    /// Pre-enable counter still visible to DMA reads for a few ticks after a
    /// rising enable (alyosha timer_reset: DMA samples 0x8B while the live
    /// counter has already reloaded).
    #[serde(default)]
    read_hold: Option<u16>,
    #[serde(default)]
    read_hold_left: u8,
    /// Applied as `start_latency` when the rising enable latch commits.
    #[serde(default)]
    pending_latency: u8,
    /// Set by CPU enable while counter was 0xFFFF; drained in [`Timers::tick`].
    #[serde(default)]
    pending_overflow: bool,
}

impl Timer {
    fn new() -> Self {
        Self {
            reload: 0,
            counter: 0,
            control: 0,
            control_latch: 0,
            enable_reload: 0,
            control_dirty: false,
            start_latency: 0,
            skip: 0,
            read_hold: None,
            read_hold_left: 0,
            pending_latency: 1,
            pending_overflow: false,
        }
    }

    fn started(&self) -> bool {
        self.control & (1 << 7) != 0
    }

    fn count_up(&self) -> bool {
        self.control & (1 << 2) != 0
    }

    fn shift(&self) -> u8 {
        PRESCALE_SHIFT[(self.control & 0b11) as usize]
    }

    fn apply_control_latch(&mut self) -> bool {
        if !self.control_dirty {
            return false;
        }
        let was_started = self.started();
        let was_ffff = self.counter == 0xFFFF;
        self.control = self.control_latch;
        self.control_dirty = false;
        if !was_started && self.started() {
            self.read_hold = Some(self.counter);
            self.read_hold_left = 5;
            self.counter = self.enable_reload;
            self.start_latency = self.pending_latency.max(1);
            self.pending_latency = 1;
            // A stopped counter sitting on 0xFFFF overflows on the enable edge
            // (alyosha timer_disable's second start).
            return was_ffff;
        }
        false
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Timers {
    timers: [Timer; 4],
    /// Shared free-running divider phase (NBA / mGBA absolute-time edges).
    #[serde(default)]
    phase: u64,
}

impl Default for Timers {
    fn default() -> Self {
        Self::new()
    }
}

impl Timers {
    pub fn new() -> Self {
        Self {
            timers: [Timer::new(); 4],
            phase: 0,
        }
    }

    /// State a cart sees after the BIOS boot jingle: TIM0 is stopped part-way
    /// through a count and its low byte is 0x8B (alyosha timer_reset reads it
    /// before touching the counter).
    pub fn set_post_boot(&mut self) {
        self.timers[0].counter = POST_BOOT_TIM0;
        // alyosha timer.gba: shared /64 phase after HLE boot (NBA absolute divider).
        self.phase = 37;
    }

    pub fn arm_skip(&mut self, index: usize, cycles: u8) {
        if index < 4 {
            self.timers[index].skip = cycles;
        }
    }

    pub fn read16(&self, offset: u32) -> u16 {
        let Some(index) = timer_index(offset) else {
            return 0;
        };
        let t = &self.timers[index];
        match offset & 2 {
            0 => t.counter,
            _ if t.control_dirty => t.control_latch,
            _ => t.control,
        }
    }

    /// Like [`Self::read16`], but a DMA fetch of CNT_L still sees the pre-enable
    /// counter while [`Timer::read_hold_left`] is live.
    pub fn read16_dma(&self, offset: u32) -> u16 {
        let Some(index) = timer_index(offset) else {
            return 0;
        };
        let t = &self.timers[index];
        match offset & 2 {
            0 => t
                .read_hold
                .filter(|_| t.read_hold_left > 0)
                .unwrap_or(t.counter),
            _ if t.control_dirty => t.control_latch,
            _ => t.control,
        }
    }

    /// DMA path: latch rising CNT_H until the next tick.
    pub fn write16(&mut self, offset: u32, value: u16) {
        let Some(index) = timer_index(offset) else {
            return;
        };
        let t = &mut self.timers[index];
        if offset & 2 == 0 {
            t.reload = value;
            return;
        }
        let enabling = value & (1 << 7) != 0;
        if !enabling {
            t.control = value;
            t.control_dirty = false;
            t.start_latency = 0;
            return;
        }
        t.control_latch = value;
        t.enable_reload = t.reload;
        t.control_dirty = true;
        t.pending_latency = 1;
    }

    /// DMA path: same as [`Self::write16`], but the rising enable waits longer
    /// before counting so timer_reset's CPU sample lands on 0xF3.
    pub fn write16_dma(&mut self, offset: u32, value: u16) {
        self.write16(offset, value);
        let Some(index) = timer_index(offset) else {
            return;
        };
        if offset & 2 != 0 {
            self.timers[index].pending_latency = 5;
        }
    }

    /// CPU path: apply CNT_H immediately (pause-timing).
    pub fn write16_immediate(&mut self, offset: u32, value: u16) {
        let Some(index) = timer_index(offset) else {
            return;
        };
        let t = &mut self.timers[index];
        if offset & 2 == 0 {
            t.reload = value;
            return;
        }
        let was_started = t.started();
        let was_ffff = t.counter == 0xFFFF;
        t.control = value;
        t.control_dirty = false;
        if !was_started && t.started() {
            // Enable at 0xFFFF overflows once before reload (alyosha timer_disable;
            // Alyosha TASVideos 2023-10-04).
            t.pending_overflow = was_ffff;
            t.counter = t.reload;
            // Near-overflow reloads skip arm_skip in the bus; don't also delay.
            // Match bus arm_skip withhold: only the last 24 counts to overflow.
            t.start_latency = if (0xFFE8..0xFFFF).contains(&t.reload) {
                0
            } else {
                1
            };
        } else if !t.started() {
            t.start_latency = 0;
        }
    }

    pub fn tick(&mut self, cycles: u32) -> u8 {
        let mut overflowed = 0u8;
        for _ in 0..cycles {
            let prev_phase = self.phase;
            self.phase = self.phase.wrapping_add(1);
            let mut prev_overflow = false;
            for i in 0..4 {
                if self.timers[i].pending_overflow {
                    self.timers[i].pending_overflow = false;
                    overflowed |= 1 << i;
                    prev_overflow = true;
                }
                if self.timers[i].read_hold_left > 0 {
                    self.timers[i].read_hold_left -= 1;
                    if self.timers[i].read_hold_left == 0 {
                        self.timers[i].read_hold = None;
                    }
                }
                let mut this_overflow = false;
                if self.timers[i].skip > 0 {
                    self.timers[i].skip -= 1;
                } else if self.timers[i].started() {
                    if self.timers[i].start_latency > 0 {
                        self.timers[i].start_latency -= 1;
                    } else {
                        let cascade = i > 0 && self.timers[i].count_up();
                        let step = if cascade {
                            prev_overflow
                        } else {
                            let shift = self.timers[i].shift();
                            (self.phase >> shift) != (prev_phase >> shift)
                        };
                        if step {
                            let t = &mut self.timers[i];
                            if t.counter == 0xFFFF {
                                t.counter = t.reload;
                                overflowed |= 1 << i;
                                this_overflow = true;
                            } else {
                                t.counter = t.counter.wrapping_add(1);
                            }
                        }
                    }
                }
                if self.timers[i].apply_control_latch() {
                    overflowed |= 1 << i;
                }
                // Cascade input for timer i+1 (false when this timer skipped/latency).
                prev_overflow = this_overflow;
            }
        }
        overflowed
    }

    pub fn counter(&self, index: usize) -> u16 {
        self.timers[index].counter
    }

    pub fn reload(&self, index: usize) -> u16 {
        self.timers[index].reload
    }

    pub fn any_started(&self) -> bool {
        self.timers
            .iter()
            .any(|t| t.started() || (t.control_dirty && t.control_latch & (1 << 7) != 0))
    }

    pub fn cascade(&self, index: usize) -> bool {
        self.timers[index].count_up()
    }
}

fn timer_index(offset: u32) -> Option<usize> {
    if offset < 16 {
        Some((offset / 4) as usize)
    } else {
        None
    }
}
