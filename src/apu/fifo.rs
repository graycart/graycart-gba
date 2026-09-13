//! Direct Sound FIFO A/B (P6).
//!
//! Cited: GBATEK — FIFO / DMA Sound
//!   https://problemkaputt.de/gbatek.htm
//! Cited: jsgroth — GBA audio (7-word + hold; secondary)
//!   https://jsgroth.dev/blog/posts/gba-audio/
//! Cited: mGBA `GBAAudioSampleFIFO` — DMA when empty slots > 4, before word consume
//!   https://github.com/mgba-emu/mgba/blob/master/src/gba/audio.c
//! Research: Project store `docs/graycart-gba/04-apu.md` §4–§5
//! Note: capacity modelled as 32 samples (GBATEK); half-empty ≤16 → DMA request.
//! Underrun: hold last sample (Gericom / mGBA #1847).
//! Overflow: reset empty then accept the new write (Gericom secondary).
//! Glue must service FIFO DMA *before* pop on each timer edge (startup underrun).

/// Logical FIFO depth in samples (GBATEK: 8×32-bit = 32 bytes).
pub const FIFO_CAPACITY: usize = 32;
/// Request DMA when remaining samples ≤ this (4×32-bit words).
pub const FIFO_HALF: usize = 16;

/// One Direct Sound FIFO (signed 8-bit samples).
#[derive(Debug, Clone)]
pub struct Fifo {
    buf: [i8; FIFO_CAPACITY],
    /// Next pop index.
    head: usize,
    /// Next push index.
    tail: usize,
    /// Occupied sample count.
    len: usize,
    /// Last sample played (held on underrun).
    last: i8,
    /// Timer pops while empty (held last sample — pops / stuck tone risk).
    pub underruns: u64,
    /// Pushes that hit a full FIFO (overflow reset — saw/corruption risk).
    pub overruns: u64,
}

impl Default for Fifo {
    fn default() -> Self {
        Self::new()
    }
}

impl Fifo {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            buf: [0; FIFO_CAPACITY],
            head: 0,
            tail: 0,
            len: 0,
            last: 0,
            underruns: 0,
            overruns: 0,
        }
    }

    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.len
    }

    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Clear queue (SOUNDCNT_H reset bit). Health counters are preserved.
    pub fn reset(&mut self) {
        self.head = 0;
        self.tail = 0;
        self.len = 0;
        self.last = 0;
        self.buf = [0; FIFO_CAPACITY];
    }

    /// Push four samples from a little-endian 32-bit word (LSB played first).
    pub fn push_word(&mut self, word: u32) {
        for i in 0..4 {
            let s = (word >> (8 * i)) as u8 as i8;
            self.push_sample(s);
        }
    }

    pub fn push_sample(&mut self, sample: i8) {
        if self.len >= FIFO_CAPACITY {
            // Overflow: clear like SOUNDCNT_H reset (Gericom / mGBA #1847).
            // Drop-oldest advances the playhead under DMA pressure → harsh saw.
            self.head = 0;
            self.tail = 0;
            self.len = 0;
            self.last = 0;
            self.buf = [0; FIFO_CAPACITY];
            self.overruns = self.overruns.saturating_add(1);
        }
        self.buf[self.tail] = sample;
        self.tail = (self.tail + 1) % FIFO_CAPACITY;
        self.len += 1;
    }

    /// Pop one sample for the output latch; underrun → hold `last`.
    pub fn pop_sample(&mut self) -> i8 {
        if self.len == 0 {
            self.underruns = self.underruns.saturating_add(1);
            return self.last;
        }
        let s = self.buf[self.head];
        self.head = (self.head + 1) % FIFO_CAPACITY;
        self.len -= 1;
        self.last = s;
        s
    }

    /// True when a Sound DMA refill should be requested (≤ half full).
    #[inline]
    #[must_use]
    pub fn needs_dma(&self) -> bool {
        self.len <= FIFO_HALF
    }
}

/// FIFO A + B with latched output samples.
#[derive(Debug, Clone, Default)]
pub struct FifoPair {
    pub a: Fifo,
    pub b: Fifo,
    pub latch_a: i8,
    pub latch_b: i8,
}

impl FifoPair {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            a: Fifo::new(),
            b: Fifo::new(),
            latch_a: 0,
            latch_b: 0,
        }
    }

    pub fn reset_a(&mut self) {
        self.a.reset();
        self.latch_a = 0;
    }

    pub fn reset_b(&mut self) {
        self.b.reset();
        self.latch_b = 0;
    }

    /// Timer overflow for FIFO A: request DMA while half-empty **before** pop
    /// (jsgroth / mGBA), then latch next sample.
    pub fn on_timer_a(&mut self) -> bool {
        let need = self.a.needs_dma();
        self.latch_a = self.a.pop_sample();
        need || self.a.needs_dma()
    }

    /// Timer overflow for FIFO B.
    pub fn on_timer_b(&mut self) -> bool {
        let need = self.b.needs_dma();
        self.latch_b = self.b.pop_sample();
        need || self.b.needs_dma()
    }
}
