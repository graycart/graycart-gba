# Agent guidelines -- Graycart (`graycart-gba`)

How to change this emulator. Product name **Graycart**; crate/binary target **`graycart-gba`**. Family: [graycart](https://github.com/graycart/graycart). Peer library / maintenance host: [graycart-gb](https://github.com/graycart/graycart-gb).

## Project goal

1. Run **native GBA software** (ARM7TDMI + GBA bus/PPU/APU/DMA/IRQ).
2. Run **8-bit DMG/CGB cartridges** via the GBA hardware GB/CGB compatibility path -- **prefer reusing** graycart-gb / a future gb-core crate; do not reinvent SM83/PPU/APU for sport.
3. **Supersede** `graycart-gb` as the shipping **app** / host for 8-bit + GBA (**P12 cutover** — see [`docs/supersede-cutover.md`](./docs/supersede-cutover.md)). Keep the `graycart` lib dep intentional. GBA-native modules stay **greenfield** (do not port gb into ARM).

### How agents should navigate

1. Read **this file** + [`ATTRIBUTION.md`](./ATTRIBUTION.md) before editing sources.
2. Research pack (architecture, phases, provenance): Graycart Project store `docs/graycart-gba/` -- start with `PHASES.md` and `08-implementation-plan.md`; reuse/API in `10-core-api-and-gb-reuse.md`.
3. Family-standard core API: Project store `docs/graycart-family/01-core-api.md` (not yet vendored in this repo) -- align host/core seams when that extract lands.
4. Own only your Phase partition paths; keep PRs on shared bootstrap branches when instructed.

## Hardware-first

Commercial progress is a **smoke test**. Conformance ROMs are the **accuracy test**.

- Fix the **owning subsystem** (CPU, bus, DMA, PPU, APU, timer, IRQ, cart, BIOS, compat). Trace to the earliest wrong hardware-visible state.
- **No title hacks.** No game-specific special cases when a real program path after a bad observation would explain it.
- **GBATEK first** for GBA-native behavior, then test-ROM expectations, then trusted secondary notes (mGBA, NanoBoyAdvance, Tonc, etc.). A task that cannot name its reference and acceptance test is not ready to implement.
- Unsupported cart features fail clearly -- never silent wrong-save-type or pretend-ROM-only.

## Attribution (file headers -- mandatory)

Full policy + examples: [`ATTRIBUTION.md`](./ATTRIBUTION.md).

If a source file (Rust **or** Markdown in this repo) references someone else's **code** or **internet documentation** (GBATEK, Pan Docs, ARM TRM, blogs, another emulator, research excerpts, etc.), put **credit at the top of that file** (Rust module/`//!` docs or Markdown header):

- what was used
- URL and/or name
- brief note (inspired by / ported from / cited)

Provenance folders and README link lists alone are **not** enough for in-tree sources that depend on those materials.

Example (Rust):

```rust
//! Bus waitstates and Game Pak timing.
//!
//! Cited: GBATEK -- Gamepak Waitstates / Prefetch
//!   https://problemkaputt.de/gbatek.htm
//! Inspired by: mGBA waitstate tables (secondary cross-check only).
```

Example (Markdown):

```markdown
<!--
Cited: GBATEK -- Interrupt Control
URL: https://problemkaputt.de/gbatek.htm
Note: section outline for IE/IF/IME; not a full reprint.
-->
```

Do not commit entire manuals or commercial dumps. Keep excerpts minimal and attributed.

## Core vs host

The **lib** is the machine. The **binary** (`frontend/`, when present) owns winit/wgpu/egui/cpal. Core speaks framebuffer, PCM, and button state only. Headless `--frames` must not depend on the window stack.

- Shade -> RGB and display effects live in the host only.
- UI: no disabled "coming soon" items for unfinished core features.
- Release builds are the supported play mode once a frontend exists.

Align public host/core seams with the family-standard core API when that extract lands.

## Modules

If something has its own state, rules, tests, or lifecycle, it gets its own module. Keep orchestration thin (`lib.rs` / machine glue, `bus`, `main.rs`). Do not dump PPU/timer/APU/DMA logic into the bus or CPU execute.

Suggested nouns: CPU -> Bus -> Cart / BIOS; DMA, Timer, PPU, APU, IRQ, input, `hw/`, `compat/` for the GB/CGB path (prefer gb dependency; host UI stays in `frontend/`).

Tests: `src/<module>/tests.rs` via `#[cfg(test)] mod tests;` -- not inline in production files. Integration under `tests/` uses the public API. ROM harnesses live under `tests/roms/` with fixtures in `tests/fixtures/` (licenses + README per suite).

## Parallelism

Independent tasks: disjoint files, no shared unfinished types. Do not have two implementers on the same unfinished API surface. Interface producers before consumers.

## SemVer

Same posture as [graycart-gb `AGENTS.md`](https://github.com/graycart/graycart-gb/blob/main/AGENTS.md) and Project store `docs/graycart-gba/08-implementation-plan.md` section 5:

- `Cargo.toml` `[package].version` is the only product version (window title, About, any save-header version field).
- Completed slices bump **patch** unless the change is a real 0.x **minor**.
- Scaffold started at **`0.0.1`**; first tagged runnable milestone is **`0.1.0`** (P9 playable host). P10 **0.1.1**, P11 **0.1.2**, P12 cutover **0.1.3**, console debug **0.1.4**, AV health summary **0.1.9**.
- **Do not** ship `1.0.0` until native GBA (and agreed compat) are stable enough, installers/settings/save compatibility are trusted, and basic cross-platform play is trusted.
- Git tag **`vX.Y.Z` must equal** the crate version. Release jobs should fail on mismatch.

## Validation (required)

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Default CI must stay green **without** BIOS images, commercial ROMs, or ignored ROM matrices. Default `cargo test` **does** assert vendored jsmolka **arm+thumb+memory PASS**. Never `cargo test -- --ignored` in the default workflow. Crate version **0.1.0** is the first tagged runnable milestone (P9).

Never commit `carts/*.gba`, `.sav`, state dumps, Nintendo BIOS / boot firmware, secrets, or skip hooks. Never force-push `main`.

Do not weaken tests (drop asserts, skip without a reason, title-specific expected values).

## Docs and research

- In-repo: this file, [`ATTRIBUTION.md`](./ATTRIBUTION.md), `README.md`, `CONTRIBUTING.md`, `docs/conformance.md`, [`docs/supersede-cutover.md`](./docs/supersede-cutover.md), + BIOS obtain-your-own notes.
- Research (phases, GBATEK provenance, DMG/CGB compat, core API reuse): Graycart Project store `docs/graycart-gba/` -- especially `08-implementation-plan.md`, `PHASES.md`, `07-test-strategy.md`, `11-test-apparatus.md`, `12-test-gates.md`, `09-dmg-cgb-compatibility.md`, `10-core-api-and-gb-reuse.md`, and `provenance/`.
- Family API: Project store `docs/graycart-family/` (`01-core-api.md`, `02-repo-layout.md`, `repos.md`).

Planned phase ladder **P0–P12** is complete on the product path. Do not invent P13. Keep default CI honest-green (jsmolka arm+thumb+memory PASS).
