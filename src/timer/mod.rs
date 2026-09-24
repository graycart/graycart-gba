//! Four GBA hardware timers.
//!
//! Cited: GBATEK Timers. https://problemkaputt.de/gbatek.htm
//!
//! DMA writes to TMxCNT_H use ares-style latch (`write16`). CPU writes apply
//! immediately (`write16_immediate`) so pause-timing Imm/HBlank samples keep
//! the HEAD phase under GBATEK I/O Word=2 setup timing.

#[cfg(test)]
mod tests;

const PRESCALE: [u32; 4] = [1, 64, 256, 1024];

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
struct Timer {
    reload: u16,
    counter: u16,
    control: u16,
    control_latch: u16,
    enable_reload: u16,
    control_dirty: bool,
    divider: u32,
    start_latency: u8,
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
            divider: 0,
            start_latency: 0,
        }
    }

    fn started(&self) -> bool {
        self.control & (1 << 7) != 0
    }

    fn count_up(&self) -> bool {
        self.control & (1 << 2) != 0
    }

    fn rate(&self) -> u32 {
        PRESCALE[(self.control & 0b11) as usize]
    }

    fn apply_control_latch(&mut self) {
        if !self.control_dirty {
            return;
        }
        let was_started = self.started();
        self.control = self.control_latch;
        self.control_dirty = false;
        if !was_started && self.started() {
            self.counter = self.enable_reload;
            self.divider = 0;
            self.start_latency = 1;
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Timers {
    timers: [Timer; 4],
}

impl Timers {
    pub fn new() -> Self {
        Self {
            timers: [Timer::new(); 4],
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
        t.control = value;
        t.control_dirty = false;
        if !was_started && t.started() {
            t.counter = t.reload;
            t.divider = 0;
            t.start_latency = 1;
        } else if !t.started() {
            t.start_latency = 0;
        }
    }

    pub fn tick(&mut self, cycles: u32) -> u8 {
        let mut overflowed = 0u8;
        for _ in 0..cycles {
            let mut prev_overflow = false;
            for i in 0..4 {
                let mut this_overflow = false;
                if self.timers[i].started() {
                    if self.timers[i].start_latency > 0 {
                        self.timers[i].start_latency -= 1;
                        prev_overflow = false;
                    } else {
                        let cascade = i > 0 && self.timers[i].count_up();
                        let step = if cascade {
                            prev_overflow
                        } else {
                            let t = &mut self.timers[i];
                            t.divider += 1;
                            if t.divider >= t.rate() {
                                t.divider = 0;
                                true
                            } else {
                                false
                            }
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
                self.timers[i].apply_control_latch();
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
        self.timers.iter().any(|t| {
            t.started() || (t.control_dirty && t.control_latch & (1 << 7) != 0)
        })
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
