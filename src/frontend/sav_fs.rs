//! Battery `.sav` path helpers (sidecar next to the ROM).
//!
//! Cited: graycart-gb save sidecar posture
//!   https://github.com/graycart/graycart-gb (default_save_path / flush_save)
//! Cited: GBATEK cart backup sizes (raw dump, no footer)
//!   https://problemkaputt.de/gbatek.htm
//! Note: host owns paths; core owns raw bytes via `Gba::battery_sav`.

use std::fs;
use std::path::{Path, PathBuf};

/// `<rom>.sav` beside the ROM (same stem).
#[must_use]
pub fn default_save_path(rom: &Path) -> PathBuf {
    rom.with_extension("sav")
}

/// Load `.sav` bytes if the file exists; `Ok(None)` when absent.
pub fn load_save(path: &Path) -> Result<Option<Vec<u8>>, String> {
    if !path.is_file() {
        return Ok(None);
    }
    fs::read(path)
        .map(Some)
        .map_err(|e| format!("read {}: {e}", path.display()))
}

/// Write raw `.sav` bytes (creates/overwrites).
pub fn flush_save(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if bytes.is_empty() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
        }
    }
    fs::write(path, bytes).map_err(|e| format!("write {}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn default_save_path_replaces_extension() {
        assert_eq!(
            default_save_path(Path::new("/tmp/game.gba")),
            PathBuf::from("/tmp/game.sav")
        );
    }

    #[test]
    fn flush_and_load_roundtrip() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("graycart-gba-p9-{nanos}.sav"));
        let data = vec![1u8, 2, 3, 4];
        flush_save(&path, &data).expect("flush");
        let loaded = load_save(&path).expect("load").expect("present");
        assert_eq!(loaded, data);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn load_missing_is_none() {
        let path = Path::new("/tmp/graycart-gba-p9-does-not-exist-please.sav");
        assert!(load_save(path).unwrap().is_none());
    }
}
