//! Optional BiosLle image load (user-supplied; never vendored).
//!
//! Cited: GBATEK — BIOS System ROM 16 KiB
//!   https://problemkaputt.de/gbatek.htm
//! Research: Project store `docs/graycart-gba/06-cart-bios-saves.md` §3.4
//! Note: publish hashes only — never commit `gba_bios.bin`.

use std::path::{Path, PathBuf};

/// Expected retail BIOS size.
pub const BIOS_SIZE: usize = 16 * 1024;

/// Env var for optional BIOS path (CI must not require it).
pub const GBA_BIOS_ENV: &str = "GBA_BIOS";

/// Resolve a BIOS path from `GBA_BIOS` or `None`.
#[must_use]
pub fn bios_path_from_env() -> Option<PathBuf> {
    std::env::var_os(GBA_BIOS_ENV).map(PathBuf::from)
}

/// Load a 16 KiB BIOS image from `path`.
///
/// Returns `Err` with a clear message on missing/short/oversize files — never panics.
pub fn load_bios_file(path: &Path) -> Result<Vec<u8>, String> {
    let bytes = std::fs::read(path).map_err(|e| {
        format!(
            "failed to read BIOS at {}: {e} (set {GBA_BIOS_ENV} to your dump; never commit BIOS)",
            path.display()
        )
    })?;
    validate_bios_bytes(&bytes)
}

/// Validate length; returns the image or a clear error.
pub fn validate_bios_bytes(bytes: &[u8]) -> Result<Vec<u8>, String> {
    if bytes.len() != BIOS_SIZE {
        return Err(format!(
            "BIOS must be {BIOS_SIZE} bytes, got {} (refuse truncated/padded dumps)",
            bytes.len()
        ));
    }
    Ok(bytes.to_vec())
}

/// SHA-256 hex of a BIOS image (for docs / user verify — hashes only in-tree).
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    crate::ppu::hash::sha256_hex(bytes)
}
