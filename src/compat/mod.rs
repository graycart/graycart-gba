//! DMG/CGB-via-GBA hardware compat (P10 bring-up / P11 accuracy) — prefer graycart reuse.
//!
//! Cited: graycart-gba implementation plan §2.2 / §3.11–3.12 / PHASES P10–P11
//!   Project store: `docs/graycart-gba/08-implementation-plan.md`
//! Cited: `09-dmg-cgb-compatibility.md`, `10-core-api-and-gb-reuse.md`
//! Note: wrapper around graycart SM83/PPU/APU — do not reinvent cores here.

mod boot;
mod bridge;
mod dep;
mod detect;
mod machine;

pub use boot::{require_boot, AgbBootFirmware, CompatBootMode, Mode8Handoff, DISPCNT_CGB_MODE_BIT};
pub use bridge::{
    apply_lr_stretch, framebuffer_rgb888, gba_mask_to_gb_buttons, shade_to_rgb888, StretchMode,
};
pub use dep::{GRAYCART_DEP_LABEL, GRAYCART_GIT_REV};
pub use detect::{
    header_cgb_byte, hint_from_extension, select_profile, CartClass, LoadPathHint, MachineProfile,
};
pub use machine::{CompatMachine, CompatSilicon};

use crate::bus::waitcnt::WaitCnt;

/// SoC-side compat latch on [`crate::Gba`] — WAITCNT cart class + Mode-8 handoff.
///
/// The live SM83 machine lives in [`CompatMachine`] when the product is on the
/// GB path; this struct records the GBA-side detect/boot posture.
#[derive(Debug, Clone, Default)]
pub struct Compat {
    /// Last Mode-8 / HALT handoff (native until enter_compat).
    pub handoff: Mode8Handoff,
    /// User-supplied CGB-AGB boot ROM slot (never in git).
    pub firmware: AgbBootFirmware,
    /// Preferred boot mode for the next compat entry.
    pub boot_mode: CompatBootMode,
    /// Display stretch preference (L/R).
    pub stretch: StretchMode,
}

impl Compat {
    /// Power-on native (no SM83 handoff yet).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Reflect WAITCNT bit 15 into a [`CartClass`].
    #[must_use]
    pub fn cart_class(&self, waitcnt: WaitCnt) -> CartClass {
        CartClass::from_waitcnt(waitcnt)
    }

    /// Enter documented Mode-8 + HALT posture (HLE).
    pub fn enter_compat_handoff(&mut self) {
        self.handoff = Mode8Handoff::enter_compat();
    }

    /// Leave compat (native GBA again).
    pub fn leave_compat_handoff(&mut self) {
        self.handoff = Mode8Handoff::native();
    }

    /// Sync cart-type RO bit from an explicit load hint / attach.
    pub fn apply_cart_class(waitcnt: &mut WaitCnt, class: CartClass) {
        waitcnt.set_cart_type_cgb(matches!(class, CartClass::GbCompat));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compat_latch_enter_leave() {
        let mut c = Compat::new();
        assert!(!c.handoff.sm83_active);
        c.enter_compat_handoff();
        assert!(c.handoff.sm83_active);
        c.leave_compat_handoff();
        assert!(!c.handoff.sm83_active);
    }

    #[test]
    fn apply_cart_class_sets_waitcnt_bit15() {
        let mut wc = WaitCnt::power_on();
        Compat::apply_cart_class(&mut wc, CartClass::GbCompat);
        assert!(wc.cart_type_cgb());
        Compat::apply_cart_class(&mut wc, CartClass::GbaRom);
        assert!(!wc.cart_type_cgb());
    }
}
