//! graycart-gba binary stub — headless CLI only (P0).
//!
//! Accepts `--version` / `-V` and `--frames N` per PHASES.md P0 checklist.
//! Windowed host lives under `frontend/` in later phases (plan §2.1 / S8).

mod frontend;

use graycart_gba::Gba;
use std::env;
use std::process;

fn main() {
    let mut args = env::args().skip(1).peekable();

    if args.peek().is_none() {
        // No args: stay headless until a real frontend lands (S8).
        eprintln!("{}", frontend::usage());
        process::exit(0);
    }

    let mut frame_cap: Option<u64> = None;

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
            other => {
                eprintln!("unknown argument: {other}");
                eprintln!("{}", frontend::usage());
                process::exit(1);
            }
        }
    }

    if let Some(n) = frame_cap {
        let mut gba = Gba::new();
        gba.run_frames(n);
    }
}
