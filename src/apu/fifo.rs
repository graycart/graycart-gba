//! DMA sound FIFOs (Sound A / Sound B).
//!
//! Cited: GBATEK Sound Controller, DMA sound.
//! <https://problemkaputt.de/gbatek.htm>

/// Capacity of each DMA sound FIFO in bytes.
const CAPACITY: usize = 32;

/// Half-full threshold: refill when at most this many bytes remain.
const REFILL_THRESHOLD: usize = 16;

/// SRAM mirror range forbidden as a DMA sound source.
const SRAM_LO: u32 = 0x0E00_0000;
const SRAM_HI: u32 = 0x0FFF_FFFF;

/// One 32-byte DMA sound FIFO.
#[derive(Debug, Clone)]
pub struct Fifo {
    buf: [u8; CAPACITY],
    /// Next byte to pop.
    head: usize,
    len: usize,
    underrun: u32,
    overrun: u32,
    empty_drain: u32,
    dma_requests: u32,
}

impl Fifo {
    pub fn new() -> Self {
        Self {
            buf: [0; CAPACITY],
            head: 0,
            len: 0,
            underrun: 0,
            overrun: 0,
            empty_drain: 0,
            dma_requests: 0,
        }
    }

    /// Push one sample byte (low address / low byte of a store plays first).
    ///
    /// When full, the byte is dropped and counts an overrun. A successful push that
    /// leaves `len > 16` clears the refill request.
    pub fn push_byte(&mut self, byte: u8) {
        if self.len >= CAPACITY {
            self.overrun = self.overrun.saturating_add(1);
            return;
        }
        let idx = (self.head + self.len) % CAPACITY;
        self.buf[idx] = byte;
        self.len += 1;
    }

    /// Push four bytes from a little-endian word (low byte plays first).
    ///
    /// If fewer than four bytes are free, the write is dropped and counts an overrun.
    pub fn push_word(&mut self, value: u32) {
        if self.len + 4 > CAPACITY {
            self.overrun = self.overrun.saturating_add(1);
            return;
        }
        for i in 0..4 {
            let byte = (value >> (8 * i)) as u8;
            let idx = (self.head + self.len) % CAPACITY;
            self.buf[idx] = byte;
            self.len += 1;
        }
    }

    /// Pop one signed sample byte. Empty FIFO returns 0 and counts underrun + empty-drain.
    pub fn pop(&mut self) -> i8 {
        if self.len == 0 {
            self.underrun = self.underrun.saturating_add(1);
            self.empty_drain = self.empty_drain.saturating_add(1);
            return 0;
        }
        let was_above = self.len > REFILL_THRESHOLD;
        let sample = self.buf[self.head] as i8;
        self.head = (self.head + 1) % CAPACITY;
        self.len -= 1;
        if was_above && self.len <= REFILL_THRESHOLD {
            self.dma_requests = self.dma_requests.saturating_add(1);
        }
        sample
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Clear buffered bytes. Counters are left alone.
    pub fn reset(&mut self) {
        self.buf = [0; CAPACITY];
        self.head = 0;
        self.len = 0;
    }

    /// True when the FIFO holds 16 bytes or fewer (including empty).
    pub fn wants_refill(&self) -> bool {
        self.len <= REFILL_THRESHOLD
    }

    /// True while a refill is still needed (`len <= 16`).
    ///
    /// Does not clear the request: a failed DMA must leave it asserted until a push
    /// leaves `len > 16`.
    pub fn take_dma_request(&mut self) -> bool {
        self.wants_refill()
    }

    pub fn underrun(&self) -> u32 {
        self.underrun
    }

    pub fn overrun(&self) -> u32 {
        self.overrun
    }

    pub fn empty_drain(&self) -> u32 {
        self.empty_drain
    }

    pub fn dma_requests(&self) -> u32 {
        self.dma_requests
    }
}

impl Default for Fifo {
    fn default() -> Self {
        Self::new()
    }
}

/// Returns false when `addr` lies in the GBA SRAM range `0x0E000000..=0x0FFFFFFF`.
///
/// Does not read memory.
pub fn source_allowed(addr: u32) -> bool {
    !(SRAM_LO..=SRAM_HI).contains(&addr)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_word_pops_little_endian_then_underruns() {
        let mut fifo = Fifo::new();
        fifo.push_word(0x0403_0201);
        assert_eq!(fifo.pop(), 0x01_i8);
        assert_eq!(fifo.pop(), 0x02_i8);
        assert_eq!(fifo.pop(), 0x03_i8);
        assert_eq!(fifo.pop(), 0x04_i8);
        assert_eq!(fifo.pop(), 0);
        assert_eq!(fifo.underrun(), 1);
        assert_eq!(fifo.empty_drain(), 1);
    }

    #[test]
    fn overrun_when_full_rejects_word() {
        let mut fifo = Fifo::new();
        for i in 0..8 {
            fifo.push_word(i);
        }
        assert_eq!(fifo.len(), 32);
        fifo.push_word(0xDEAD_BEEF);
        assert_eq!(fifo.len(), 32);
        assert_eq!(fifo.overrun(), 1);
    }

    #[test]
    fn dma_request_stays_while_at_or_below_half() {
        let mut fifo = Fifo::new();
        for i in 0..8 {
            fifo.push_word(i);
        }
        assert_eq!(fifo.len(), 32);
        for _ in 0..16 {
            let _ = fifo.pop();
        }
        assert_eq!(fifo.len(), 16);
        assert!(fifo.wants_refill());
        assert!(fifo.take_dma_request());
        // Failed refill (no push): request must stay asserted.
        assert!(fifo.take_dma_request());
        assert_eq!(fifo.dma_requests(), 1);

        let _ = fifo.pop();
        assert_eq!(fifo.len(), 15);
        assert!(fifo.take_dma_request());

        // One word leaves len at 19 (>16): request clears.
        fifo.push_word(0x0102_0304);
        assert_eq!(fifo.len(), 19);
        assert!(!fifo.take_dma_request());
    }

    #[test]
    fn empty_fifo_wants_refill() {
        let fifo = Fifo::new();
        assert_eq!(fifo.len(), 0);
        assert!(fifo.wants_refill());
    }

    #[test]
    fn source_allowed_rejects_sram() {
        assert!(source_allowed(0x0300_0000));
        assert!(!source_allowed(0x0E00_0000));
        assert!(!source_allowed(0x0F00_0100));
    }
}
