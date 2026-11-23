//! Cartridge GPIO at 0x080000C4 / C6 / C8.
//!
//! When an RTC is present the three registers talk to the S-3511A. Otherwise a
//! cart marked `has_gpio` still rejects (rumble / tilt / solar stub).

use super::rtc::SiRtc;

/// GPIO port sitting in the ROM hole at 0xC4/C6/C8.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Gpio {
    /// Data register low 4 bits (SCK/SIO/CS/…).
    data: u8,
    /// Direction: 1 = output (CPU drives), 0 = input (device drives).
    direction: u8,
    /// Control bit0: 1 = GPIO registers readable.
    readable: bool,
    /// Optional S-3511A. `None` → unsupported GPIO (warn path).
    rtc: Option<SiRtc>,
}

impl Gpio {
    /// GPIO with a live RTC (Pokemon Ruby/Sapphire/Emerald, etc.).
    pub fn with_rtc() -> Self {
        Self {
            data: 0,
            direction: 0,
            readable: false,
            rtc: Some(SiRtc::new_from_host()),
        }
    }

    /// GPIO present but no device (tests / reject path).
    pub fn unsupported() -> Self {
        Self {
            data: 0,
            direction: 0,
            readable: false,
            rtc: None,
        }
    }

    pub fn has_rtc(&self) -> bool {
        self.rtc.is_some()
    }

    pub fn rtc_mut(&mut self) -> Option<&mut SiRtc> {
        self.rtc.as_mut()
    }

    pub fn rtc(&self) -> Option<&SiRtc> {
        self.rtc.as_ref()
    }

    /// Read a GPIO register. `offset` is 0xC4 / 0xC6 / 0xC8.
    /// Returns `None` when control says write-only (reads as 0).
    pub fn read(&self, offset: u32) -> u16 {
        if !self.readable {
            return 0;
        }
        match offset {
            0xC4 => u16::from(self.data & 0xF),
            0xC6 => u16::from(self.direction & 0xF),
            0xC8 => u16::from(u8::from(self.readable)),
            _ => 0,
        }
    }

    /// Write a GPIO register. Returns true when an RTC (or stub device) handled it.
    pub fn write(&mut self, offset: u32, value: u16) {
        match offset {
            0xC4 => {
                let latch = (value as u8) & 0xF;
                // CPU-driven bits come from the latch; device may drive inputs.
                self.data = (self.data & !self.direction) | (latch & self.direction);
                self.apply_rtc();
            }
            0xC6 => {
                self.direction = (value as u8) & 0xF;
                // Re-apply latch onto newly-output pins (mGBA non-vba path).
                self.apply_rtc();
            }
            0xC8 => {
                self.readable = value & 1 != 0;
            }
            _ => {}
        }
    }

    fn apply_rtc(&mut self) {
        let pins = self.data & 0xF;
        let Some(rtc) = self.rtc.as_mut() else {
            return;
        };
        let sio = rtc.update_pins(pins);
        // SIO is bit 1. When direction marks it as input, device drives it.
        if self.direction & 2 == 0 {
            if sio {
                self.data |= 2;
            } else {
                self.data &= !2;
            }
        }
    }
}
