<!--
Cited: PHASES.md P8–P9; 07-test-strategy.md §6; 11-test-apparatus.md §2; mgba-emu/suite
Note: living accuracy board — update thresholds when suite automation lands.
-->
# graycart-gba conformance board

Recorded pass thresholds for SoC-depth suites. Default CI does **not** run
`--ignored` rows. jsmolka arm/thumb/memory remain the default-CI PASS spine.

## P8 Timing (2026-09-12)

| Suite | Threshold (passes) | Status | Notes |
|-------|--------------------|--------|-------|
| mGBA **timing** | **0** | not run | `suite.gba` absent; unit G8-prefetch / disable / irq-delay / cycles green |
| mGBA **dma** | **0** | not run | same fixture gap; unit G8-dma-delay green |
| Full mGBA suite board | — | stretch | `#[ignore]` until pinned prebuilt + headless oracle |

Launch mode for future suite runs: **BiosHle** (record BIOS vs no-BIOS if that
changes timing numbers).

Update this table when `tests/fixtures/mgba-suite/suite.gba` is deliberately
vendored and `tests/roms/p8.rs` asserts `passes >= threshold`.

## P9 Frontend (2026-09-12)

| Gate | Status | Notes |
|------|--------|-------|
| Host/core seam (FB + PCM + buttons + `.sav`) | unit green | no GUI imports in core modules |
| Windowed host (eframe / winit+wgpu+egui) + cpal | present | GUI smoke `#[ignore]` without display |
| ROM picker / pause / reset / `.sav` sidecar | present | `.gba` only; no 8-bit UX claim |
| `carts/` commercial smoke | skip-if-missing | see `carts/README.md` |
| Crate / tag | **0.1.0** | first tagged runnable milestone |
| Accuracy tracker / installers | stretch | not required for P9 exit |

Headless `--frames` / `--hash-out` / `--audio-out` remain window-free.
