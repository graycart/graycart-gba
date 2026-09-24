# Agent guidelines — Graycart (`graycart-gba`)

How to change this emulator. Product name **Graycart**; crate **`graycart-gba`**. Family: [graycart](https://github.com/graycart/graycart). Peer library: [graycart-gb](https://github.com/graycart/graycart-gb).

Do the next useful thing. Do not lecture. Do not correct an example the user already understands. Do not withhold the files or the edit to invent a rule they did not ask for. Do not be fucking autistic.

This tree is a **greenfield rewrite**. Crate **`0.2.2`** is page 16 (BIOS CpuSet / CpuFastSet). Page 15 (`0.2.1`) is IntrWait / VBlankIntrWait and the `0x03007FF8` check flag. Page 14 (`0.2.0`) was the Game Boy shell behind `frontend`. Page 13 (`0.1.0`) was the first bare play window. Page 12 (`0.0.14`) is Game Boy carts through `graycart`. Page 11 (`0.0.13`) is prefetch / waitstates. Page 10 (`0.0.12`) is cartridge / saves. Page 9 (`0.0.11`) is APU. Page 8 (`0.0.10`) is DMA. Page 7 (`0.0.9`) is windows, blend, mosaic. Page 6 (`0.0.8`) is tiles and sprites. Page 5 (`0.0.7`) is the bitmap picture. Page 4 (`0.0.6`) is timers, IRQ, keypad, halt. Page 3 (`0.0.5`) is the bus. Page 2 (`0.0.4`) is Thumb. Page 1 (`0.0.3`) is ARM. Page 0 (`0.0.2`) is the debug report. Do not restore pre-rewrite 0.1.x by copying old phases, title fixes, BIOS HLE, or the deleted `src/debug` module back in.

## Continue here

Read [`docs/implementation-plan.md`](./docs/implementation-plan.md). Page 16 is done (`0.2.2` — CpuSet / CpuFastSet). Next is page 17 (`0.2.3`). Research and sources: [`docs/README.md`](./docs/README.md).

The command line is:

```text
graycart-gba <rom> --frames <n> [--debug]
graycart-gba <rom> --debug [--frames <n>]
graycart-gba <directory> --frames <n> [--debug]
graycart-gba <directory> --debug [--frames <n>]
graycart-gba   # with `--features frontend`: Game Boy shell (File / Emulation / Video / Input / Audio / Help / Debug)
```

`--debug` stops on its own when the result is pass or fail (idle pass, idle fail, fault, halt with nothing to wake it, or a loop that repeats). The process exits non-zero on fail. `--frames` is a cap on that run, and it is required when `--debug` is absent.

A directory runs every `.gba`, `.gb`, and `.gbc` in it. No `--trace`, `--disasm`, `--ppm`, or `--wav`. When a page is ready to test, print the full command. Do not make the user assemble flags.

Debug text is the contract. Lines start with `gba-debug:` and use `key=value`. `--debug` writes them to stderr and to `gba-debug.log` in the working directory (the file is replaced each run). A summary is every 60 frames, plus a line when boot state changes. `--debug --frames N` ends with a stdout block headed `=== graycart-gba AV report (frame=N) ===`, and that block is copied into the log. A file from `carts/` is `smoke`, never `PASS` or `FAIL`. The audio line, including before the APU exists, is:

```text
gba-debug: apu health frame=<n> master=<0|1> pwm=<hz>Hz fifoA=<route> underrun=<a>/<b> overrun=<a>/<b> empty=<a>/<b> lag=<a>/<b> dma_req=<a>/<b> peak=[<min>..<max>] dc≈[<l>,<r>] clip=<n> extreme=<n> psg_on=0x<n> psg_nr50=0x<vv> fifoB=<route>
```

Pass/fail for jsmolka is `gba-debug: cpu result=PASS r12=0 r7=<n>` or `result=FAIL` with both registers, `pc`, and the mnemonic (`thumb.gba` stores the id in `r7`). Field list and warn lines are in the implementation plan.

This checkout started detached at `46607fe` (tag-side 0.1.14). The umbrella repo still records that commit until its gitlink is updated. Game Boy dumps used for smoke live in `graycart-gb/carts/` and are gitignored. A copy of those carts, plus local notes and a Gitea token, was left on the machine that did the wipe at `Projects/graycart-local-backup`. Do not commit any of that.

## Hardware-first

Commercial progress is a **smoke test**. Conformance ROMs are the **accuracy test**.

- Fix the **owning subsystem** (CPU, bus, DMA, PPU, APU, timer, IRQ, cart, BIOS, compat). Trace to the earliest wrong hardware-visible state.
- **No title hacks.** No game-specific special cases when a real program path after a bad observation would explain it.
- **GBATEK first** for GBA-native behavior, then test-ROM expectations, then trusted secondary notes (mGBA, NanoBoyAdvance, Tonc). A task that cannot name its reference and acceptance test is not ready to implement.
- Unsupported cart features fail clearly.

## Attribution

Full policy: [`ATTRIBUTION.md`](./ATTRIBUTION.md).

If a source file references someone else's code or documentation, credit it at the top of that file: what, URL or name, and a short note (cited / inspired by / ported from).

## Core vs host

The **lib** is the machine. The **binary** owns winit/wgpu/egui/cpal behind the `frontend` feature. Core speaks framebuffer, PCM, and button state only. Headless `--frames` must not depend on the window stack.

- Shade to RGB and display effects live in the host only.
- UI: no disabled "coming soon" items.
- Release builds are the supported play mode once a frontend exists.

## Modules

If something has its own state, rules, tests, or lifecycle, it gets its own module. Keep orchestration thin. Do not dump PPU, timer, APU, or DMA logic into the bus or CPU execute.

Tests: `src/<module>/tests.rs` via `#[cfg(test)] mod tests;` — not inline in production files. Integration under `tests/` uses the public API. ROM harnesses live under `tests/roms/` with fixtures in `tests/fixtures/`.

8-bit DMG/CGB behavior belongs in **graycart-gb**. This repo may depend on that crate. It does not reimplement the SM83 machine.

## SemVer

`Cargo.toml` `[package].version` is the only product version.

- This rewrite starts at **`0.0.1`**.
- One finished page, one version, then stop. Pages 0–12 bump **patch** (`0.0.2` through `0.0.14`). Page 13 (the first runnable host) is **`0.1.0`**. Page 14 (Game Boy shell) is **`0.2.0`**; pages 15–22 continue `0.2.1`–`0.2.8`.
- Each bump updates `Cargo.toml` and adds a `CHANGELOG.md` entry in the same change. Page 0 creates the changelog.
- **Do not** ship `1.0.0` until native GBA is stable, installers exist, settings migrations are trusted, save compatibility is defined, and basic cross-platform play is trusted.
- A git tag is created only when asked. If a tag exists, **`vX.Y.Z` must equal** the crate version.

## Validation

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Default CI must stay green without BIOS images or commercial ROMs.

Never commit `carts/*.gba`, `.sav`, state dumps, Nintendo BIOS / boot firmware, secrets, or skip hooks. Never force-push `main`.

Do not weaken tests (drop asserts, skip without a reason, title-specific expected values).
