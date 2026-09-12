//! Presentment / IO bridge — P10 **G10-bridge**.
//!
//! Cited: GBATEK — Keypad (L/R stretch in 8-bit mode, not GB keys)
//!   https://problemkaputt.de/gbatek-gba-keypad-input.htm
//! Cited: graycart-gba `09-dmg-cgb-compatibility.md` §5–7
//!   Project store: `docs/graycart-gba/09-dmg-cgb-compatibility.md`
//! Note: L/R must **not** feed `FF00`; stretch is a display option.

use crate::input::button as gba_btn;
use graycart::{GameBoyButton, Shade, SCREEN_HEIGHT, SCREEN_WIDTH};

/// Hardware L/R toggle: letterboxed 160×144 vs horizontal stretch to 240×144.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StretchMode {
    /// 160×144 letterboxed inside 240×160 (default HW-ish).
    #[default]
    Letterbox160,
    /// 240×144 horizontal stretch (HW L/R toggle).
    Stretch240,
}

impl StretchMode {
    /// Toggle as real AGB L/R would.
    #[must_use]
    pub const fn toggled(self) -> Self {
        match self {
            Self::Letterbox160 => Self::Stretch240,
            Self::Stretch240 => Self::Letterbox160,
        }
    }

    /// Present width for the GB panel inside the AGB LCD.
    #[must_use]
    pub const fn present_width(self) -> usize {
        match self {
            Self::Letterbox160 => SCREEN_WIDTH,
            Self::Stretch240 => 240,
        }
    }
}

/// Map a GBA KEYINPUT-style pressed mask → Game Boy buttons.
///
/// **Drops L and R** — those toggle [`StretchMode`], they are not `FF00` inputs.
#[must_use]
pub fn gba_mask_to_gb_buttons(mask: u16) -> (Vec<GameBoyButton>, bool) {
    let mut out = Vec::new();
    if mask & gba_btn::A != 0 {
        out.push(GameBoyButton::A);
    }
    if mask & gba_btn::B != 0 {
        out.push(GameBoyButton::B);
    }
    if mask & gba_btn::SELECT != 0 {
        out.push(GameBoyButton::Select);
    }
    if mask & gba_btn::START != 0 {
        out.push(GameBoyButton::Start);
    }
    if mask & gba_btn::RIGHT != 0 {
        out.push(GameBoyButton::Right);
    }
    if mask & gba_btn::LEFT != 0 {
        out.push(GameBoyButton::Left);
    }
    if mask & gba_btn::UP != 0 {
        out.push(GameBoyButton::Up);
    }
    if mask & gba_btn::DOWN != 0 {
        out.push(GameBoyButton::Down);
    }
    let lr_edge = (mask & (gba_btn::L | gba_btn::R)) != 0;
    (out, lr_edge)
}

/// Classic DMG shade → rough sRGB888 (host presentment; not core).
#[must_use]
pub fn shade_to_rgb888(shade: Shade) -> [u8; 3] {
    match shade {
        Shade::Lightest => [0xE0, 0xF8, 0xD0],
        Shade::Light => [0x88, 0xC0, 0x70],
        Shade::Dark => [0x34, 0x68, 0x56],
        Shade::Darkest => [0x08, 0x18, 0x20],
    }
}

/// Expand 160×144 shades to RGB888 tightly packed (row-major).
#[must_use]
pub fn framebuffer_rgb888(shades: &[Shade]) -> Vec<u8> {
    let mut out = Vec::with_capacity(SCREEN_WIDTH * SCREEN_HEIGHT * 3);
    for &s in shades.iter().take(SCREEN_WIDTH * SCREEN_HEIGHT) {
        let rgb = shade_to_rgb888(s);
        out.extend_from_slice(&rgb);
    }
    out
}

/// Apply L/R stretch preference when the host saw an L/R press edge.
#[must_use]
pub fn apply_lr_stretch(current: StretchMode, lr_pressed: bool) -> StretchMode {
    if lr_pressed {
        current.toggled()
    } else {
        current
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lr_not_mapped_to_gb_buttons() {
        let mask = gba_btn::L | gba_btn::R | gba_btn::A;
        let (btns, lr) = gba_mask_to_gb_buttons(mask);
        assert!(lr);
        assert_eq!(btns, vec![GameBoyButton::A]);
        assert!(!btns.contains(&GameBoyButton::Left));
    }

    #[test]
    fn dpad_and_face_map() {
        let mask = gba_btn::UP | gba_btn::B | gba_btn::START;
        let (btns, lr) = gba_mask_to_gb_buttons(mask);
        assert!(!lr);
        assert!(btns.contains(&GameBoyButton::Up));
        assert!(btns.contains(&GameBoyButton::B));
        assert!(btns.contains(&GameBoyButton::Start));
    }

    #[test]
    fn stretch_toggle() {
        assert_eq!(
            apply_lr_stretch(StretchMode::Letterbox160, true),
            StretchMode::Stretch240
        );
        assert_eq!(StretchMode::Stretch240.present_width(), 240);
        assert_eq!(StretchMode::Letterbox160.present_width(), 160);
    }

    #[test]
    fn rgb_expand_size() {
        let shades = vec![Shade::Lightest; SCREEN_WIDTH * SCREEN_HEIGHT];
        let rgb = framebuffer_rgb888(&shades);
        assert_eq!(rgb.len(), SCREEN_WIDTH * SCREEN_HEIGHT * 3);
        assert_eq!(&rgb[..3], &[0xE0, 0xF8, 0xD0]);
    }
}
