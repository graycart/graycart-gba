//! ROM path helpers + native file dialog.
//!
//! Cited: graycart-gb `src/frontend/rom` posture (extensions / dialog)
//!   https://github.com/graycart/graycart-gb/tree/main/src/frontend/rom
//! Note: P9 opens `.gba` only; `.gb`/`.gbc` wait for compat (P10+).

use std::path::{Path, PathBuf};

/// True when `path` looks like a GBA ROM the P9 host will load.
#[must_use]
pub fn is_rom_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("gba"))
}

/// Native open dialog filtered to `.gba`. Returns `None` if cancelled / unavailable.
pub fn open_rom_dialog() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter("Game Boy Advance ROM", &["gba"])
        .set_title("Open GBA ROM")
        .pick_file()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn is_rom_path_accepts_gba_only() {
        assert!(is_rom_path(Path::new("foo.gba")));
        assert!(is_rom_path(Path::new("FOO.GBA")));
        assert!(!is_rom_path(Path::new("foo.gb")));
        assert!(!is_rom_path(Path::new("foo.gbc")));
        assert!(!is_rom_path(Path::new("foo.bin")));
        let _ = PathBuf::from("x.gba");
    }
}
