//! Play shell: Game Boy frontend layout, retargeted at this crate for `.gba`
//! and at `graycart` for `.gb` / `.gbc`.
//!
//! Pure keypad binding helpers live in [`input`] and compile without the
//! `frontend` feature. Window / device I/O sits behind `feature = "frontend"`.

pub mod input;

#[cfg(feature = "frontend")]
mod app;
#[cfg(feature = "frontend")]
mod audio;
#[cfg(feature = "frontend")]
mod boot_rom;
#[cfg(feature = "frontend")]
mod brand;
#[cfg(feature = "frontend")]
mod controls;
#[cfg(feature = "frontend")]
mod debug;
#[cfg(feature = "frontend")]
mod fonts;
#[cfg(feature = "frontend")]
mod host;
#[cfg(feature = "frontend")]
mod host_input;
#[cfg(feature = "frontend")]
mod launch;
#[cfg(feature = "frontend")]
mod pace;
#[cfg(feature = "frontend")]
mod playback;
#[cfg(feature = "frontend")]
mod report;
#[cfg(feature = "frontend")]
mod rewind;
#[cfg(feature = "frontend")]
mod rom;
#[cfg(feature = "frontend")]
mod runtime;
#[cfg(feature = "frontend")]
mod screenshot;
#[cfg(feature = "frontend")]
mod settings;
#[cfg(feature = "frontend")]
mod state_slots;
#[cfg(feature = "frontend")]
mod telemetry;
#[cfg(feature = "frontend")]
mod ui;
#[cfg(feature = "frontend")]
mod video;

#[cfg(feature = "frontend")]
pub use app::run;
