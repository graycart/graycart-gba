//! Audio Processing Unit placeholder (P6) + FIFO DMA request stub (P5).
//!
//! Module layout from graycart-gba implementation plan §2.2.
//! Behavior: see research `docs/graycart-gba/04-apu.md` (PSG not implemented).
//! Cited: GBATEK — Sound FIFO / DMA Sound
//!   https://problemkaputt.de/gbatek.htm
//! Note: P5 exposes a software FIFO-request latch for DMA1/2 Special; real
//! Timer0/1 half-empty sampling lands with P6.

/// Stub APU — PSG later; FIFO request bit for DMA coupling.
#[derive(Debug, Default)]
pub struct Apu {
    /// Bit0 → DMA1 FIFO request, bit1 → DMA2. Cleared when consumed by DMA glue.
    pub fifo_dma_request: u8,
}

impl Apu {
    /// Raise a FIFO refill request (tests / future timer sampling).
    pub fn request_fifo_dma(&mut self, dma1: bool, dma2: bool) {
        if dma1 {
            self.fifo_dma_request |= 1;
        }
        if dma2 {
            self.fifo_dma_request |= 2;
        }
    }

    /// Take and clear pending FIFO DMA request bits.
    pub fn take_fifo_dma_request(&mut self) -> u8 {
        let v = self.fifo_dma_request;
        self.fifo_dma_request = 0;
        v
    }
}
