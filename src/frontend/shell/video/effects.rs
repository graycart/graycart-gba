//! Frontend-only display modes applied after shade→RGB (never touches core FB).

use graycart::{SCREEN_HEIGHT, SCREEN_WIDTH};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DisplayMode {
    #[default]
    Sharp,
    SoftLcd,
    Soft,
    PixelGrid,
    DmgGhosting,
}

impl DisplayMode {
    pub const ALL: [Self; 5] = [
        Self::Sharp,
        Self::SoftLcd,
        Self::Soft,
        Self::PixelGrid,
        Self::DmgGhosting,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Sharp => "Sharp Pixels",
            Self::SoftLcd => "Soft LCD",
            Self::Soft => "Soft",
            Self::PixelGrid => "Pixel Grid",
            Self::DmgGhosting => "DMG Ghosting",
        }
    }

    /// Prefer linear filtering (Fill) vs nearest (PixelPerfect).
    pub fn prefers_linear_filter(self) -> bool {
        matches!(self, Self::SoftLcd | Self::Soft)
    }
}

const FRAME_BYTES: usize = SCREEN_WIDTH * SCREEN_HEIGHT * 4;

/// Apply a CPU-side presentation effect in-place on RGBA8888 `frame`.
///
/// `scratch` is reused as soft-blur source and as the previous presented frame
/// for ghosting. Modes that do not need history clear it so the default Sharp
/// path does not allocate a full-frame clone every present.
pub fn apply_effect(mode: DisplayMode, frame: &mut [u8], scratch: &mut Option<Vec<u8>>) {
    debug_assert_eq!(frame.len(), FRAME_BYTES);
    match mode {
        DisplayMode::Sharp | DisplayMode::SoftLcd => {
            scratch.take();
        }
        DisplayMode::Soft => {
            soft_blur(frame, scratch);
        }
        DisplayMode::PixelGrid => {
            pixel_grid(frame);
            scratch.take();
        }
        DisplayMode::DmgGhosting => {
            if let Some(prev) = scratch.as_ref()
                && prev.len() == frame.len()
            {
                ghost_blend(frame, prev, 0.28);
            }
            store_frame(scratch, frame);
        }
    }
}

/// Copy `frame` into `dst`, reusing an existing buffer when the length matches.
fn store_frame(dst: &mut Option<Vec<u8>>, frame: &[u8]) {
    match dst {
        Some(buf) if buf.len() == frame.len() => buf.copy_from_slice(frame),
        _ => *dst = Some(frame.to_vec()),
    }
}

fn soft_blur(frame: &mut [u8], scratch: &mut Option<Vec<u8>>) {
    store_frame(scratch, frame);
    let src = scratch.as_ref().expect("scratch filled above");
    for y in 0..SCREEN_HEIGHT {
        for x in 0..SCREEN_WIDTH {
            let mut acc = [0u32; 3];
            let mut n = 0u32;
            for dy in -1i32..=1 {
                for dx in -1i32..=1 {
                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;
                    if nx < 0 || ny < 0 || nx >= SCREEN_WIDTH as i32 || ny >= SCREEN_HEIGHT as i32 {
                        continue;
                    }
                    let i = ((ny as usize) * SCREEN_WIDTH + nx as usize) * 4;
                    acc[0] += u32::from(src[i]);
                    acc[1] += u32::from(src[i + 1]);
                    acc[2] += u32::from(src[i + 2]);
                    n += 1;
                }
            }
            let o = (y * SCREEN_WIDTH + x) * 4;
            frame[o] = (acc[0] / n) as u8;
            frame[o + 1] = (acc[1] / n) as u8;
            frame[o + 2] = (acc[2] / n) as u8;
        }
    }
}

fn pixel_grid(frame: &mut [u8]) {
    for y in 0..SCREEN_HEIGHT {
        for x in 0..SCREEN_WIDTH {
            if x % 2 == 1 || y % 2 == 1 {
                let o = (y * SCREEN_WIDTH + x) * 4;
                frame[o] = (u16::from(frame[o]) * 3 / 4) as u8;
                frame[o + 1] = (u16::from(frame[o + 1]) * 3 / 4) as u8;
                frame[o + 2] = (u16::from(frame[o + 2]) * 3 / 4) as u8;
            }
        }
    }
}

