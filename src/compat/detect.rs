//! Cart class / machine profile detect — P10 **G10-detect**.
//!
//! Cited: GBATEK — Backwards Compatibility CGB Mode / WAITCNT bit 15
//!   https://problemkaputt.de/gbatek-gba-backwards-compatibility-cgb-mode.htm
//! Cited: graycart-gba `09-dmg-cgb-compatibility.md` §2
//!   Project store: `docs/graycart-gba/09-dmg-cgb-compatibility.md`
//! Note: SoC profile comes from WAITCNT.bit15 / explicit load path — **not**
//! cartridge header `$0143` alone (that unlocks DMG vs CGB features *inside* compat).

use crate::bus::waitcnt::WaitCnt;

/// Hardware cart pin class mirrored by WAITCNT bit 15.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CartClass {
    /// WAITCNT.15 = 0 — GBA Game Pak.
    GbaRom,
    /// WAITCNT.15 = 1 — DMG/CGB cartridge in the slot.
    GbCompat,
}

/// Which machine the product should run after detect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachineProfile {
    /// Native ARM7 + GBA SoC.
    NativeGba,
    /// SM83 path via reused graycart behind the HW wrapper.
    CompatGb,
}

/// How the host/load path asked to enter a machine (explicit > WAITCNT).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadPathHint {
    /// Extension / CLI / API said `.gba` (or native bytes).
    GbaRom,
    /// Extension / CLI / API said `.gb` / `.gbc`.
    GbRom,
    /// No hint — fall back to WAITCNT cart-type bit.
    FromWaitCnt,
}

impl CartClass {
    /// Read WAITCNT bit 15 (GBATEK: 0 = GBA, 1 = CGB/DMG cart).
    #[must_use]
    pub const fn from_waitcnt(wc: WaitCnt) -> Self {
        if wc.cart_type_cgb() {
            Self::GbCompat
        } else {
            Self::GbaRom
        }
    }

    /// Map class → machine profile.
    #[must_use]
    pub const fn profile(self) -> MachineProfile {
        match self {
            Self::GbaRom => MachineProfile::NativeGba,
            Self::GbCompat => MachineProfile::CompatGb,
        }
    }
}

/// Select machine profile: explicit load path wins; else WAITCNT bit 15.
///
/// Header `$0143` is intentionally **not** consulted here.
#[must_use]
pub fn select_profile(hint: LoadPathHint, waitcnt: WaitCnt) -> MachineProfile {
    match hint {
        LoadPathHint::GbaRom => MachineProfile::NativeGba,
        LoadPathHint::GbRom => MachineProfile::CompatGb,
        LoadPathHint::FromWaitCnt => CartClass::from_waitcnt(waitcnt).profile(),
    }
}

/// Infer a load-path hint from a file extension (host convenience only).
#[must_use]
pub fn hint_from_extension(ext: Option<&str>) -> LoadPathHint {
    match ext.map(|e| e.to_ascii_lowercase()).as_deref() {
        Some("gba") => LoadPathHint::GbaRom,
        Some("gb") | Some("gbc") => LoadPathHint::GbRom,
        _ => LoadPathHint::FromWaitCnt,
    }
}

/// CGB flag byte at header `$0143` — **feature** unlock inside compat, not SoC select.
#[must_use]
pub fn header_cgb_byte(rom: &[u8]) -> Option<u8> {
    rom.get(0x0143).copied()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::waitcnt::WaitCnt;

    #[test]
    fn waitcnt_bit15_selects_compat() {
        let mut wc = WaitCnt::power_on();
        assert_eq!(CartClass::from_waitcnt(wc), CartClass::GbaRom);
        assert_eq!(
            select_profile(LoadPathHint::FromWaitCnt, wc),
            MachineProfile::NativeGba
        );

        wc.set_cart_type_cgb(true);
        assert_eq!(CartClass::from_waitcnt(wc), CartClass::GbCompat);
        assert_eq!(
            select_profile(LoadPathHint::FromWaitCnt, wc),
            MachineProfile::CompatGb
        );
    }

    #[test]
    fn explicit_gb_path_overrides_gba_waitcnt() {
        let wc = WaitCnt::power_on(); // bit15 clear
        assert_eq!(
            select_profile(LoadPathHint::GbRom, wc),
            MachineProfile::CompatGb
        );
    }

    #[test]
    fn explicit_gba_path_overrides_cgb_waitcnt() {
        let mut wc = WaitCnt::power_on();
        wc.set_cart_type_cgb(true);
        assert_eq!(
            select_profile(LoadPathHint::GbaRom, wc),
            MachineProfile::NativeGba
        );
    }

    #[test]
    fn extension_hints() {
        assert_eq!(hint_from_extension(Some("GBA")), LoadPathHint::GbaRom);
        assert_eq!(hint_from_extension(Some("gbc")), LoadPathHint::GbRom);
        assert_eq!(hint_from_extension(Some("gb")), LoadPathHint::GbRom);
        assert_eq!(hint_from_extension(Some("bin")), LoadPathHint::FromWaitCnt);
    }

    #[test]
    fn header_cgb_byte_is_not_soc_selector() {
        // Document: $0143 alone must not drive select_profile.
        let mut rom = vec![0u8; 0x150];
        rom[0x0143] = 0xC0; // CGB-only flag style
        assert_eq!(header_cgb_byte(&rom), Some(0xC0));
        let wc = WaitCnt::power_on();
        assert_eq!(
            select_profile(LoadPathHint::FromWaitCnt, wc),
            MachineProfile::NativeGba,
            "header must not flip SoC profile without WAITCNT/load hint"
        );
    }
}
