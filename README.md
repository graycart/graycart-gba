# graycart-gba

Game Boy Advance emulator in the [Graycart family](https://github.com/graycart/graycart).

**Goal:** native GBA software, plus **DMG/CGB** via the GBA hardware compatibility path (prefer reusing [graycart-gb](https://github.com/graycart/graycart-gb)). This repo is the long-term shipping **app** for 8-bit + GBA (P12 cutover); GBA-native cores stay greenfield.

## Status

**P12 Supersede cutover** — graycart-gba is the recommended play host for
**GBA + DMG/CGB**. Dual-run → default-gba plan:
[`docs/supersede-cutover.md`](./docs/supersede-cutover.md). Intentional
whole-crate `graycart` dep remains (supersede the **app**, not the library).
Crate **0.1.3**. Planned phase ladder **P0–P12** complete.

**P11 Compat accuracy** — Blargg/Mooneye boards via `CompatMachine` (FastDmg /
FastCgb); frontend loads `.gba` / `.gb` / `.gbc` with 8-bit `.sav`. Suite
matrices are `#[ignore]` with thresholds in `docs/conformance.md`. Crate
**0.1.2**.

**P10 DMG/CGB compat bring-up** — `graycart` whole-crate dep behind `compat/`
(WAITCNT detect, Mode-8/HALT posture, `CompatMachine`). Crate **0.1.1**.

**P9 Frontend** — windowed host (eframe / winit+wgpu+egui + cpal). First tagged
runnable milestone: **0.1.0**.

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
| [`docs/conformance.md`](./docs/conformance.md) | Accuracy / phase gate board (P8–P12) |
| [`docs/supersede-cutover.md`](./docs/supersede-cutover.md) | P12 dual-run → default-gba handoff |
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

`Cargo.toml` `[package].version` is the only product version; git tag `vX.Y.Z` must equal it. First tagged runnable milestone is **0.1.0** (P9 playable host). P10 **0.1.1**, P11 **0.1.2**, P12 **0.1.3**. Do not ship `1.0.0` until native GBA (and agreed compat) are stable enough.

## Legal / dumps

- Never commit Nintendo BIOS/boot firmware or commercial ROMs.
- Local dumps belong under `carts/` (gitignored) or outside the tree.
- Code is MIT (see [`LICENSE`](./LICENSE)). Fixture suites keep upstream licenses.

## License

[MIT](LICENSE) - Copyright (c) 2026 Graycart.
