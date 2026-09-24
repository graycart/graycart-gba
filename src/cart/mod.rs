//! Cartridge header, save kind, and sidecar.
//!
//! Cited: GBATEK Cartridges.
//! <https://problemkaputt.de/gbatek.htm>

mod eeprom;
mod flash;
mod header;

pub use eeprom::Eeprom;
pub use flash::{Flash, FlashSize};
pub use header::{
    SaveKind, detect_save, gpio_reject_line, parse_header, read_sidecar, sidecar_path, touch_gpio,
    write_sidecar,
};
