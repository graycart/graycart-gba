//! Game Pak ROM header parse (title / code / complement).
//!
//! Cited: GBATEK — GBA Cartridge Header
//!   https://problemkaputt.de/gbatek.htm#gbacartridgeheader
//! Research: Project store `docs/graycart-gba/06-cart-bios-saves.md` §4
//! Note: BIOS still validates logo/checksum on LLE boot; this is for UI/detect.

/// Minimum bytes needed to read the fixed header fields through `0BEh`.
pub const HEADER_MIN_LEN: usize = 0xC0;

/// Parsed cartridge header fields (no logo blob retained).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CartHeader {
    /// ARM entry word at `000h` (usually `B rom_start`).
    pub entry: u32,
    /// Game title ASCII at `0A0h` (12 bytes, NUL-padded).
    pub title: [u8; 12],
    /// Game code at `0ACh` (4 bytes, e.g. `UTTD`).
    pub game_code: [u8; 4],
    /// Maker code at `0B0h` (2 bytes).
    pub maker_code: [u8; 2],
    /// Fixed value at `0B2h` (must be `0x96` on retail).
    pub fixed_96: u8,
    /// Main unit code at `0B3h`.
    pub unit_code: u8,
    /// Device type at `0B4h`.
    pub device_type: u8,
    /// Software version at `0BCh`.
    pub version: u8,
    /// Complement check byte at `0BDh`.
    pub complement: u8,
}

impl CartHeader {
    /// Parse header from a ROM image. Returns `None` if shorter than [`HEADER_MIN_LEN`].
    #[must_use]
    pub fn parse(rom: &[u8]) -> Option<Self> {
        if rom.len() < HEADER_MIN_LEN {
            return None;
        }
        let entry = u32::from_le_bytes([rom[0], rom[1], rom[2], rom[3]]);
        let mut title = [0u8; 12];
        title.copy_from_slice(&rom[0xA0..0xAC]);
        let mut game_code = [0u8; 4];
        game_code.copy_from_slice(&rom[0xAC..0xB0]);
        let mut maker_code = [0u8; 2];
        maker_code.copy_from_slice(&rom[0xB0..0xB2]);
        Some(Self {
            entry,
            title,
            game_code,
            maker_code,
            fixed_96: rom[0xB2],
            unit_code: rom[0xB3],
            device_type: rom[0xB4],
            version: rom[0xBC],
            complement: rom[0xBD],
        })
    }

    /// GBATEK complement over `0A0h..=0BCh`:  
    /// `chk = 0; for b in bytes { chk = chk - b }; chk = (chk - 0x19) & 0xFF`.
    #[must_use]
    pub fn compute_complement(rom: &[u8]) -> Option<u8> {
        if rom.len() < 0xBD {
            return None;
        }
        let mut chk: u8 = 0;
        for &b in &rom[0xA0..=0xBC] {
            chk = chk.wrapping_sub(b);
        }
        Some(chk.wrapping_sub(0x19))
    }

    /// True when fixed `0x96` matches and complement byte is correct.
    #[must_use]
    pub fn checksum_ok(&self, rom: &[u8]) -> bool {
        self.fixed_96 == 0x96 && Self::compute_complement(rom).is_some_and(|c| c == self.complement)
    }

    /// Title as lossy UTF-8 trimmed of NULs / spaces.
    #[must_use]
    pub fn title_str(&self) -> String {
        let s = String::from_utf8_lossy(&self.title);
        s.trim_matches(|c: char| c == '\0' || c == ' ').to_string()
    }

    /// Game code as ASCII (lossy).
    #[must_use]
    pub fn game_code_str(&self) -> String {
        String::from_utf8_lossy(&self.game_code).into_owned()
    }
}
