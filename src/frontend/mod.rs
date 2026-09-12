//! Host / CLI helpers (binary-only). GUI chrome lands in S8 / P9.
//!
//! Layout noun from graycart-gba implementation plan §2.2.

/// Short usage text for the P0 headless stub.
pub fn usage() -> &'static str {
    "graycart-gba — Game Boy Advance emulator (scaffold)\n\
     \n\
     Usage:\n\
       graycart-gba --version\n\
       graycart-gba --frames <N>\n\
       graycart-gba --help\n\
     \n\
     P0: no ROM load or window yet. Core≠host split reserved for later phases."
}
