//! graycart-gba binary — headless CLI + windowed host (P9).
//!
//! Accepts `--version` / `-V`, `--frames N`, `--hash-out PATH`, `--audio-out PATH`,
//! `--run`, `--debug`, `--trace [N]`, `--quiet` / `--verbose`, optional ROM.
//! No-args opens the windowed host.
//!
//! Cited: graycart-gba test strategy §5.5 / §7 (headless hash + soft audio)
//!   Project store: `docs/graycart-gba/07-test-strategy.md`
//! Cited: PHASES P9 / AGENTS.md (core≠host; headless must not need window stack)
//!   Project store: `docs/graycart-gba/PHASES.md`
//! Cited: graycart-gb CLI verbosity / `--trace` posture
//!   https://github.com/graycart/graycart-gb (src/main.rs)

mod frontend;

use graycart_gba::debug::{print_load_summary, DebugConfig, Verbosity};
use graycart_gba::{Gba, RomLaunchMode};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process;

fn main() {
    let argv: Vec<String> = env::args().skip(1).collect();

    if argv.is_empty() {
        open_gui(
            None,
            DebugConfig::from_env_and_cli(false, false, None, Verbosity::Normal),
        );
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
        let mut cli_debug = false;
        let mut cli_trace = false;
        let mut trace_steps: Option<u64> = None;
        let mut verbosity = Verbosity::Normal;

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
                "--debug" => cli_debug = true,
                "--trace" => {
                    cli_trace = true;
                    // Optional step count (GB: `--trace N` required; we accept bare `--trace`).
                    if let Some(peek) = args.peek() {
                        if !peek.starts_with('-') {
                            if let Ok(n) = peek.parse::<u64>() {
                                let _ = args.next();
                                trace_steps = Some(n);
                                saw_headless = true;
                            }
                        }
                    }
                }
                "--quiet" => verbosity = Verbosity::Quiet,
                "--verbose" => verbosity = Verbosity::Verbose,
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

        let debug_cfg = DebugConfig::from_env_and_cli(cli_debug, cli_trace, trace_steps, verbosity);

        if saw_headless {
            run_headless(frame_cap, hash_out, audio_out, rom_path, debug_cfg);
            return;
        }
        if want_run || rom_path.is_some() {
            open_gui(rom_path, debug_cfg);
            return;
        }
        // Bare `--debug` / `--verbose` without ROM → GUI with config.
        if cli_debug || cli_trace || !matches!(verbosity, Verbosity::Normal) {
            open_gui(None, debug_cfg);
            return;
        }
        eprintln!("{}", frontend::usage());
        process::exit(1);
    }

    // Positional ROM → windowed host (optional trailing flags).
    let rom = PathBuf::from(args.next().expect("peeked"));
    let mut cli_debug = false;
    let mut cli_trace = false;
    let mut trace_steps: Option<u64> = None;
    let mut verbosity = Verbosity::Normal;
    let mut frame_cap: Option<u64> = None;
    let mut hash_out: Option<PathBuf> = None;
    let mut audio_out: Option<PathBuf> = None;
    let mut saw_headless = false;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--debug" => cli_debug = true,
            "--quiet" => verbosity = Verbosity::Quiet,
            "--verbose" => verbosity = Verbosity::Verbose,
            "--trace" => {
                cli_trace = true;
                if let Some(peek) = args.peek() {
                    if !peek.starts_with('-') {
                        if let Ok(n) = peek.parse::<u64>() {
                            let _ = args.next();
                            trace_steps = Some(n);
                            saw_headless = true;
                        }
                    }
                }
            }
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
            "--help" | "-h" => {
                println!("{}", frontend::usage());
                process::exit(0);
            }
            other => {
                eprintln!("unexpected argument: {other}");
                eprintln!("{}", frontend::usage());
                process::exit(1);
            }
        }
    }

    let debug_cfg = DebugConfig::from_env_and_cli(cli_debug, cli_trace, trace_steps, verbosity);
    if saw_headless {
        run_headless(frame_cap, hash_out, audio_out, Some(rom), debug_cfg);
    } else {
        open_gui(Some(rom), debug_cfg);
    }
}

fn open_gui(rom: Option<PathBuf>, debug_cfg: DebugConfig) {
    if let Err(e) = frontend::run(rom, debug_cfg) {
        eprintln!("gui error: {e}");
        process::exit(1);
    }
}

fn run_headless(
    frame_cap: Option<u64>,
    hash_out: Option<PathBuf>,
    audio_out: Option<PathBuf>,
    rom_path: Option<PathBuf>,
    debug_cfg: DebugConfig,
) {
    let mut gba = Gba::new();
    gba.set_debug_config(debug_cfg);

    if let Some(path) = rom_path.as_ref() {
        let bytes = fs::read(path).unwrap_or_else(|e| {
            eprintln!("failed to read {}: {e}", path.display());
            process::exit(1);
        });
        gba.load_rom(&bytes);
        if let Err(e) = gba.reset(RomLaunchMode::BiosHle) {
            eprintln!("{e}");
            process::exit(1);
        }
        print_load_summary(
            debug_cfg.verbosity,
            &path.display().to_string(),
            &bytes,
            gba.cart.header.as_ref(),
            gba.cart.save_kind(),
            RomLaunchMode::BiosHle,
        );
    }

    // GB-style: `--trace N` alone dumps N steps then exits.
    if let Some(n) = debug_cfg.trace_steps {
        if frame_cap.is_none() {
            for _ in 0..n {
                gba.step_instruction();
            }
            return;
        }
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
