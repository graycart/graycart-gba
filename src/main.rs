//! graycart-gba binary — headless CLI + windowed host (P9).
//!
//! Accepts `--version` / `-V`, `--frames N`, `--hash-out PATH`, `--audio-out PATH`,
//! `--run`, optional ROM. No-args opens the windowed host.
//!
//! Cited: graycart-gba test strategy §5.5 / §7 (headless hash + soft audio)
//!   Project store: `docs/graycart-gba/07-test-strategy.md`
//! Cited: PHASES P9 / AGENTS.md (core≠host; headless must not need window stack)
//!   Project store: `docs/graycart-gba/PHASES.md`

mod frontend;

use graycart_gba::Gba;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process;

fn main() {
    let argv: Vec<String> = env::args().skip(1).collect();

    if argv.is_empty() {
        open_gui(None);
        return;
    }

    let mut args = argv.into_iter().peekable();

    // Flag-only / GUI path before headless --frames parsing.
    if args
        .peek()
        .is_some_and(|a| a.starts_with('-') && a.as_str() != "-")
    {
        let mut want_run = false;
        let mut frame_cap: Option<u64> = None;
        let mut hash_out: Option<PathBuf> = None;
        let mut audio_out: Option<PathBuf> = None;
        let mut rom_path: Option<PathBuf> = None;
        let mut saw_headless = false;

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
                "--run" => want_run = true,
                "--frames" => {
                    saw_headless = true;
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
                    saw_headless = true;
                    let Some(p) = args.next() else {
                        eprintln!("--hash-out requires a path");
                        process::exit(1);
                    };
                    hash_out = Some(PathBuf::from(p));
                }
                "--audio-out" => {
                    saw_headless = true;
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

        if saw_headless {
            run_headless(frame_cap, hash_out, audio_out, rom_path);
            return;
        }
        if want_run || rom_path.is_some() {
            open_gui(rom_path);
            return;
        }
        eprintln!("{}", frontend::usage());
        process::exit(1);
    }

    // Positional ROM → windowed host.
    let rom = PathBuf::from(args.next().expect("peeked"));
    if let Some(extra) = args.next() {
        eprintln!("unexpected argument: {extra}");
        eprintln!("{}", frontend::usage());
        process::exit(1);
    }
    open_gui(Some(rom));
}

fn open_gui(rom: Option<PathBuf>) {
    if let Err(e) = frontend::run(rom) {
        eprintln!("gui error: {e}");
        process::exit(1);
    }
}

fn run_headless(
    frame_cap: Option<u64>,
    hash_out: Option<PathBuf>,
    audio_out: Option<PathBuf>,
    rom_path: Option<PathBuf>,
) {
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
