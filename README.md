# graycart-gba

Game Boy Advance emulator in the [Graycart family](https://github.com/graycart/graycart).

**Goal:** native GBA software, plus **DMG/CGB** via the GBA hardware compatibility path (prefer reusing [graycart-gb](https://github.com/graycart/graycart-gb)). This repo will eventually supersede graycart-gb as the shipping **app**; GBA-native cores stay greenfield.

## Status

P1 CPU + P2 bus/DMA Immediate are gated by vendored **jsmolka** MIT prebuilts:
**`arm.gba` + `thumb.gba` + `memory.gba` PASS** under BiosHle + r12/idle oracle
(default CI). Crate **0.0.1**. P3+ (timers/IRQ/…) not started. Deeper SoC suites
(mGBA suite, NBA hw-test) are LICENSE/path stubs with `#[ignore]` matrices only.

| Doc | What |
|-----|------|
| [`AGENTS.md`](./AGENTS.md) | How agents navigate: build/test, modules, SemVer, no BIOS/ROMs in git, parallel ownership |
| [`ATTRIBUTION.md`](./ATTRIBUTION.md) | **Mandatory** file-header credit rule (what / URL / inspired-by note) |
| [`CONTRIBUTING.md`](./CONTRIBUTING.md) | Short contributor blurb |
| [`tests/fixtures/README.md`](./tests/fixtures/README.md) | Fixture license table + harness layout |

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

`Cargo.toml` `[package].version` is the only product version; git tag `vX.Y.Z` must equal it. Scaffold is **0.0.1**; first tagged runnable milestone targets **0.1.0** (Project store `08-implementation-plan.md` section 5). Do not ship `1.0.0` until native GBA (and agreed compat) are stable enough.

## Legal / dumps

- Never commit Nintendo BIOS/boot firmware or commercial ROMs.
- Local dumps belong under `carts/` (gitignored) or outside the tree.
- Code is MIT (see [`LICENSE`](./LICENSE)). Fixture suites keep upstream licenses.

## License

[MIT](LICENSE) - Copyright (c) 2026 Graycart.
