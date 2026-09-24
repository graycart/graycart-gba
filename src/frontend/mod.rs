//! Pure host helpers: path → machine kind, shade→RGBA, output device choice, resample.
//! The play shell lives in [`shell`]; window crates stay behind feature `frontend`.

pub mod audio;
pub mod launch;
pub mod shell;
pub mod video;

pub use audio::{AudioOutputChoice, choose_output, resample_linear};
pub use launch::{MachineKind, machine_kind};
pub use video::{gba_framebuffer_to_rgba, sm83_framebuffer_to_rgba};

#[cfg(test)]
mod wiring_tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn machine_kind_bgr555_and_choose_output_are_reachable() {
        assert_eq!(machine_kind(Path::new("a.gba")), Some(MachineKind::Arm));
        assert_eq!(video::bgr555_to_rgba(0x7FFF), [255, 255, 255, 255]);
        let choice = choose_output(None, None, "Speakers", &[]);
        assert_eq!(choice, AudioOutputChoice::Silent);
    }
}
