//! Keypad (`KEYINPUT` / `KEYCNT`) — host pad → MMIO + keypad IRQ (P3 **G3-keyinput**).
//!
//! Cited: GBATEK — GBA Keypad Input
//!   https://problemkaputt.de/gbatek-gba-keypad-input.htm
//! Cross-check: research `docs/graycart-gba/05-io-timers-irq-input.md` §6.
//! Note: unused KEYINPUT bits 10–15 read as 1 (IO-TBD-2); level re-raise after IF
//! ack while held is TBD (IO-TBD-3).

/// `KEYINPUT` (`04000130`) — read-only, active-low buttons.
pub const KEYINPUT_ADDR: u32 = 0x0400_0130;
/// `KEYCNT` (`04000132`) — R/W IRQ select / enable / condition.
pub const KEYCNT_ADDR: u32 = 0x0400_0132;

/// IE/IF bit for keypad IRQ.
pub const IRQ_KEYPAD_BIT: u16 = 1 << 12;

/// Low 10 button bits (shared by KEYINPUT and KEYCNT select).
pub const BUTTON_MASK: u16 = 0x03FF;
/// KEYCNT writable fields: buttons 0–9, IRQ enable (14), AND/OR (15).
pub const KEYCNT_WRITABLE_MASK: u16 = 0xC3FF;
/// KEYCNT bit 14 — IRQ enable.
pub const KEYCNT_IRQ_ENABLE: u16 = 1 << 14;
/// KEYCNT bit 15 — `0` = OR (any), `1` = AND (all selected).
pub const KEYCNT_IRQ_AND: u16 = 1 << 15;

/// Host-logical button bits (`1` = pressed). Invert for `KEYINPUT`.
pub mod button {
    pub const A: u16 = 1 << 0;
    pub const B: u16 = 1 << 1;
    pub const SELECT: u16 = 1 << 2;
    pub const START: u16 = 1 << 3;
    pub const RIGHT: u16 = 1 << 4;
    pub const LEFT: u16 = 1 << 5;
    pub const UP: u16 = 1 << 6;
    pub const DOWN: u16 = 1 << 7;
    pub const R: u16 = 1 << 8;
    pub const L: u16 = 1 << 9;
}

/// Sink for raising IF bits without depending on irq module internals.
///
/// The irq stream should implement this (or callers pass a closure via
/// [`Input::poll_keypad_irq`]) so keypad can set IF bit 12.
pub trait RaiseIf {
    /// OR `bits` into the interrupt request flags (`IF`).
    fn raise_if(&mut self, bits: u16);
}

impl<F: FnMut(u16)> RaiseIf for F {
    fn raise_if(&mut self, bits: u16) {
        self(bits);
    }
}

/// Keypad register block + host pad state.
#[derive(Debug, Clone)]
pub struct Input {
    /// Host-logical pressed mask (`1` = pressed). Only bits 0–9 are meaningful.
    pressed: u16,
    /// Raw `KEYCNT` (masked on write).
    keycnt: u16,
}

impl Default for Input {
    fn default() -> Self {
        Self::new()
    }
}

impl Input {
    /// Power-on: all buttons released; KEYCNT cleared.
    #[must_use]
    pub fn new() -> Self {
        Self {
            pressed: 0,
            keycnt: 0,
        }
    }

    // --- Host → pad mapping ---

    /// Replace the full pressed mask (host-logical: `1` = pressed).
    pub fn set_pressed(&mut self, mask: u16) {
        self.pressed = mask & BUTTON_MASK;
    }

    /// Mark buttons pressed (OR into host mask).
    pub fn press(&mut self, buttons: u16) {
        self.pressed |= buttons & BUTTON_MASK;
    }

    /// Mark buttons released (clear from host mask).
    pub fn release(&mut self, buttons: u16) {
        self.pressed &= !(buttons & BUTTON_MASK);
    }

    /// Release every button.
    pub fn clear_pressed(&mut self) {
        self.pressed = 0;
    }

    /// Host-logical pressed mask (`1` = pressed).
    #[must_use]
    pub fn pressed_mask(&self) -> u16 {
        self.pressed
    }

    // --- KEYINPUT (active-low) ---

    /// `KEYINPUT` halfword: `0` = pressed, `1` = released; bits 10–15 forced `1`.
    #[must_use]
    pub fn read_keyinput(&self) -> u16 {
        // Invert low 10; unused high bits read as 1 (common emu / GBATEK “not used”).
        (!self.pressed & BUTTON_MASK) | !BUTTON_MASK
    }

    // --- KEYCNT ---

    #[must_use]
    pub fn read_keycnt(&self) -> u16 {
        self.keycnt
    }

    /// Write `KEYCNT`; only buttons 0–9 + bits 14–15 are kept.
    pub fn write_keycnt(&mut self, value: u16) {
        self.keycnt = value & KEYCNT_WRITABLE_MASK;
    }

