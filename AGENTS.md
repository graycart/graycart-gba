# Agent guidelines — Graycart (`graycart-gba`)

How to change this emulator. Product name **Graycart**; crate **`graycart-gba`**. Family: [graycart](https://github.com/graycart/graycart). Peer library: [graycart-gb](https://github.com/graycart/graycart-gb).

This tree is a **greenfield rewrite** at crate **0.0.1**. The 0.1.x machine was removed from this working tree and has not been replaced. Page 0 of the plan has not been coded. Do not restore 0.1.x by copying old phases, title fixes, BIOS HLE, or `src/debug` back in.

## Continue here

Read [`docs/implementation-plan.md`](./docs/implementation-plan.md) and implement **page 0 only**, then stop for a manual test. Research and sources: [`docs/README.md`](./docs/README.md).

The command line is only:

```text
graycart-gba <rom> --frames <n> [--debug]
graycart-gba <rom> --frames <n> --debug=trace
```

No `--trace`, `--disasm`, `--ppm`, or `--wav`. When a page is ready to test, print the full command. Do not make the user assemble flags.

Debug text is the contract. Stderr lines start with `gba-debug:` and use `key=value`. `--debug --frames N` ends with a stdout block headed `=== graycart-gba AV report (frame=N) ===`. The audio line, including before the APU exists, is:

```text
gba-debug: apu health frame=<n> master=<0|1> pwm=<hz>Hz fifoA=<route> underrun=<a>/<b> overrun=<a>/<b> empty=<a>/<b> lag=<a>/<b> dma_req=<a>/<b> peak=[<min>..<max>] dc≈[<l>,<r>] clip=<n> extreme=<n> psg_on=0x<n> psg_nr50=0x<vv> fifoB=<route>
```

Pass/fail for jsmolka is `gba-debug: cpu result=PASS r12=0` or `result=FAIL` with `pc` and the mnemonic. Field list and warn lines are in the implementation plan.

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

The **lib** is the machine. A future **binary** owns winit/wgpu/egui/cpal. Core speaks framebuffer, PCM, and button state only. Headless `--frames` must not depend on the window stack.

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
- Completed slices bump **patch** unless the change is a real 0.x **minor**.
- **Do not** ship `1.0.0` until native GBA is stable, installers exist, settings migrations are trusted, save compatibility is defined, and basic cross-platform play is trusted.
- Git tag **`vX.Y.Z` must equal** the crate version.

## Validation

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Default CI must stay green without BIOS images or commercial ROMs.

Never commit `carts/*.gba`, `.sav`, state dumps, Nintendo BIOS / boot firmware, secrets, or skip hooks. Never force-push `main`.

Do not weaken tests (drop asserts, skip without a reason, title-specific expected values).
