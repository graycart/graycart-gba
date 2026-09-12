//! Picture Processing Unit — scanline timing, modes 0–5, OBJ basics (P4).
//!
//! Cited: GBATEK — LCD I/O / Dimensions / BG / OBJ
//!   https://problemkaputt.de/gbatek.htm
//! Research: Project store `docs/graycart-gba/03-ppu.md`
//! Note: functional compositor; windows/blend/affine before cycle-perfect.
//! OAM line staging / HBlank 1006/226 split are stretch.

pub mod affine;
pub mod bg;
pub mod bitmap;
pub mod blend;
pub mod hash;
pub mod health;
pub mod obj;
pub mod regs;
pub mod render;
pub mod timing;
pub mod vram_fetch;
pub mod window;

#[cfg(test)]
mod tests_modes;
#[cfg(test)]
mod tests_obj;
#[cfg(test)]
mod tests_timing;
#[cfg(test)]
mod tests_video;

use crate::bus::Bus;
use crate::irq::Irq;

use bg::latch_affine_refs;
use bitmap::latch_bg2;
use hash::{format_hash_file, framebuffer_to_rgb888, sha256_hex};
use regs::LcdRegs;
use render::render_scanline;
use timing::{Timing, VDRAW_LINES};

/// Visible framebuffer width.
pub const FB_WIDTH: usize = 240;
/// Visible framebuffer height.
pub const FB_HEIGHT: usize = 160;

/// PPU with LCD regs, timing, and RGB555 framebuffer.
#[derive(Debug, Clone)]
pub struct Ppu {
    pub regs: LcdRegs,
    pub timing: Timing,
    /// 240×160 RGB555 presentment buffer.
    pub fb: Vec<u16>,
    /// Internal affine refs (BG2 / BG3) as signed 8.8.
    bg2_ref: (i32, i32),
    bg3_ref: (i32, i32),
    /// Frames completed (after VCOUNT 227 → 0).
    pub frames: u64,
    prev_vcount: u16,
}

impl Default for Ppu {
    fn default() -> Self {
        let mut p = Self {
            regs: LcdRegs::default(),
            timing: Timing::default(),
            fb: vec![0; FB_WIDTH * FB_HEIGHT],
            bg2_ref: (0, 0),
            bg3_ref: (0, 0),
            frames: 0,
            prev_vcount: 0,
        };
        p.relatch_affine();
        p
    }
}

impl Ppu {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn relatch_affine(&mut self) {
        self.bg2_ref = latch_bg2(&self.regs);
        self.bg3_ref = latch_affine_refs(&self.regs, 3);
    }

    /// MMIO halfword read (`off` within `0x04000000`).
    #[must_use]
    pub fn read16(&self, off: usize) -> u16 {
        self.regs.read16(off, self.timing.vcount, self.timing.flags)
    }

    /// MMIO halfword write.
    pub fn write16(&mut self, off: usize, value: u16) {
        let before_x2 = self.regs.bg2_x;
        let before_y2 = self.regs.bg2_y;
        let before_x3 = self.regs.bg3_x;
        let before_y3 = self.regs.bg3_y;
        self.regs.write16(off, value);
        // Mid-frame ref write updates internal immediately (03-ppu §4.1).
        if self.regs.bg2_x != before_x2 || self.regs.bg2_y != before_y2 {
            self.bg2_ref = latch_bg2(&self.regs);
        }
        if self.regs.bg3_x != before_x3 || self.regs.bg3_y != before_y3 {
            self.bg3_ref = latch_affine_refs(&self.regs, 3);
        }
    }

    /// Advance PPU by `cycles`; render scanlines; raise LCD IRQs.
    pub fn step(&mut self, cycles: u32, bus: &Bus, irq: &mut Irq) {
        let mut left = cycles;
        while left > 0 {
            let to_boundary = if self.timing.cycle_in_line < timing::HDRAW_CYCLES {
                timing::HDRAW_CYCLES - self.timing.cycle_in_line
            } else {
                timing::CYCLES_PER_LINE - self.timing.cycle_in_line
            };
            let chunk = left.min(to_boundary).max(1);
            let render_line = self.timing.step(chunk, &self.regs);
            if let Some(line) = render_line {
                self.render_line(line, bus);
            }
            self.timing.service_irqs(&self.regs, irq);

            if self.prev_vcount == 227 && self.timing.vcount == 0 {
                self.frames = self.frames.wrapping_add(1);
                self.relatch_affine();
            }
            self.prev_vcount = self.timing.vcount;
            left -= chunk;
        }
    }

    fn render_line(&mut self, line: u16, bus: &Bus) {
        if line >= VDRAW_LINES {
            return;
        }
        let mut line_buf = [0u16; FB_WIDTH];
        render_scanline(
            &self.regs,
            line,
            &bus.vram,
            &bus.palette,
            &bus.oam,
            self.bg2_ref,
            self.bg3_ref,
            &mut line_buf,
        );
        let start = usize::from(line) * FB_WIDTH;
        self.fb[start..start + FB_WIDTH].copy_from_slice(&line_buf);

        // After scanline: internals += (PB, PD) in 8.8.
        self.bg2_ref.0 += i32::from(self.regs.bg2_pb);
        self.bg2_ref.1 += i32::from(self.regs.bg2_pd);
        self.bg3_ref.0 += i32::from(self.regs.bg3_pb);
        self.bg3_ref.1 += i32::from(self.regs.bg3_pd);
    }

    /// RGB888 bytes of the presentment buffer.
    #[must_use]
    pub fn framebuffer_rgb(&self) -> Vec<u8> {
        framebuffer_to_rgb888(&self.fb)
    }

    /// SHA-256 hex of RGB888 framebuffer.
    #[must_use]
    pub fn frame_hash_sha256(&self) -> String {
        sha256_hex(&self.framebuffer_rgb())
    }

    /// Golden file body for current FB at `frame` index.
    #[must_use]
    pub fn hash_file_body(&self, frame: u64) -> String {
        format_hash_file(frame, &self.frame_hash_sha256())
    }

    /// Sync DISPSTAT/VCOUNT into `bus.io` for dumb peekers.
    pub fn mirror_status_to_io(&self, bus: &mut Bus) {
        let st = self.read16(0x04);
        let vc = self.read16(0x06);
        if let Some(s) = bus.io.get_mut(0x04..0x08) {
            s[0] = st as u8;
            s[1] = (st >> 8) as u8;
            s[2] = vc as u8;
            s[3] = (vc >> 8) as u8;
        }
    }
}

/// Re-export frame cycle count for `Gba::run_frames`.
pub use timing::CYCLES_PER_FRAME as FRAME_CYCLES;
