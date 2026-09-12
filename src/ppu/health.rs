//! Framebuffer / LCD health heuristics for `--debug` video summaries.
//!
//! Cited: graycart-gb Debug Monitor health overview (spirit — console for GBA)
//!   https://github.com/graycart/graycart-gb (src/frontend/debug/ui/health.rs)
//! Cited: GBATEK — DISPCNT / mosaic / blend / windows
//!   https://problemkaputt.de/gbatek.htm
//! Note: cheap subsampled scan; not a pixel-perfect compositor audit.

use super::regs::LcdRegs;
use super::{FB_HEIGHT, FB_WIDTH};

/// Fraction of sampled pixels that must match to flag all-black / backdrop-only.
pub const MONO_FRAME_PCT: u32 = 98;

/// Result of a cheap framebuffer health scan.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FrameHealth {
    pub samples: u32,
    pub black: u32,
    pub backdrop: u32,
    pub other: u32,
}

impl FrameHealth {
    #[must_use]
    pub fn black_pct(self) -> u32 {
        if self.samples == 0 {
            return 0;
        }
        (self.black * 100) / self.samples
    }

    #[must_use]
    pub fn backdrop_pct(self) -> u32 {
        if self.samples == 0 {
            return 0;
        }
        (self.backdrop * 100) / self.samples
    }

    #[must_use]
    pub fn is_all_black(self) -> bool {
        self.samples > 0 && self.black_pct() >= MONO_FRAME_PCT
    }

    #[must_use]
    pub fn is_backdrop_only(self) -> bool {
        self.samples > 0 && self.backdrop_pct() >= MONO_FRAME_PCT && !self.is_all_black()
    }
}

/// Subsample the RGB555 framebuffer (every 8th pixel) vs palette backdrop.
#[must_use]
pub fn analyze_framebuffer(fb: &[u16], backdrop: u16) -> FrameHealth {
    let mut h = FrameHealth::default();
    if fb.len() < FB_WIDTH * FB_HEIGHT {
        return h;
    }
    // Stride keeps this O(few thousand) per debug period, not per frame cost in hot path
    // when called once per summary period.
    for y in (0..FB_HEIGHT).step_by(4) {
        for x in (0..FB_WIDTH).step_by(4) {
            let px = fb[y * FB_WIDTH + x];
            h.samples += 1;
            if px == 0 {
                h.black += 1;
            } else if px == backdrop {
                h.backdrop += 1;
            } else {
                h.other += 1;
            }
        }
    }
    h
}

/// Backdrop colour from BG palette entry 0 (RGB555).
#[must_use]
pub fn backdrop_from_palette(palette: &[u8]) -> u16 {
    if palette.len() < 2 {
        return 0;
    }
    u16::from(palette[0]) | (u16::from(palette[1]) << 8)
}

/// Compact layer enable string (BG0–3 / OBJ / WIN0 / WIN1 / OBJWIN).
#[must_use]
pub fn layer_enable_label(regs: &LcdRegs) -> String {
    let mut parts = Vec::new();
    for (bit, name) in [
        (8u16, "BG0"),
        (9, "BG1"),
        (10, "BG2"),
        (11, "BG3"),
        (12, "OBJ"),
        (13, "WIN0"),
        (14, "WIN1"),
        (15, "OBJWIN"),
    ] {
        if regs.layer_enable(bit) {
            parts.push(name);
        }
    }
    if parts.is_empty() {
        "none".into()
    } else {
        parts.join("|")
    }
}

/// DISPCNT layer+win enable mask bits 8–15.
#[must_use]
pub fn layer_mask(dispcnt: u16) -> u8 {
    ((dispcnt >> 8) & 0xFF) as u8
}

/// Blend mode from BLDCNT[7:6]: 0 off, 1 alpha, 2 bright+, 3 bright−.
#[must_use]
pub fn blend_mode(regs: &LcdRegs) -> u16 {
    (regs.bldcnt >> 6) & 0x3
}

#[must_use]
pub fn blend_label(mode: u16) -> &'static str {
    match mode {
        0 => "off",
        1 => "alpha",
        2 => "bright+",
        _ => "bright-",
    }
}

#[must_use]
pub fn mosaic_active(regs: &LcdRegs) -> bool {
    regs.mosaic != 0
}

#[must_use]
pub fn objwin_active(regs: &LcdRegs) -> bool {
    regs.layer_enable(15)
}
