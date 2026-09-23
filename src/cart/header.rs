//! ROM header and save-kind detection.
//!
//! Cited: GBATEK Cartridges.
//! <https://problemkaputt.de/gbatek.htm>

use std::path::{Path, PathBuf};

/// Minimum cartridge header size (0xC0).
const HEADER_MIN: usize = 0xC0;

/// Save type inferred from Nintendo's ASCII ID strings in the ROM.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveKind {
    None,
    Sram,
    Flash64,
    Flash128,
    Eeprom,
}

impl SaveKind {
    pub fn name(self) -> &'static str {
        match self {
            SaveKind::None => "none",
            SaveKind::Sram => "sram",
            SaveKind::Flash64 => "flash64",
            SaveKind::Flash128 => "flash128",
            SaveKind::Eeprom => "eeprom",
        }
    }
}

/// Reject a cart image shorter than 192 bytes. Does not panic.
pub fn parse_header(rom: &[u8]) -> Result<(), String> {
    if rom.len() < HEADER_MIN {
        return Err(format!(
            "cart header truncated: need {HEADER_MIN} bytes, got {}",
            rom.len()
        ));
    }
    Ok(())
}

/// Scan ROM for Nintendo save-ID ASCII strings. Longer matches win.
///
/// EEPROM 512 vs 8 KiB is not decided here.
pub fn detect_save(rom: &[u8]) -> SaveKind {
    const CANDIDATES: &[(&[u8], SaveKind)] = &[
        (b"FLASH1M_V", SaveKind::Flash128),
        (b"FLASH512_V", SaveKind::Flash64),
        (b"FLASH_V", SaveKind::Flash64),
        (b"SRAM_V", SaveKind::Sram),
        (b"EEPROM_V", SaveKind::Eeprom),
    ];

    let mut best: Option<(usize, SaveKind)> = None;
    for &(pat, kind) in CANDIDATES {
        if contains_bytes(rom, pat) {
            let len = pat.len();
            if best.is_none_or(|(best_len, _)| len > best_len) {
                best = Some((len, kind));
            }
        }
    }
    best.map(|(_, k)| k).unwrap_or(SaveKind::None)
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

/// Exact warn line for unsupported cart GPIO (RTC, rumble, tilt, solar).
pub fn gpio_reject_line() -> &'static str {
    "gba-debug: warn cart gpio unsupported"
}

/// Call when a cart marked as having GPIO is touched at 0x080000C4/C6/C8.
///
/// `has_gpio` is an input: do not treat those addresses as GPIO on a normal ROM.
pub fn touch_gpio(has_gpio: bool) -> Option<&'static str> {
    if has_gpio {
        Some(gpio_reject_line())
    } else {
        None
    }
}

/// ROM path with extension replaced by `sav` (`game.gba` → `game.sav`).
pub fn sidecar_path(rom: &Path) -> PathBuf {
    rom.with_extension("sav")
}

/// Load the save sidecar. Missing file → empty `Ok`. Other I/O errors propagate
/// so a caller does not treat a permission failure as an erased chip and flush
/// over a good `.sav`.
pub fn read_sidecar(path: &Path) -> Result<Vec<u8>, String> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(bytes),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(format!("save sidecar {}: {e}", path.display())),
    }
}

/// Write save-sidecar bytes.
pub fn write_sidecar(path: &Path, bytes: &[u8]) -> Result<(), String> {
    std::fs::write(path, bytes).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn truncated_under_192() {
        let rom = [0u8; 100];
        let err = parse_header(&rom).unwrap_err();
        assert!(
            err.contains("truncated"),
            "error should mention truncated, got: {err}"
        );
    }

    #[test]
    fn header_192_zeros_ok_save_none() {
        let rom = [0u8; 192];
        assert!(parse_header(&rom).is_ok());
        assert_eq!(detect_save(&rom), SaveKind::None);
    }

    #[test]
    fn save_flash1m_is_flash128() {
        let mut rom = vec![0u8; 192];
        rom.extend_from_slice(b"FLASH1M_V123");
        assert_eq!(detect_save(&rom), SaveKind::Flash128);
    }

    #[test]
    fn save_flash512_is_flash64() {
        let mut rom = vec![0u8; 192];
        rom.extend_from_slice(b"FLASH512_V");
        assert_eq!(detect_save(&rom), SaveKind::Flash64);
    }

    #[test]
    fn save_sram() {
        let mut rom = vec![0u8; 192];
        rom.extend_from_slice(b"SRAM_V");
        assert_eq!(detect_save(&rom), SaveKind::Sram);
    }

    #[test]
    fn save_eeprom() {
        let mut rom = vec![0u8; 192];
        rom.extend_from_slice(b"EEPROM_V");
        assert_eq!(detect_save(&rom), SaveKind::Eeprom);
    }

    #[test]
    fn flash1m_wins_over_flash64_substring() {
        // Longer match: FLASH1M_V must not be classified as Flash64 via FLASH_V.
        let mut rom = vec![0u8; 192];
        rom.extend_from_slice(b"....FLASH1M_V....");
        assert_eq!(detect_save(&rom), SaveKind::Flash128);
        assert_ne!(detect_save(&rom), SaveKind::Flash64);
    }

    #[test]
    fn gpio_line_exact() {
        assert_eq!(gpio_reject_line(), "gba-debug: warn cart gpio unsupported");
    }

    #[test]
    fn touch_gpio_respects_flag() {
        assert_eq!(touch_gpio(true), Some(gpio_reject_line()));
        assert_eq!(touch_gpio(false), None);
    }

    #[test]
    fn sidecar_round_trip_in_temp() {
        let dir =
            std::env::temp_dir().join(format!("graycart-gba-header-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let rom = dir.join("game.gba");
        fs::write(&rom, b"fake-rom").unwrap();

        let sav = sidecar_path(&rom);
        assert!(
            sav.to_string_lossy().ends_with(".sav"),
            "sidecar_path should end in .sav: {}",
            sav.display()
        );

        let payload = b"save-bytes-xyz";
        write_sidecar(&sav, payload).unwrap();
        assert_eq!(read_sidecar(&sav).unwrap(), payload);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_sidecar_missing_is_empty() {
        let dir = std::env::temp_dir().join(format!(
            "graycart-gba-header-missing-{}",
            std::process::id()
        ));
        let _ = fs::create_dir_all(&dir);
        let missing = dir.join("nope.sav");
        let _ = fs::remove_file(&missing);
        assert!(read_sidecar(&missing).unwrap().is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_kind_names() {
        assert_eq!(SaveKind::None.name(), "none");
        assert_eq!(SaveKind::Sram.name(), "sram");
        assert_eq!(SaveKind::Flash64.name(), "flash64");
        assert_eq!(SaveKind::Flash128.name(), "flash128");
        assert_eq!(SaveKind::Eeprom.name(), "eeprom");
    }
}
