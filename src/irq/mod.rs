//! Interrupt Control (IE / IF / IME).
//!
//! Spec: GBATEK Interrupt Control —
//! <https://problemkaputt.de/gbatek.htm>

#[cfg(test)]
mod tests;

/// GBA interrupt enable / request / master-enable registers.
///
/// Offsets are relative to `0x04000200`:
/// - `0`: IE
/// - `2`: IF
/// - `8`: IME
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Irq {
    ie: u16,
    iff: u16,
    ime: bool,
    /// BIOS IntrWait check flags, mirrored at IWRAM `0x03007FF8`.
    #[serde(default)]
    check_flags: u16,
}

impl Irq {
    /// Create with IE=0, IF=0, IME=0.
    pub fn new() -> Self {
        Self {
            ie: 0,
            iff: 0,
            ime: false,
            check_flags: 0,
        }
    }

    /// Read a 16-bit register at the given offset from `0x04000200`.
    pub fn read16(&self, offset: u32) -> u16 {
        match offset {
            0 => self.ie,
            2 => self.iff,
            8 => u16::from(self.ime),
            _ => 0,
        }
    }

    /// Write a 16-bit register at the given offset from `0x04000200`.
    ///
    /// Writing IF acknowledges: each 1-bit in `value` clears that IF bit.
    pub fn write16(&mut self, offset: u32, value: u16) {
        match offset {
            0 => self.ie = value,
            2 => self.iff &= !value,
            8 => self.ime = (value & 1) != 0,
            _ => {}
        }
    }

    /// OR `bit` (a mask, not an index) into IF and the IntrWait check flags.
    pub fn raise(&mut self, bit: u16) {
        self.iff |= bit;
        self.check_flags |= bit;
    }

    /// Halfword at `0x03007FF8` (BIOS interrupt check flags).
    pub fn check_flags(&self) -> u16 {
        self.check_flags
    }

    /// Replace the IntrWait check flags (also written through IWRAM `0x03007FF8`).
    pub fn set_check_flags(&mut self, value: u16) {
        self.check_flags = value;
    }

    /// True when IME bit 0 is set and `(IE & IF) != 0`.
    pub fn pending(&self) -> bool {
        self.ime && (self.ie & self.iff) != 0
    }

    pub fn ie(&self) -> u16 {
        self.ie
    }

    pub fn iff(&self) -> u16 {
        self.iff
    }

    pub fn ime(&self) -> bool {
        self.ime
    }

    /// False when a halted CPU can never be woken: IME is off, or IE is 0.
    pub fn can_wake(&self) -> bool {
        self.ime && self.ie != 0
    }
}

impl Default for Irq {
    fn default() -> Self {
        Self::new()
    }
}
