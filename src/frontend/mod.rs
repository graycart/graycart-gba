//! Host layer — winit/wgpu/egui/cpal posture (via `eframe`); core ≠ GUI.
//!
//! Cited: graycart-gba AGENTS.md / implementation plan §2.1 (core≠host)
//!   Project store: `docs/graycart-gba/AGENTS.md`, `08-implementation-plan.md`
//! Cited: graycart-gb frontend posture (lean port — not feature parity)
//!   https://github.com/graycart/graycart-gb/tree/main/src/frontend
//! Note: DMG/CGB cart UX is P10+ — do not claim shipping 8-bit host here.

mod app;
mod audio;
mod input;
mod pace;
mod rom;
mod sav_fs;
mod ui;
mod video;

pub use app::run;

/// Short usage text for CLI + GUI entry points (P0–P9).
pub fn usage() -> &'static str {
    "graycart-gba — Game Boy Advance emulator\n\
     \n\
     Usage:\n\
       graycart-gba                      open windowed host (ROM picker)\n\
       graycart-gba --run                same as no args\n\
       graycart-gba <rom.gba>            windowed host with ROM\n\
       graycart-gba --frames <N> [--hash-out <path>] [--audio-out <path>] [rom.gba]\n\
       graycart-gba --version\n\
       graycart-gba --help\n\
     \n\
     Headless --frames does not require the window stack.\n\
     --hash-out writes final-frame SHA-256 of 240×160 RGB888.\n\
     --audio-out writes soft stereo WAV of the PCM snapshot.\n\
     Battery .sav is written beside the ROM on exit when a cart is loaded.\n\
     DMG/CGB (.gb/.gbc) support is planned (compat P10+) — not shipping in P9."
}
