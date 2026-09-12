//! Compat Mode-8 / HALTCNT handoff + CGB-AGB boot ROM slot — P10 **G10-boot**.
//!
//! Cited: GBATEK — Backwards Compatibility CGB Mode / DISPCNT bit 3 / HALTCNT
//!   https://problemkaputt.de/gbatek-gba-backwards-compatibility-cgb-mode.htm
//! Cited: Pan Docs — Power Up Sequence / CGB Registers (AGB `B` bit 0)
//!   https://gbdev.io/pandocs/Power_Up_Sequence.html
//! Cited: graycart-gba `09-dmg-cgb-compatibility.md` §2–3, §8
//!   Project store: `docs/graycart-gba/09-dmg-cgb-compatibility.md`
//! Note: CGB-AGB boot ROM is **user-supplied** (same legal posture as `gba_bios.bin`).
//! Fast/HLE post-boot is OK for P10 smoke; LLE requires firmware bytes.

/// DISPCNT bit 3 — “CGB mode” / informal Mode-8 arm (BIOS-writable on HW).
pub const DISPCNT_CGB_MODE_BIT: u16 = 1 << 3;

/// How the SM83 machine reaches cart `$0100`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CompatBootMode {
    /// Post-boot CPU/MMIO snapshot via graycart `apply_fast` (CI / smoke default).
    #[default]
    FastHle,
    /// Real CGB-AGB boot ROM overlay — requires [`AgbBootFirmware`] bytes.
    BootRomLle,
}

/// User-supplied CGB-AGB / AGB0 boot ROM image (never vendored in git).
#[derive(Debug, Clone, Default)]
pub struct AgbBootFirmware {
    bytes: Option<Vec<u8>>,
}

impl AgbBootFirmware {
    /// Empty slot (FastHle only until loaded).
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Install firmware bytes. Rejects empty images.
    pub fn load(&mut self, bytes: &[u8]) -> Result<(), String> {
        if bytes.is_empty() {
            return Err("CGB-AGB boot ROM image is empty".into());
        }
        self.bytes = Some(bytes.to_vec());
        Ok(())
    }

    /// Clear the slot.
    pub fn clear(&mut self) {
        self.bytes = None;
    }

    /// Whether LLE boot can proceed.
    #[must_use]
    pub fn is_loaded(&self) -> bool {
        self.bytes.as_ref().is_some_and(|b| !b.is_empty())
    }

    /// Borrow loaded bytes.
    #[must_use]
    pub fn bytes(&self) -> Option<&[u8]> {
        self.bytes.as_deref()
    }
}

/// Documented ARM → SM83 handoff latch (HLE of BIOS Mode-8 + HALT).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Mode8Handoff {
    /// DISPCNT.3 set (Mode-8 / CGB presentment armed).
    pub dispcnt_cgb: bool,
    /// HALTCNT applied — ARM stopped for practical purposes; SM83 owns the machine.
    pub halt_applied: bool,
    /// SM83 path is live after handoff.
    pub sm83_active: bool,
}

impl Mode8Handoff {
    /// Idle native GBA (no compat entry).
    #[must_use]
    pub const fn native() -> Self {
        Self {
            dispcnt_cgb: false,
            halt_applied: false,
            sm83_active: false,
        }
    }

    /// Apply the documented detect→Mode-8→HALT sequence (HLE).
    #[must_use]
    pub const fn enter_compat() -> Self {
        Self {
            dispcnt_cgb: true,
            halt_applied: true,
            sm83_active: true,
        }
    }

    /// Merge DISPCNT bit 3 into a register value (HLE; HW is BIOS-only write).
    #[must_use]
    pub const fn with_dispcnt_cgb(dispcnt: u16) -> u16 {
        dispcnt | DISPCNT_CGB_MODE_BIT
    }
}

/// Validate boot mode against firmware presence.
pub fn require_boot(mode: CompatBootMode, fw: &AgbBootFirmware) -> Result<(), String> {
    match mode {
        CompatBootMode::FastHle => Ok(()),
        CompatBootMode::BootRomLle => {
            if fw.is_loaded() {
                Ok(())
            } else {
                Err("BootRomLle requires user-provided CGB-AGB boot ROM (never in git)".into())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fast_hle_needs_no_firmware() {
        let fw = AgbBootFirmware::empty();
        assert!(require_boot(CompatBootMode::FastHle, &fw).is_ok());
        assert!(!fw.is_loaded());
    }

    #[test]
    fn lle_errors_without_firmware() {
        let fw = AgbBootFirmware::empty();
        let err = require_boot(CompatBootMode::BootRomLle, &fw).unwrap_err();
        assert!(err.contains("user-provided") || err.contains("never"));
    }

    #[test]
    fn lle_ok_with_firmware() {
        let mut fw = AgbBootFirmware::empty();
        fw.load(&[0u8; 256]).unwrap();
        assert!(require_boot(CompatBootMode::BootRomLle, &fw).is_ok());
    }

    #[test]
    fn mode8_handoff_sets_bits() {
        let h = Mode8Handoff::enter_compat();
        assert!(h.dispcnt_cgb && h.halt_applied && h.sm83_active);
        assert_eq!(Mode8Handoff::with_dispcnt_cgb(0), DISPCNT_CGB_MODE_BIT);
        assert!(!Mode8Handoff::native().sm83_active);
    }

    #[test]
    fn empty_firmware_load_rejected() {
        let mut fw = AgbBootFirmware::empty();
        assert!(fw.load(&[]).is_err());
    }
}
