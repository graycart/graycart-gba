# graycart-gba

Game Boy Advance emulator in the [Graycart family](https://github.com/graycart/graycart).

**Goal:** native GBA software, plus **DMG/CGB** via the GBA hardware compatibility path (prefer reusing [graycart-gb](https://github.com/graycart/graycart-gb)). This repo will eventually supersede graycart-gb as the shipping **app**; GBA-native cores stay greenfield.

## Status

**P9 Frontend** — windowed host (eframe / winit+wgpu+egui + cpal): ROM picker,
pause/reset, `.sav` beside ROM. Headless `--frames` / `--hash-out` / `--audio-out`
unchanged. Crate **0.1.0** (first tagged runnable milestone). DMG/CGB cart UX is
**not** shipping yet (compat P10+).

Default CI still gates vendored **jsmolka** MIT prebuilts:
**`arm.gba` + `thumb.gba` + `memory.gba` PASS** under BiosHle + r12/idle oracle.
Deeper SoC suites (mGBA suite, NBA hw-test) stay LICENSE/path stubs with
`#[ignore]` until fixtures land. Optional commercial smoke: local `carts/*.gba`
(skip if missing — never commit dumps).

### BIOS (obtain your own)

Default play uses **BiosHle** (no firmware file required). For BiosLle, supply
your own `gba_bios.bin` (never commit Nintendo BIOS). See [`AGENTS.md`](./AGENTS.md)
and `docs/conformance.md`.

| Doc | What |
|-----|------|
| [`AGENTS.md`](./AGENTS.md) | How agents navigate: build/test, modules, SemVer, no BIOS/ROMs in git, parallel ownership |
| [`ATTRIBUTION.md`](./ATTRIBUTION.md) | **Mandatory** file-header credit rule (what / URL / inspired-by note) |
| [`CONTRIBUTING.md`](./CONTRIBUTING.md) | Short contributor blurb |
| [`docs/conformance.md`](./docs/conformance.md) | Accuracy / phase gate board (P8 timing, P9 frontend) |
| [`tests/fixtures/README.md`](./tests/fixtures/README.md) | Fixture license table + harness layout |
| [`carts/README.md`](./carts/README.md) | Optional local dumps (gitignored binaries) |

Research, phases, and provenance live in the Graycart Project store under `docs/graycart-gba/` until linked or copied here. Family core API: Project store `docs/graycart-family/` (especially `01-core-api.md`).

## Build / test

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

### CI matrix (`.github/workflows/ci.yml`)

| Job name | Runner | What it runs |
|----------|--------|--------------|
| `ubuntu-latest` | Linux | `fmt` + `clippy -D warnings` + `cargo test` (+ alsa/udev pkgs) |
| `macos-latest` | macOS | same rust checks |
| `windows-latest` | Windows | same rust checks |

- **`fail-fast: false`** — one OS flake does not cancel the others.
- **Default `cargo test` asserts** jsmolka **arm + thumb + memory** → **PASS** (honest green; never fake).
- **Not in default CI:** `cargo test -- --ignored` (mGBA suite / NBA hw-test / full matrix printout). Those need future fixtures or local opt-in — never merge-blocking until SoC-depth thresholds are agreed. No BIOS images required.
- Optional later: a `workflow_dispatch` / nightly that runs `--ignored` for logging only — not wired yet on purpose.

## SemVer

`Cargo.toml` `[package].version` is the only product version; git tag `vX.Y.Z` must equal it. First tagged runnable milestone is **0.1.0** (P9 playable host). Do not ship `1.0.0` until native GBA (and agreed compat) are stable enough.

## Legal / dumps

- Never commit Nintendo BIOS/boot firmware or commercial ROMs.
- Local dumps belong under `carts/` (gitignored) or outside the tree.
- Code is MIT (see [`LICENSE`](./LICENSE)). Fixture suites keep upstream licenses.

## License

[MIT](LICENSE) - Copyright (c) 2026 Graycart.
