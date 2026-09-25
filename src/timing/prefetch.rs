//! Game Pak opcode prefetch buffer. Page 11 fills this in.
//!
//! Cited: GBATEK Game Pak Prefetch Buffer.
//! <https://problemkaputt.de/gbatek.htm>

/// Game Pak opcode prefetch buffer (up to eight halfwords).
///
/// Tracks which halfword addresses were filled during CPU idle bus time.
/// Does not read ROM; the caller still performs the bus access. This type
/// only answers whether a sequential opcode fetch was already prefetched.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Prefetch {
    enabled: bool,
    /// FIFO of filled halfword addresses (oldest at index 0).
    slots: [u32; Self::CAPACITY],
    len: usize,
    /// Leftover cycles toward the next halfword fill.
    carry: u32,
    hits: u64,
}

impl Default for Prefetch {
    fn default() -> Self {
        Self::new()
    }
}

impl Prefetch {
    const CAPACITY: usize = 8;

    pub fn new() -> Self {
        Self {
            enabled: false,
            slots: [0; Self::CAPACITY],
            len: 0,
            carry: 0,
            hits: 0,
        }
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.drop_buffer();
        }
    }

    /// Spend `cycles` of internal/idle bus time filling from `next_addr`.
    ///
    /// Each halfword costs `s_cycles` (clamped to at least 1). Leftover
    /// cycles carry into the next `idle` call. Stops at eight halfwords.
    pub fn idle(&mut self, cycles: u32, next_addr: u32, s_cycles: u32) {
        if !self.enabled || self.len >= Self::CAPACITY {
            return;
        }

        let cost = s_cycles.max(1);
        let mut available = self.carry.saturating_add(cycles);
        let mut addr = next_addr & !1;

        while self.len < Self::CAPACITY && available >= cost {
            // A 128 KiB Game Pak boundary stops the prefetcher; the CPU fetch
            // there is non-sequential (alyosha prefetcher readme).
            if self.len > 0 && addr & 0x1_FFFF == 0 {
                break;
            }
            available -= cost;
            self.slots[self.len] = addr;
            self.len += 1;
            addr = addr.wrapping_add(2);
        }

        self.carry = if self.len < Self::CAPACITY {
            available
        } else {
            0
        };
    }

    /// Try to satisfy a sequential opcode halfword fetch at `addr`.
    ///
    /// On a hit, consumes the oldest slot and returns `true`. On a miss,
    /// drops the whole buffer and returns `false` without counting a hit.
    /// When disabled, always misses and leaves the buffer empty.
    pub fn take(&mut self, addr: u32) -> bool {
        self.take_n(addr, 1)
    }

    /// Try to satisfy `count` contiguous halfword opcode fetches starting at `addr`.
    ///
    /// All-or-nothing: a partial match does not consume slots (it drops the
    /// buffer and misses). Used for ARM word fetches that need two halfwords.
    pub fn take_n(&mut self, addr: u32, count: usize) -> bool {
        if !self.enabled || count == 0 {
            return false;
        }

        let mut addr = addr & !1;
        if self.len < count {
            self.drop_buffer();
            return false;
        }
        for i in 0..count {
            if self.slots[i] != addr {
                self.drop_buffer();
                return false;
            }
            addr = addr.wrapping_add(2);
        }

        let remain = self.len - count;
        for i in 0..remain {
            self.slots[i] = self.slots[i + count];
        }
        self.len = remain;
        self.hits += count as u64;
        true
    }

    /// Drop all filled slots and leftover fill progress.
    pub fn invalidate(&mut self) {
        self.drop_buffer();
    }

    /// Number of successful `take` hits since [`Self::new`].
    pub fn hits(&self) -> u64 {
        self.hits
    }

    fn drop_buffer(&mut self) {
        self.len = 0;
        self.carry = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::Prefetch;

    #[test]
    fn disabled_idle_then_take_is_miss() {
        let mut p = Prefetch::new();
        p.set_enabled(false);
        p.idle(1, 0x0800_0000, 1);
        assert!(!p.take(0x0800_0000));
        assert_eq!(p.hits(), 0);
    }

    #[test]
    fn enabled_s1_idle_then_take_hits() {
        let mut p = Prefetch::new();
        p.set_enabled(true);
        p.idle(1, 0x0800_0000, 1);
        assert!(p.take(0x0800_0000));
        assert_eq!(p.hits(), 1);
        assert!(!p.take(0x0800_0002));
        assert_eq!(p.hits(), 1);
    }

    #[test]
    fn enabled_s2_needs_two_idle_cycles_to_fill() {
        let mut p = Prefetch::new();
        p.set_enabled(true);
        let addr = 0x0800_0000;
        p.idle(1, addr, 2);
        // One cycle of two required: still empty.
        p.idle(1, addr, 2);
        assert!(p.take(addr));
        assert_eq!(p.hits(), 1);
    }

    #[test]
    fn invalidate_after_fill_makes_take_miss() {
        let mut p = Prefetch::new();
        p.set_enabled(true);
        p.idle(1, 0x0800_0000, 1);
        p.invalidate();
        assert!(!p.take(0x0800_0000));
        assert_eq!(p.hits(), 0);
    }

    #[test]
    fn buffer_caps_at_eight_halfwords() {
        let mut p = Prefetch::new();
        p.set_enabled(true);
        let mut addr = 0x0800_0000;
        for _ in 0..8 {
            p.idle(1, addr, 1);
            addr = addr.wrapping_add(2);
        }
        // Ninth idle must not add a ninth slot.
        p.idle(1, addr, 1);

        for i in 0..8 {
            let half = 0x0800_0000u32.wrapping_add(i * 2);
            assert!(p.take(half), "expected hit at {half:#010x}");
        }
        assert_eq!(p.hits(), 8);
        assert!(!p.take(0x0800_0010));
        assert_eq!(p.hits(), 8);
    }

    #[test]
    fn take_n_word_is_atomic_on_partial_miss() {
        let mut p = Prefetch::new();
        p.set_enabled(true);
        // Only the first halfword of a word is present.
        p.idle(1, 0x0800_0000, 1);
        assert!(!p.take_n(0x0800_0000, 2));
        assert_eq!(p.hits(), 0);
        // Buffer was dropped; the lone halfword must not linger for a later take.
        assert!(!p.take(0x0800_0000));
        assert_eq!(p.hits(), 0);
    }

    #[test]
    fn take_n_word_consumes_two_on_hit() {
        let mut p = Prefetch::new();
        p.set_enabled(true);
        p.idle(1, 0x0800_0000, 1);
        p.idle(1, 0x0800_0002, 1);
        p.idle(1, 0x0800_0004, 1);
        assert!(p.take_n(0x0800_0000, 2));
        assert_eq!(p.hits(), 2);
        assert!(p.take(0x0800_0004));
        assert_eq!(p.hits(), 3);
    }
}