    /// True when KEYCNT IRQ enable is set and the OR/AND press condition holds.
    #[must_use]
    pub fn keypad_irq_condition_met(&self) -> bool {
        if self.keycnt & KEYCNT_IRQ_ENABLE == 0 {
            return false;
        }
        let select = self.keycnt & BUTTON_MASK;
        if select == 0 {
            // No buttons selected → condition never true (nothing to match).
            return false;
        }
        let pressed = self.pressed & BUTTON_MASK;
        if self.keycnt & KEYCNT_IRQ_AND != 0 {
            // AND: all selected buttons pressed.
            (pressed & select) == select
        } else {
            // OR: any selected button pressed.
            (pressed & select) != 0
        }
    }

    /// If the KEYCNT IRQ condition holds, raise IF bit 12 via `irq`.
    ///
    /// Prefer wiring this to `irq::Irq` once that stream exposes a public
    /// `raise_if` / equivalent; unit tests use a recording sink.
    pub fn poll_keypad_irq<R: RaiseIf>(&self, irq: &mut R) {
        if self.keypad_irq_condition_met() {
            irq.raise_if(IRQ_KEYPAD_BIT);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyinput_default_all_released_active_low() {
        let input = Input::new();
        // All 10 buttons released (1) + unused bits 1 → 0xFFFF.
        assert_eq!(input.read_keyinput(), 0xFFFF);
        assert_eq!(KEYINPUT_ADDR, 0x0400_0130);
        assert_eq!(KEYCNT_ADDR, 0x0400_0132);
    }

    #[test]
    fn host_press_clears_keyinput_bits() {
        let mut input = Input::new();
        input.press(button::A | button::START);
        let ki = input.read_keyinput();
        assert_eq!(ki & button::A, 0, "A pressed → KEYINPUT bit0 clear");
        assert_eq!(ki & button::START, 0, "Start pressed → bit3 clear");
        assert_ne!(ki & button::B, 0, "B still released");
        // Unused bits stay 1.
        assert_eq!(ki & !BUTTON_MASK, !BUTTON_MASK);
    }

    #[test]
    fn set_pressed_and_release_roundtrip() {
        let mut input = Input::new();
        input.set_pressed(button::L | button::R | 0xF000); // high junk ignored
        assert_eq!(input.pressed_mask(), button::L | button::R);
        input.release(button::L);
        assert_eq!(input.pressed_mask(), button::R);
        input.clear_pressed();
        assert_eq!(input.read_keyinput(), 0xFFFF);
    }

    #[test]
    fn keycnt_write_masks_unused_bits() {
        let mut input = Input::new();
        input.write_keycnt(0xFFFF);
        assert_eq!(input.read_keycnt(), KEYCNT_WRITABLE_MASK);
        input.write_keycnt(button::A | KEYCNT_IRQ_ENABLE);
        assert_eq!(input.read_keycnt(), button::A | KEYCNT_IRQ_ENABLE);
    }

    #[test]
    fn keycnt_or_condition_any_selected() {
        let mut input = Input::new();
        // IRQ enable + select A|B, OR mode (bit15=0).
        input.write_keycnt(button::A | button::B | KEYCNT_IRQ_ENABLE);
        assert!(!input.keypad_irq_condition_met());
        input.press(button::B);
        assert!(input.keypad_irq_condition_met());
        input.clear_pressed();
        input.press(button::SELECT); // not in select mask
        assert!(!input.keypad_irq_condition_met());
    }

    #[test]
    fn keycnt_and_condition_all_selected() {
        let mut input = Input::new();
        input.write_keycnt(button::A | button::B | KEYCNT_IRQ_ENABLE | KEYCNT_IRQ_AND);
        input.press(button::A);
        assert!(!input.keypad_irq_condition_met(), "AND needs both A and B");
        input.press(button::B);
        assert!(input.keypad_irq_condition_met());
    }

    #[test]
    fn keycnt_irq_disabled_never_fires() {
        let mut input = Input::new();
        input.write_keycnt(button::A); // no bit14
        input.press(button::A);
        assert!(!input.keypad_irq_condition_met());
    }

    #[test]
    fn poll_raises_if_bit_12_via_sink() {
        let mut input = Input::new();
        input.write_keycnt(button::START | KEYCNT_IRQ_ENABLE);
        input.press(button::START);

        let mut raised = 0u16;
        input.poll_keypad_irq(&mut |bits: u16| raised |= bits);
        assert_eq!(raised, IRQ_KEYPAD_BIT);

        // No raise when condition false.
        raised = 0;
        input.release(button::START);
        input.poll_keypad_irq(&mut |bits: u16| raised |= bits);
        assert_eq!(raised, 0);
    }

    #[test]
    fn empty_select_mask_does_not_fire() {
        let mut input = Input::new();
        input.write_keycnt(KEYCNT_IRQ_ENABLE); // enable, no buttons
        input.press(BUTTON_MASK);
        assert!(!input.keypad_irq_condition_met());
    }
}
