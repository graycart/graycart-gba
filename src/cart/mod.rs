//! Cartridge header, save kind, and sidecar.
//!
//! Cited: GBATEK Cartridges.
//! <https://problemkaputt.de/gbatek.htm>

mod eeprom;
mod flash;
mod gpio;
mod header;
mod rtc;

pub use eeprom::Eeprom;
pub use flash::{Flash, FlashSize};
pub use gpio::Gpio;
pub use header::{
    SaveKind, detect_rtc, detect_save, gpio_reject_line, parse_header, read_sidecar, sidecar_path,
    touch_gpio, write_sidecar,
};
pub use rtc::{RTC_SAVE_LEN, SiRtc};
