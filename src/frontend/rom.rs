//! ROM path helpers + native file dialog.
//!
//! Cited: graycart-gb `src/frontend/rom` posture (extensions / dialog)
//!   https://github.com/graycart/graycart-gb/tree/main/src/frontend/rom
//! Note: P11 opens `.gba` / `.gb` / `.gbc` (native vs compat).

use std::path::{Path, PathBuf};

/// Which machine a path should load into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RomKind {
    /// Native GBA (`.gba`).
    Gba,
    /// DMG/CGB via compat (`.gb` / `.gbc`).
    GbCompat,
}

/// Classify a path by extension (case-insensitive).
#[must_use]
pub fn rom_kind(path: &Path) -> Option<RomKind> {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("gba") => Some(RomKind::Gba),
        Some("gb") | Some("gbc") => Some(RomKind::GbCompat),
        _ => None,
    }
}

/// True when `path` is a known cartridge the host will load.
#[must_use]
pub fn is_rom_path(path: &Path) -> bool {
    rom_kind(path).is_some()
}

/// Native open dialog filtered to `.gba` / `.gb` / `.gbc`. Returns `None` if cancelled.
pub fn open_rom_dialog() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter("Game Boy / Advance ROM", &["gba", "gb", "gbc"])
        .set_title("Open ROM")
        .pick_file()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn is_rom_path_accepts_gba_and_gb_family() {
        assert_eq!(rom_kind(Path::new("foo.gba")), Some(RomKind::Gba));
        assert_eq!(rom_kind(Path::new("FOO.GBA")), Some(RomKind::Gba));
        assert_eq!(rom_kind(Path::new("foo.gb")), Some(RomKind::GbCompat));
        assert_eq!(rom_kind(Path::new("foo.gbc")), Some(RomKind::GbCompat));
        assert!(is_rom_path(Path::new("foo.gb")));
        assert!(!is_rom_path(Path::new("foo.bin")));
        let _ = PathBuf::from("x.gba");
    }
}
