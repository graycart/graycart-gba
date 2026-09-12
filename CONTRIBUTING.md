# Contributing to Graycart (`graycart-gba`)

Thanks for helping with the Game Boy Advance emulator in the [Graycart family](https://github.com/graycart/graycart).

For engineering norms (project goal, hardware-first, **file-header attribution**, SemVer, validation), see [`AGENTS.md`](./AGENTS.md).

## Current status

Phase 0 scaffold is in progress (`Cargo.toml` **0.0.1**, CI, stubs). Design and phase gates live in the Graycart Project research pack (`docs/graycart-gba/`). Do not start emulator **subsystem** implementation (P1+) until Phase 0 exit and the research review gate are accepted.

## When code exists

Rust stable (see CI). Validation before calling a change done:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Do **not** commit commercial ROMs, `.sav` / state files, or Nintendo BIOS / boot firmware. Local dumps belong under `carts/` (gitignored) or stay outside the tree.

## Attribution

If your change references external docs or other projects’ code, credit them **at the top of the file** you edit — see [`ATTRIBUTION.md`](./ATTRIBUTION.md) and the Attribution section in [`AGENTS.md`](./AGENTS.md). Provenance indexes alone are not enough.

## License

Code in this repository is MIT (see [`LICENSE`](./LICENSE)). Test fixtures (when added) keep their **upstream** licenses and are **not** covered by the repo MIT grant.
