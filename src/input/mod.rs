//! Keypad input.
//!
//! Cited: GBATEK, Keypad Input. https://problemkaputt.de/gbatek.htm

#[cfg(test)]
mod tests;

const KEY_MASK: u16 = 0x03FF;
const KEYCNT_IRQ_ENABLE: u16 = 1 << 14;
const KEYCNT_AND: u16 = 1 << 15;

/// GBA keypad registers (KEYINPUT / KEYCNT).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Keypad {
    /// KEYINPUT bits 0–9; 0 = pressed, 1 = released. Bits 10–15 always read 0.
    input: u16,
    /// KEYCNT: bits 0–9 select, bit 14 IRQ enable, bit 15 OR/AND.
    cnt: u16,
}

impl Keypad {
    pub fn new() -> Self {
        Self {
            input: KEY_MASK,
            cnt: 0,
        }
    }

    pub fn read_input(&self) -> u16 {
        self.input & KEY_MASK
    }

    pub fn read_cnt(&self) -> u16 {
        self.cnt
    }

    pub fn write_cnt(&mut self, value: u16) {
        self.cnt = value;
    }

    /// `pressed` is a mask of GBATEK key bits (bit 0 = A, 1 = B, 2 = Select,
    /// 3 = Start, 4 = Right, 5 = Left, 6 = Up, 7 = Down, 8 = R, 9 = L). Those
    /// bits become 0 in KEYINPUT. Bits not in the mask become 1 (released).
    /// Only bits 0–9 are stored.
    pub fn set_pressed(&mut self, pressed: u16) {
        let pressed = pressed & KEY_MASK;
        self.input = (!pressed) & KEY_MASK;
    }

    /// True when bit 14 is set and the OR/AND condition matches. False if no
    /// select bits are set.
    pub fn irq_asserted(&self) -> bool {
        if self.cnt & KEYCNT_IRQ_ENABLE == 0 {
            return false;
        }
        let select = self.cnt & KEY_MASK;
        if select == 0 {
            return false;
        }
        // Pressed means KEYINPUT bit is 0.
        let pressed = (!self.input) & KEY_MASK;
        if self.cnt & KEYCNT_AND != 0 {
            // AND: all selected keys pressed.
            pressed & select == select
        } else {
            // OR: any selected key pressed.
            pressed & select != 0
        }
    }
}

impl Default for Keypad {
    fn default() -> Self {
        Self::new()
    }
}
