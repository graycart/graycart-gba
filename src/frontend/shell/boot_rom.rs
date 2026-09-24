//! Boot ROM cache beside the executable (`boot/dmg_boot.bin`, `boot/cgb_boot.bin`).

use graycart::hw::CGB_BOOT_ROM_SIZE;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

const BOOT_ROM_SIZE: usize = 256;
const BOOT_SUBDIR: &str = "boot";
const BOOT_ROM_FILENAME: &str = "dmg_boot.bin";
const BOOT_ROM_TMP: &str = "dmg_boot.bin.tmp";
const CGB_BOOT_ROM_FILENAME: &str = "cgb_boot.bin";
const CGB_BOOT_ROM_TMP: &str = "cgb_boot.bin.tmp";

pub fn boot_rom_cache_path(exe_dir: &Path) -> PathBuf {
    exe_dir.join(BOOT_SUBDIR).join(BOOT_ROM_FILENAME)
}

pub fn cgb_boot_rom_cache_path(exe_dir: &Path) -> PathBuf {
    exe_dir.join(BOOT_SUBDIR).join(CGB_BOOT_ROM_FILENAME)
}

pub fn load_cached_boot_rom(exe_dir: &Path) -> Option<[u8; 256]> {
    let path = boot_rom_cache_path(exe_dir);
    if fs::metadata(&path).ok()?.len() as usize != BOOT_ROM_SIZE {
        return None;
    }

    let mut file = fs::File::open(path).ok()?;
    let mut buf = [0u8; BOOT_ROM_SIZE];
    file.read_exact(&mut buf).ok()?;
    Some(buf)
}

pub fn load_cached_cgb_boot_rom(exe_dir: &Path) -> Option<Box<[u8; CGB_BOOT_ROM_SIZE]>> {
    let path = cgb_boot_rom_cache_path(exe_dir);
    if fs::metadata(&path).ok()?.len() as usize != CGB_BOOT_ROM_SIZE {
        return None;
    }

    let mut file = fs::File::open(path).ok()?;
    let mut buf = Box::new([0u8; CGB_BOOT_ROM_SIZE]);
    file.read_exact(&mut buf[..]).ok()?;
    Some(buf)
}

pub fn install_boot_rom(exe_dir: &Path, bytes: &[u8]) -> Result<(), String> {
    install_named(
        exe_dir,
        bytes,
        BOOT_ROM_SIZE,
        BOOT_ROM_FILENAME,
        BOOT_ROM_TMP,
    )
}

pub fn install_cgb_boot_rom(exe_dir: &Path, bytes: &[u8]) -> Result<(), String> {
    install_named(
        exe_dir,
        bytes,
        CGB_BOOT_ROM_SIZE,
        CGB_BOOT_ROM_FILENAME,
        CGB_BOOT_ROM_TMP,
    )
}

fn install_named(
    exe_dir: &Path,
    bytes: &[u8],
    expected_len: usize,
    filename: &str,
    tmp_name: &str,
) -> Result<(), String> {
    if bytes.len() != expected_len {
        return Err(format!(
            "boot ROM must be exactly {expected_len} bytes (got {})",
            bytes.len()
        ));
    }

    let path = exe_dir.join(BOOT_SUBDIR).join(filename);
    let boot_dir = path
        .parent()
        .ok_or_else(|| "invalid boot ROM cache path".to_string())?;
    fs::create_dir_all(boot_dir).map_err(|e| e.to_string())?;

    let tmp_path = boot_dir.join(tmp_name);
    fs::write(&tmp_path, bytes).map_err(|e| e.to_string())?;

    match fs::rename(&tmp_path, &path) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = fs::remove_file(&tmp_path);
            Err(e.to_string())
        }
    }
}

/// True when a correctly sized DMG or CGB boot image is present beside the exe.
///
/// Host-path only: checks file length via metadata (no full read). Callers that
/// need the bytes still use [`load_cached_boot_rom`] / [`load_cached_cgb_boot_rom`].
pub fn any_cached_boot_firmware(exe_dir: &Path) -> bool {
    cached_file_has_len(&boot_rom_cache_path(exe_dir), BOOT_ROM_SIZE)
        || cached_file_has_len(&cgb_boot_rom_cache_path(exe_dir), CGB_BOOT_ROM_SIZE)
}

fn cached_file_has_len(path: &Path, expected: usize) -> bool {
    fs::metadata(path)
        .ok()
        .is_some_and(|m| m.len() as usize == expected)
}

fn remove_if_present(path: &Path) -> Result<(), String> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

/// Remove both DMG and CGB cached firmware images.
pub fn clear_boot_rom(exe_dir: &Path) -> Result<(), String> {
    remove_if_present(&boot_rom_cache_path(exe_dir))?;
    remove_if_present(&cgb_boot_rom_cache_path(exe_dir))
}

#[cfg(test)]
mod tests;
