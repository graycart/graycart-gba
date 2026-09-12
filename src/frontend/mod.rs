//! Host / CLI helpers (binary-only). GUI chrome lands in S8 / P9.
//!
//! Layout noun from graycart-gba implementation plan §2.2.

/// Short usage text for the headless CLI (P0 + P4 hash).
pub fn usage() -> &'static str {
    "graycart-gba — Game Boy Advance emulator\n\
     \n\
     Usage:\n\
       graycart-gba --version\n\
       graycart-gba --frames <N> [--hash-out <path>] [rom.gba]\n\
       graycart-gba --help\n\
     \n\
     --hash-out writes final-frame SHA-256 of 240×160 RGB888 (G4-hash-cli).\n\
     Core≠host: windowed UI reserved for later phases."
}
