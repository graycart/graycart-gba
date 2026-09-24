//! ARM save-slot snapshots (`GAS1` beside the ROM, `.gasN`). Distinct from the `.sav` sidecar.

mod format;

#[cfg(test)]
mod tests;

pub use format::{Gas1Error, Gas1Metadata, decode_gas1, encode_gas1, rom_sha256, sanitize_title};

pub const SNAPSHOT_FORMAT_VERSION: u16 = 1;
