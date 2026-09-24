//! Four GBA hardware timers.
//!
//! Cited: GBATEK Timers. https://problemkaputt.de/gbatek.htm

#[cfg(test)]
mod tests;

const PRESCALE: [u32; 4] = [1, 64, 256, 1024];

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
struct Timer {
    /// Value written to TMxCNT_L; loaded into `counter` on start edge or overflow.
    reload: u16,
    /// Live counter returned by reads of TMxCNT_L.
    counter: u16,
    /// TMxCNT_H.
    control: u16,
    /// Cycles toward the next increment when not in count-up mode.
    divider: u32,
}

impl Timer {
    fn new() -> Self {
        Self {
            reload: 0,
            counter: 0,
            control: 0,
            divider: 0,
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
}

/// GBA timers 0–3 (I/O base 0x04000100).
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

    /// Read a 16-bit timer register at `offset` from 0x04000100.
    pub fn read16(&self, offset: u32) -> u16 {
        let Some(index) = timer_index(offset) else {
            return 0;
        };
        let t = &self.timers[index];
        match offset & 2 {
            0 => t.counter,
            _ => t.control,
        }
    }

    /// Write a 16-bit timer register at `offset` from 0x04000100.
    pub fn write16(&mut self, offset: u32, value: u16) {
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
        if !was_started && t.started() {
            t.counter = t.reload;
            t.divider = 0;
        }
    }

    /// Advance this many 16.78 MHz cycles.
    ///
    /// Returns a bitmask of timers that overflowed at least once (bit 0 = timer 0).
    pub fn tick(&mut self, cycles: u32) -> u8 {
        let mut overflowed = 0u8;
        for _ in 0..cycles {
            let mut prev_overflow = false;
            for i in 0..4 {
                let mut this_overflow = false;
                if self.timers[i].started() {
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

    pub fn cascade(&self, index: usize) -> bool {
        self.timers[index].count_up()
    }
}

impl Default for Timers {
    fn default() -> Self {
        Self::new()
    }
}

fn timer_index(offset: u32) -> Option<usize> {
    if offset >= 16 || !offset.is_multiple_of(2) {
        return None;
    }
    Some((offset / 4) as usize)
}