fn ghost_blend(frame: &mut [u8], previous: &[u8], ghost: f32) {
    let keep = 1.0 - ghost;
    for (dst, &src) in frame.iter_mut().zip(previous.iter()) {
        *dst = (f32::from(*dst) * keep + f32::from(src) * ghost) as u8;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn ghosting_changes_frame_when_history_exists() {
        let mut frame = vec![255u8; FRAME_BYTES];
        let mut prev = Some(vec![0u8; FRAME_BYTES]);
        apply_effect(DisplayMode::DmgGhosting, &mut frame, &mut prev);
        assert!(frame[0] < 255);
    }

    #[test]
    fn sharp_and_soft_lcd_drop_frame_history() {
        let mut frame = vec![1u8; FRAME_BYTES];
        let mut prev = Some(vec![0u8; FRAME_BYTES]);
        apply_effect(DisplayMode::Sharp, &mut frame, &mut prev);
        assert!(prev.is_none());
        prev = Some(vec![0u8; FRAME_BYTES]);
        apply_effect(DisplayMode::SoftLcd, &mut frame, &mut prev);
        assert!(prev.is_none());
    }

    #[test]
    fn pixel_grid_does_not_retain_history() {
        let mut frame = vec![200u8; FRAME_BYTES];
        let mut prev = Some(vec![0u8; FRAME_BYTES]);
        apply_effect(DisplayMode::PixelGrid, &mut frame, &mut prev);
        assert!(prev.is_none());
        assert!(frame[4] < 200); // odd column darkened
    }

    #[test]
    fn ghosting_reuses_previous_buffer_capacity() {
        let mut frame = vec![255u8; FRAME_BYTES];
        let mut prev = Some(vec![0u8; FRAME_BYTES]);
        let ptr = prev.as_ref().unwrap().as_ptr();
        apply_effect(DisplayMode::DmgGhosting, &mut frame, &mut prev);
        assert_eq!(prev.as_ref().unwrap().as_ptr(), ptr);
        assert_eq!(prev.as_ref().unwrap().len(), FRAME_BYTES);
    }

    #[test]
    fn soft_reuses_scratch_across_frames() {
        let mut frame = vec![40u8; FRAME_BYTES];
        let mut scratch = None;
        apply_effect(DisplayMode::Soft, &mut frame, &mut scratch);
        let ptr = scratch.as_ref().unwrap().as_ptr();
        frame.fill(80);
        apply_effect(DisplayMode::Soft, &mut frame, &mut scratch);
        assert_eq!(scratch.as_ref().unwrap().as_ptr(), ptr);
    }

    /// Before: Sharp cloned ~92 KiB every present (`frame.to_vec()`).
    /// After: Sharp clears scratch and allocates nothing.
    #[test]
    fn sharp_present_avoids_full_frame_clone_cost() {
        let mut frame = vec![7u8; FRAME_BYTES];
        let mut scratch = None;
        let n = 2_000usize;

        let t0 = Instant::now();
        for _ in 0..n {
            apply_effect(DisplayMode::Sharp, &mut frame, &mut scratch);
        }
        let sharp = t0.elapsed();

        let t1 = Instant::now();
        for _ in 0..n {
            let _waste = frame.to_vec();
            std::hint::black_box(_waste);
        }
        let clones = t1.elapsed();

        assert!(scratch.is_none());
        assert!(
            sharp * 2 < clones,
            "expected Sharp present path << full-frame clone; sharp={sharp:?} clones={clones:?}"
        );
    }

    /// Manual timing note for Stream G3 acceptance (not CI-gated).
    #[test]
    #[ignore = "manual host-path microbench — run with --ignored --nocapture"]
    fn stream_g3_present_effect_microbench() {
        let mut frame = vec![9u8; FRAME_BYTES];
        let mut scratch = None;
        let n = 10_000usize;

        let t_sharp = Instant::now();
        for _ in 0..n {
            apply_effect(DisplayMode::Sharp, &mut frame, &mut scratch);
        }
        let sharp = t_sharp.elapsed();

        scratch = Some(vec![0u8; FRAME_BYTES]);
        let t_ghost = Instant::now();
        for _ in 0..n {
            apply_effect(DisplayMode::DmgGhosting, &mut frame, &mut scratch);
        }
        let ghost = t_ghost.elapsed();

        let t_clone = Instant::now();
        for _ in 0..n {
            std::hint::black_box(frame.to_vec());
        }
        let clone = t_clone.elapsed();

        eprintln!(
            "G3 present effect ({n} iters, {FRAME_BYTES} bytes):\n\
             sharp={sharp:?}\n\
             ghost_reuse={ghost:?}\n\
             full_frame_to_vec={clone:?}\n\
             before: Sharp ≈ to_vec every present; after: Sharp ≪ to_vec"
        );
    }
}
