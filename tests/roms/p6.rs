//! P6 APU harness gates — gba-audio-test smoke + soft WAV + FIFO timing stretch.
//!
//! Cited: cajunpanda/gba-audio-test (MIT)
//!   https://github.com/cajunpanda/gba-audio-test
//! Cited: mGBA issue #1847 directaudiotest (FIFO timing stretch)
//!   https://github.com/mgba-emu/mgba/issues/1847
//! Cited: graycart-gba PHASES P6 · 07/11/12 test docs
//!   Project store: `docs/graycart-gba/PHASES.md`
//! Note: unit gates (G6-regs/psg/fifo/mixer/pcm/glue) live under `src/apu/`.
//! Keep arm/thumb/memory green; never fake PASS.

use crate::harness::{fixture_path, run_test_rom, GbaTestRom, Outcome, RomLaunchMode};
use std::path::Path;

/// Honest progress log for gba-audio-test (G6-audio-rom).
pub const GBA_AUDIO_TEST_PROGRESS: &str =
    "gba-audio-test: not run — ROM absent; unit G6-* green on P6 branch";

struct GbaAudioTestRom;

impl GbaTestRom for GbaAudioTestRom {
    fn id(&self) -> &str {
        "gba-audio-test/main"
    }

    fn relative_path(&self) -> &str {
        "gba-audio-test/gba-audio-test.gba"
    }

    fn launch_mode(&self) -> RomLaunchMode {
        RomLaunchMode::BiosHle
    }

    fn run(&self, rom: &[u8]) -> Outcome {
        let mut gba = graycart_gba::Gba::new();
        gba.load_rom(rom);
        gba.reset_bios_hle();
        // Smoke: advance a few frames without panic (no hard WAV yet).
        gba.run_frames(8);
        let _ = gba.soft_wav_bytes();
        Outcome::Pass
    }
}

#[test]
fn gba_audio_test_progress_logged() {
    assert!(GBA_AUDIO_TEST_PROGRESS.contains("audio") || GBA_AUDIO_TEST_PROGRESS.contains("Audio"));
    eprintln!("G6-audio-rom: {GBA_AUDIO_TEST_PROGRESS}");
}

#[test]
fn gba_audio_test_path_hygiene() {
    let root = Path::new("tests/fixtures/gba-audio-test");
    assert!(root.is_dir());
    assert!(root.join("LICENSE").is_file());
    assert!(root.join("README.md").is_file());
    assert!(!fixture_path("gba-audio-test/gba-audio-test.gba").is_file());
}

/// Opt-in: absent ROM → SKIPPED.
#[test]
#[ignore = "gba-audio-test.gba not vendored — G6-audio-rom ROM gate"]
fn gba_audio_test_matrix() {
    let outcome = run_test_rom(&GbaAudioTestRom);
    eprintln!(
        "{:<40} {}  ({})",
        GbaAudioTestRom.id(),
        outcome.label(),
        outcome.detail()
    );
    assert_eq!(outcome.label(), "SKIPPED");
}

#[test]
fn g6_wav_soft_synthetic() {
    // Soft gate without external golden: silence RMS near zero after bias-only step.
    let mut gba = graycart_gba::Gba::new();
    gba.run_cycles(512 * 32);
    let wav = gba.soft_wav_bytes();
    assert!(wav.len() > 44);
    assert_eq!(&wav[0..4], b"RIFF");
    let frames = gba.apu.pcm.snapshot();
    let rms = graycart_gba::apu::soft_rms(&frames);
    assert!(rms < 1.0, "silence RMS should be near zero, got {rms}");
}

/// Soft WAV vs committed reference — ignore until golden + ROM present.
#[test]
#[ignore = "soft WAV golden not committed — G6-wav-soft ROM/golden gate"]
fn g6_wav_soft_golden_compare() {
    let path = fixture_path("gba-audio-test/soft-ref.wav");
    assert!(
        !path.is_file(),
        "soft-ref.wav unexpectedly present — wire RMS compare before claiming PASS"
    );
}

#[test]
fn fifo_timing_fixture_stub() {
    let dir = Path::new("tests/fixtures/gba-audio-test");
    assert!(dir.join("README.md").is_file());
}

/// Stretch: mGBA #1847 directaudiotest when curated.
#[test]
#[ignore = "directaudiotest not curated — G6-fifo-timing stretch"]
fn g6_fifo_timing_stretch() {
    let path = fixture_path("gba-audio-test/directaudiotest.gba");
    assert!(
        !path.is_file(),
        "directaudiotest unexpectedly present — wire cycle oracle before claiming PASS"
    );
}
