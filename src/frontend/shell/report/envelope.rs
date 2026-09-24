//! Local crash-report envelope (`crash-report.md` + `crash-meta.json`).

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const SCHEMA_VERSION: u32 = 1;
pub const META_FILE: &str = "crash-meta.json";
pub const REPORT_FILE: &str = "crash-report.md";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnvelopeKind {
    Panic,
    Fault,
    Bug,
    Feature,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrashMeta {
    pub schema_version: u32,
    pub created_at: u64,
    pub consumed: bool,
    /// User dismissed without filing; dump remains on disk.
    #[serde(default)]
    pub skipped: bool,
    pub kind: EnvelopeKind,
    #[serde(default)]
    pub title_hint: String,
}

impl CrashMeta {
    pub fn new(kind: EnvelopeKind, title_hint: impl Into<String>) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            created_at: unix_now(),
            consumed: false,
            skipped: false,
            kind,
            title_hint: title_hint.into(),
        }
    }

    pub fn pending(&self) -> bool {
        !self.consumed && !self.skipped
    }
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// App-data crash directory (`…/Graycart/crash/`), or exe-adjacent `graycart-crash/` fallback.
pub fn crash_dir() -> Option<PathBuf> {
    if let Some(mut dir) = dirs::data_dir() {
        dir.push("Graycart");
        dir.push("crash");
        return Some(dir);
    }
    std::env::current_exe().ok().and_then(|exe| {
        let mut dir = exe.parent()?.to_path_buf();
        dir.push("graycart-crash");
        Some(dir)
    })
}

pub fn write_envelope(dir: &Path, meta: &CrashMeta, markdown: &str) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let meta_bytes = serde_json::to_vec_pretty(meta).map_err(|e| e.to_string())?;
    fs::write(dir.join(META_FILE), meta_bytes).map_err(|e| e.to_string())?;
    fs::write(dir.join(REPORT_FILE), markdown).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn read_envelope(dir: &Path) -> Result<(CrashMeta, String), String> {
    let meta_bytes = fs::read(dir.join(META_FILE)).map_err(|e| e.to_string())?;
    let meta: CrashMeta = serde_json::from_slice(&meta_bytes).map_err(|e| e.to_string())?;
    let markdown = fs::read_to_string(dir.join(REPORT_FILE)).map_err(|e| e.to_string())?;
    Ok((meta, markdown))
}

/// Load a pending (unconsumed, unskipped) envelope if present.
pub fn load_pending(dir: &Path) -> Option<(CrashMeta, String)> {
    let (meta, md) = read_envelope(dir).ok()?;
    meta.pending().then_some((meta, md))
}

pub fn mark_consumed(dir: &Path) -> Result<(), String> {
    let (mut meta, md) = read_envelope(dir)?;
    meta.consumed = true;
    meta.skipped = false;
    write_envelope(dir, &meta, &md)
}

pub fn mark_skipped(dir: &Path) -> Result<(), String> {
    let (mut meta, md) = read_envelope(dir)?;
    meta.skipped = true;
    write_envelope(dir, &meta, &md)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn round_trip_and_pending_flags() {
        let dir = tempdir().unwrap();
        let meta = CrashMeta::new(EnvelopeKind::Panic, "boom");
        write_envelope(dir.path(), &meta, "# crash\n").unwrap();
        let (loaded, md) = load_pending(dir.path()).unwrap();
        assert!(loaded.pending());
        assert_eq!(loaded.kind, EnvelopeKind::Panic);
        assert_eq!(md, "# crash\n");

        mark_skipped(dir.path()).unwrap();
        assert!(load_pending(dir.path()).is_none());

        let (mut meta, md) = read_envelope(dir.path()).unwrap();
        meta.skipped = false;
        meta.consumed = false;
        write_envelope(dir.path(), &meta, &md).unwrap();
        mark_consumed(dir.path()).unwrap();
        assert!(load_pending(dir.path()).is_none());
    }
}
