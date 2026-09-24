//! Frontend-only shade → RGB themes. DMG/compat present uses [`Shade`]; NativeCgb uses RGB555.

use graycart::{Framebuffer, SCREEN_HEIGHT, SCREEN_WIDTH, Shade};
use serde::{Deserialize, Serialize};

/// Named palette themes (presentation only).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PalettePreset {
    #[default]
    #[serde(alias = "original_dmg")]
    ClassicDmg,
    LightDmg,
    PocketGray,
    Graphite,
    IceBlue,
    Cobalt,
    GameBoyLight,
    Amber,
    Sepia,
    TerminalGreen,
    Monochrome,
    Lavender,
    Rose,
    Cyberpunk,
    Vaporwave,
    Sunset,
    Arctic,
    Midnight,
    Atomic,
    HighContrast,
    Custom,
}

impl PalettePreset {
    pub const ALL: [Self; 21] = [
        Self::ClassicDmg,
        Self::LightDmg,
        Self::PocketGray,
        Self::Graphite,
        Self::IceBlue,
        Self::Cobalt,
        Self::GameBoyLight,
        Self::Amber,
        Self::Sepia,
        Self::TerminalGreen,
        Self::Monochrome,
        Self::Lavender,
        Self::Rose,
        Self::Cyberpunk,
        Self::Vaporwave,
        Self::Sunset,
        Self::Arctic,
        Self::Midnight,
        Self::Atomic,
        Self::HighContrast,
        Self::Custom,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::ClassicDmg => "Classic DMG",
            Self::LightDmg => "Light DMG",
            Self::PocketGray => "Pocket Gray",
            Self::Graphite => "Graphite",
            Self::IceBlue => "Ice Blue",
            Self::Cobalt => "Cobalt",
            Self::GameBoyLight => "Game Boy Light",
            Self::Amber => "Amber",
            Self::Sepia => "Sepia",
            Self::TerminalGreen => "Terminal Green",
            Self::Monochrome => "Monochrome",
            Self::Lavender => "Lavender",
            Self::Rose => "Rose",
            Self::Cyberpunk => "Cyberpunk",
            Self::Vaporwave => "Vaporwave",
            Self::Sunset => "Sunset",
            Self::Arctic => "Arctic",
            Self::Midnight => "Midnight",
            Self::Atomic => "Atomic",
            Self::HighContrast => "High Contrast",
            Self::Custom => "Custom",
        }
    }

    pub fn palette(self) -> Palette {
        match self {
            Self::ClassicDmg | Self::Custom => Palette::from_rgb([
                [0x9B, 0xBC, 0x0F],
                [0x8B, 0xAC, 0x0F],
                [0x30, 0x62, 0x30],
                [0x0F, 0x38, 0x0F],
            ]),
            Self::LightDmg => Palette::from_rgb([
                [0xE0, 0xF8, 0xD0],
                [0x88, 0xC0, 0x70],
                [0x34, 0x68, 0x56],
                [0x08, 0x18, 0x20],
            ]),
            Self::PocketGray => Palette::from_rgb([
                [0xFF, 0xFF, 0xFF],
                [0xAA, 0xAA, 0xAA],
                [0x55, 0x55, 0x55],
                [0x00, 0x00, 0x00],
            ]),
            Self::Graphite => Palette::from_rgb([
                [0xE8, 0xE8, 0xE8],
                [0xA0, 0xA0, 0xA8],
                [0x50, 0x50, 0x58],
                [0x18, 0x18, 0x1C],
            ]),
            Self::IceBlue => Palette::from_rgb([
                [0xE8, 0xF8, 0xFF],
                [0x90, 0xD0, 0xF0],
                [0x38, 0x70, 0xB0],
                [0x10, 0x20, 0x48],
            ]),
            Self::Cobalt => Palette::from_rgb([
                [0xD0, 0xE8, 0xFF],
                [0x58, 0x90, 0xD8],
                [0x20, 0x48, 0x98],
                [0x08, 0x10, 0x30],
            ]),
            Self::GameBoyLight => Palette::from_rgb([
                [0xF8, 0xF8, 0xB0],
                [0xB0, 0xD0, 0x60],
                [0x50, 0x90, 0x40],
                [0x18, 0x38, 0x18],
            ]),
            Self::Amber => Palette::from_rgb([
                [0xFC, 0xF4, 0xD0],
                [0xE0, 0xA8, 0x48],
                [0xA0, 0x58, 0x18],
                [0x40, 0x18, 0x08],
            ]),
            Self::Sepia => Palette::from_rgb([
                [0xF4, 0xE4, 0xC8],
                [0xC8, 0xA0, 0x70],
                [0x78, 0x50, 0x38],
                [0x28, 0x18, 0x10],
            ]),
            Self::TerminalGreen => Palette::from_rgb([
                [0xB0, 0xFF, 0xB0],
                [0x40, 0xD0, 0x40],
                [0x10, 0x80, 0x10],
                [0x00, 0x20, 0x00],
            ]),
            Self::Monochrome => Palette::from_rgb([
                [0xFF, 0xFF, 0xFF],
                [0xC0, 0xC0, 0xC0],
                [0x60, 0x60, 0x60],
                [0x00, 0x00, 0x00],
            ]),
            Self::Lavender => Palette::from_rgb([
                [0xF0, 0xE8, 0xFF],
                [0xC0, 0xA8, 0xE8],
                [0x70, 0x50, 0xA8],
                [0x28, 0x18, 0x40],
            ]),
            Self::Rose => Palette::from_rgb([
                [0xFF, 0xE8, 0xF0],
                [0xF0, 0xA0, 0xB8],
                [0xA0, 0x40, 0x60],
                [0x38, 0x10, 0x20],
            ]),
            Self::Cyberpunk => Palette::from_rgb([
                [0xE0, 0xFF, 0xF8],
                [0x00, 0xE8, 0xC0],
                [0xE0, 0x20, 0x80],
                [0x18, 0x08, 0x28],
            ]),
            Self::Vaporwave => Palette::from_rgb([
                [0xFF, 0xE0, 0xF8],
                [0x80, 0xE0, 0xFF],
                [0xE0, 0x60, 0xC0],
                [0x30, 0x18, 0x50],
            ]),
            Self::Sunset => Palette::from_rgb([
                [0xFF, 0xF0, 0xC8],
                [0xFF, 0xA0, 0x50],
                [0xD0, 0x40, 0x50],
                [0x30, 0x10, 0x28],
            ]),
            Self::Arctic => Palette::from_rgb([
                [0xF0, 0xF8, 0xFF],
                [0xB0, 0xD0, 0xE8],
                [0x50, 0x78, 0xA0],
                [0x10, 0x18, 0x30],
            ]),
            Self::Midnight => Palette::from_rgb([
                [0xA0, 0xB0, 0xD0],
                [0x50, 0x60, 0x90],
                [0x28, 0x30, 0x58],
                [0x08, 0x08, 0x18],
            ]),
            Self::Atomic => Palette::from_rgb([
                [0xF0, 0xFF, 0xC0],
                [0xA0, 0xE0, 0x30],
                [0x30, 0x90, 0x20],
                [0x08, 0x20, 0x08],
            ]),
            Self::HighContrast => Palette::from_rgb([
                [0xFF, 0xFF, 0xFF],
                [0xFF, 0xFF, 0xFF],
                [0x00, 0x00, 0x00],
                [0x00, 0x00, 0x00],
            ]),
        }
    }
}

