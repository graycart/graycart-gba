//! ARM7TDMI processor modes and banked-register ownership.
//!
//! Cited: GBATEK -- ARM CPU Flags / Mode Bits
//!   https://problemkaputt.de/gbatek.htm
//! Cited: GBATEK -- ARM CPU Register Set
//!   https://problemkaputt.de/gbatek.htm
//! Cited: ARM DDI0210C (ARM7TDMI TRM r4p1) -- processor modes
//! Note: encodings and which GPRs/SPSR are banked per mode; GBA has no usable FIQ pin
//! but FIQ mode banking still exists architecturally.

/// ARM7TDMI operating mode (CPSR M\[4:0\]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Mode {
    User = 0x10,
    Fiq = 0x11,
    Irq = 0x12,
    Supervisor = 0x13,
    Abort = 0x17,
    Undefined = 0x1B,
    System = 0x1F,
}

impl Mode {
    /// Parse the low 5 CPSR mode bits. Returns `None` for illegal encodings.
    pub fn from_bits(bits: u32) -> Option<Self> {
        match bits & 0x1F {
            0x10 => Some(Self::User),
            0x11 => Some(Self::Fiq),
            0x12 => Some(Self::Irq),
            0x13 => Some(Self::Supervisor),
            0x17 => Some(Self::Abort),
            0x1B => Some(Self::Undefined),
            0x1F => Some(Self::System),
            _ => None,
        }
    }

    /// CPSR M\[4:0\] encoding.
    #[inline]
    pub const fn bits(self) -> u32 {
        self as u32
    }

    /// Privileged modes may alter CPSR control bits (User may not).
    #[inline]
    pub const fn is_privileged(self) -> bool {
        !matches!(self, Self::User)
    }

    /// User and System share the same register banks; System is privileged.
    #[inline]
    pub const fn shares_user_banks(self) -> bool {
        matches!(self, Self::User | Self::System)
    }

    /// Exception modes own an SPSR; User/System do not.
    #[inline]
    pub const fn has_spsr(self) -> bool {
        matches!(
            self,
            Self::Fiq | Self::Irq | Self::Supervisor | Self::Abort | Self::Undefined
        )
    }

    /// Bank slot used for R13/R14 (and FIQ R8–R12).
    #[inline]
    pub(crate) const fn bank(self) -> Bank {
        match self {
            Self::User | Self::System => Bank::Usr,
            Self::Fiq => Bank::Fiq,
            Self::Irq => Bank::Irq,
            Self::Supervisor => Bank::Svc,
            Self::Abort => Bank::Abt,
            Self::Undefined => Bank::Und,
        }
    }

    /// SPSR bank index when [`Self::has_spsr`].
    #[inline]
    pub(crate) const fn spsr_index(self) -> Option<usize> {
        match self {
            Self::Fiq => Some(0),
            Self::Irq => Some(1),
            Self::Supervisor => Some(2),
            Self::Abort => Some(3),
            Self::Undefined => Some(4),
            Self::User | Self::System => None,
        }
    }
}

/// Physical bank for mode-private R13/R14 (and FIQ R8–R12).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum Bank {
    Usr = 0,
    Fiq = 1,
    Irq = 2,
    Svc = 3,
    Abt = 4,
    Und = 5,
}

impl Bank {
    #[inline]
    pub(crate) const fn index(self) -> usize {
        self as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_round_trip() {
        for bits in [0x10u32, 0x11, 0x12, 0x13, 0x17, 0x1B, 0x1F] {
            let m = Mode::from_bits(bits).expect("valid mode");
            assert_eq!(m.bits(), bits);
        }
        assert!(Mode::from_bits(0x00).is_none());
        assert!(Mode::from_bits(0x14).is_none());
    }

    #[test]
    fn privilege_and_spsr() {
        assert!(!Mode::User.is_privileged());
        assert!(Mode::System.is_privileged());
        assert!(Mode::System.shares_user_banks());
        assert!(!Mode::User.has_spsr());
        assert!(Mode::Irq.has_spsr());
        assert_eq!(Mode::User.bank(), Mode::System.bank());
        assert_ne!(Mode::Irq.bank(), Mode::Supervisor.bank());
    }
}
