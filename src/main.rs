//! graycart-gba binary — headless CLI (P0 + P4 hash + P6 soft audio).
//!
//! Accepts `--version` / `-V`, `--frames N`, `--hash-out PATH`, `--audio-out PATH`,
//! and optional ROM. Windowed host lives under `frontend/` in later phases
//! (plan §2.1 / S8).
//!
//! Cited: graycart-gba test strategy §5.5 / §7 (headless hash + soft audio)
//!   Project store: `docs/graycart-gba/07-test-strategy.md`

mod frontend;

use graycart_gba::Gba;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process;

fn main() {
    let mut args = env::args().skip(1).peekable();

    if args.peek().is_none() {
        eprintln!("{}", frontend::usage());
        process::exit(0);
    }

    let mut frame_cap: Option<u64> = None;
    let mut hash_out: Option<PathBuf> = None;
    let mut audio_out: Option<PathBuf> = None;
    let mut rom_path: Option<PathBuf> = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                println!("{}", frontend::usage());
                process::exit(0);
            }
            "--version" | "-V" => {
                println!("graycart-gba {}", env!("CARGO_PKG_VERSION"));
                process::exit(0);
            }
            "--frames" => {
                let Some(n) = args.next() else {
                    eprintln!("--frames requires a frame count");
                    process::exit(1);
                };
                frame_cap = Some(n.parse().unwrap_or_else(|_| {
                    eprintln!("invalid --frames value: {n}");
                    process::exit(1);
                }));
            }
            "--hash-out" => {
                let Some(p) = args.next() else {
                    eprintln!("--hash-out requires a path");
                    process::exit(1);
                };
                hash_out = Some(PathBuf::from(p));
            }
            "--audio-out" => {
                let Some(p) = args.next() else {
                    eprintln!("--audio-out requires a path");
                    process::exit(1);
                };
                audio_out = Some(PathBuf::from(p));
            }
            other if !other.starts_with('-') => {
                rom_path = Some(PathBuf::from(other));
            }
            other => {
                eprintln!("unknown argument: {other}");
                eprintln!("{}", frontend::usage());
                process::exit(1);
            }
        }
    }

    let mut gba = Gba::new();
    if let Some(path) = rom_path {
        let bytes = fs::read(&path).unwrap_or_else(|e| {
            eprintln!("failed to read {}: {e}", path.display());
            process::exit(1);
        });
        gba.load_rom(&bytes);
        gba.reset_bios_hle();
    }

    if let Some(n) = frame_cap {
        gba.run_frames(n);
        if let Some(path) = hash_out {
            let body = gba.ppu.hash_file_body(n);
            fs::write(&path, body).unwrap_or_else(|e| {
                eprintln!("failed to write {}: {e}", path.display());
                process::exit(1);
            });
        }
        if let Some(path) = audio_out {
            let wav = gba.soft_wav_bytes();
            fs::write(&path, wav).unwrap_or_else(|e| {
                eprintln!("failed to write {}: {e}", path.display());
                process::exit(1);
            });
        }
    } else if hash_out.is_some() || audio_out.is_some() {
        eprintln!("--hash-out / --audio-out require --frames N");
        process::exit(1);
    }
}