/// Four RGB triples for Shade0..=Shade3 (lightest → darkest).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    colors: [[u8; 3]; 4],
}

impl Palette {
    pub fn from_rgb(colors: [[u8; 3]; 4]) -> Self {
        Self { colors }
    }

    pub fn colors(self) -> [[u8; 3]; 4] {
        self.colors
    }

    pub fn shade_to_rgba(self, shade: Shade) -> [u8; 4] {
        let rgb = self.colors[shade.index() as usize];
        [rgb[0], rgb[1], rgb[2], 0xFF]
    }
}

/// Fill `out` (RGBA8888) from a framebuffer using `palette`.
pub fn framebuffer_to_rgba(fb: &Framebuffer, palette: Palette, out: &mut [u8]) {
    debug_assert_eq!(out.len(), SCREEN_WIDTH * SCREEN_HEIGHT * 4);
    for (dst, &shade) in out.as_chunks_mut::<4>().0.iter_mut().zip(fb.pixels()) {
        dst.copy_from_slice(&palette.shade_to_rgba(shade));
    }
}

/// Solid fill used for the empty “No ROM” presenter.
pub fn fill_rgba(out: &mut [u8], rgba: [u8; 4]) {
    for dst in out.as_chunks_mut::<4>().0.iter_mut() {
        dst.copy_from_slice(&rgba);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shade_conversion_is_stable() {
        let pal = PalettePreset::ClassicDmg.palette();
        assert_ne!(
            pal.shade_to_rgba(Shade::Lightest),
            pal.shade_to_rgba(Shade::Darkest)
        );
        let fb = Framebuffer::new();
        let mut buf = vec![0u8; SCREEN_WIDTH * SCREEN_HEIGHT * 4];
        framebuffer_to_rgba(&fb, pal, &mut buf);
        assert_eq!(&buf[0..4], &pal.shade_to_rgba(Shade::Lightest));
    }

    #[test]
    fn presets_cover_named_themes() {
        assert_eq!(PalettePreset::ALL.len(), 21);
        assert_ne!(
            PalettePreset::ClassicDmg
                .palette()
                .shade_to_rgba(Shade::Lightest),
            PalettePreset::Cyberpunk
                .palette()
                .shade_to_rgba(Shade::Lightest)
        );
    }
}
