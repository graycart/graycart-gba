//! Scanline / frame timing + DISPSTAT flags (P4).
//!
//! Cited: GBATEK — LCD V-Counters / Dimensions
//!   https://problemkaputt.de/gbatek.htm
//! Research: Project store `docs/graycart-gba/03-ppu.md` §10
//! Note: HBlank flag uses teaching model (960 draw + 272 blank) for functional
//!   P4; 1006/226 split is stretch / NBA-aligned TBD.

use crate::irq::{Irq, IRQ_HBLANK, IRQ_VBLANK, IRQ_VCOUNT};

use super::regs::LcdRegs;

/// CPU cycles per scanline.
pub const CYCLES_PER_LINE: u32 = 1232;
/// Visible dots (HDraw) in cycles (4 cyc/dot × 240).
pub const HDRAW_CYCLES: u32 = 960;
/// Scanlines per frame.
pub const LINES_PER_FRAME: u16 = 228;
/// Visible lines (VDraw).
pub const VDRAW_LINES: u16 = 160;
/// Last line where VBlank **flag** is set (160..=226); cleared on 227.
pub const VBLANK_FLAG_LAST: u16 = 226;
/// Total cycles per frame.
pub const CYCLES_PER_FRAME: u32 = CYCLES_PER_LINE * (LINES_PER_FRAME as u32);

/// Scanline timing state.
#[derive(Debug, Clone, Default)]
pub struct Timing {
    /// Cycle index within the current line `0..CYCLES_PER_LINE`.
    pub cycle_in_line: u32,
    /// Current VCOUNT (LY) `0..227`.
    pub vcount: u16,
    /// DISPSTAT flag bits 0–2 (VBlank / HBlank / VCounter).
    pub flags: u16,
    entered_hblank: bool,
    entered_vblank: bool,
    vcount_match_rose: bool,
    prev_vcount_match: bool,
    /// Line that should be rendered (set when entering HBlank on VDraw).
    pending_render: Option<u16>,
}

impl Timing {
    #[must_use]
    pub fn in_hblank(&self) -> bool {
        self.cycle_in_line >= HDRAW_CYCLES
    }

    #[must_use]
    pub fn in_vblank_flag(&self) -> bool {
        (160..=VBLANK_FLAG_LAST).contains(&self.vcount)
    }

    #[must_use]
    pub fn in_vdraw(&self) -> bool {
        self.vcount < VDRAW_LINES
    }

    /// True if the last [`Self::step`] entered VBlank (edge).
    #[must_use]
    pub fn entered_vblank_edge(&self) -> bool {
        self.entered_vblank
    }

    /// True if the last [`Self::step`] entered HBlank (edge).
    #[must_use]
    pub fn entered_hblank_edge(&self) -> bool {
        self.entered_hblank
    }

    /// Advance `cycles`. Returns the visible scanline to render when HBlank begins
    /// (at most one per call — callers should step ≤ line length for safety).
    pub fn step(&mut self, cycles: u32, regs: &LcdRegs) -> Option<u16> {
        self.entered_hblank = false;
        self.entered_vblank = false;
        self.vcount_match_rose = false;
        self.pending_render = None;

        for _ in 0..cycles {
            let was_hblank = self.in_hblank();
            let was_vblank = self.in_vblank_flag();

            self.cycle_in_line += 1;

            if !was_hblank && self.in_hblank() {
                self.entered_hblank = true;
                if self.in_vdraw() {
                    self.pending_render = Some(self.vcount);
                }
            }

            if self.cycle_in_line >= CYCLES_PER_LINE {
                self.cycle_in_line = 0;
                self.vcount += 1;
                if self.vcount >= LINES_PER_FRAME {
                    self.vcount = 0;
                }
                if !was_vblank && self.in_vblank_flag() {
                    self.entered_vblank = true;
                }
            }
        }

        self.refresh_flags(regs);
        self.pending_render
    }

    fn refresh_flags(&mut self, regs: &LcdRegs) {
        let mut f = 0u16;
        if self.in_vblank_flag() {
            f |= 1 << 0;
        }
        if self.in_hblank() {
            f |= 1 << 1;
        }
        let match_ly = self.vcount == regs.lyc();
        if match_ly {
            f |= 1 << 2;
            if !self.prev_vcount_match {
                self.vcount_match_rose = true;
            }
        }
        self.prev_vcount_match = match_ly;
        self.flags = f;
    }

    /// Raise LCD IRQs based on edges + DISPSTAT enables.
    pub fn service_irqs(&mut self, regs: &LcdRegs, irq: &mut Irq) {
        if self.entered_vblank && regs.irq_vblank_en() {
            irq.raise(IRQ_VBLANK);
        }
        if self.entered_hblank && regs.irq_hblank_en() {
            irq.raise(IRQ_HBLANK);
        }
        if self.vcount_match_rose && regs.irq_vcount_en() {
            irq.raise(IRQ_VCOUNT);
        }
    }
}
