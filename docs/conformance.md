<!--
Cited: PHASES.md P8–P11; 07-test-strategy.md §6; 11-test-apparatus.md §2; mgba-emu/suite
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
| ROM picker / pause / reset / `.sav` sidecar | present | P9 was `.gba` only; P11 adds `.gb`/`.gbc` |
| `carts/` commercial smoke | skip-if-missing | see `carts/README.md` |
| Crate / tag | **0.1.0** | first tagged runnable milestone |
| Accuracy tracker / installers | stretch | not required for P9 exit |

Headless `--frames` / `--hash-out` / `--audio-out` remain window-free.

## P10 DMG/CGB compat bring-up (2026-09-12)

| Gate | Status | Notes |
|------|--------|-------|
| `graycart` whole-crate dep | pinned | git rev in `src/compat/dep.rs` / Cargo.toml |
| WAITCNT.bit15 / load-path detect | unit green | header `$0143` ≠ SoC selector |
| Mode-8 / HALTCNT posture + CGB-AGB slot | unit green | FastHle default; LLE needs user firmware |
| CompatMachine wrap (load/run/FB/PCM/buttons) | unit green | no in-tree SM83 |
| Presentment / IO bridge (no L/R→FF00) | unit green | stretch stub |
| Blargg `01-special.gb` smoke | PASS when vendored | full boards → **P11** |
| Fixture LICENSE dirs | present | `tests/fixtures/blargg/`, `mooneye/` |
| Crate | **0.1.1** | patch after P9 `0.1.0` |

Stretch (non-blocking): CGB color path, soft-patch / bootlogo, BootRomLle overlay.

## P11 Compat accuracy (2026-09-12)

| Gate | Threshold / status | Notes |
|------|--------------------|-------|
| **G11-fixtures** | vendored | Blargg cpu_instrs + dmg_sound + cgb_sound; Mooneye acceptance + misc |
| **G11-dmg-cpu** | **12/12** recorded | Blargg `cpu_instrs` (11 individual + all-in-one) PASS via FastDmg; presence default; matrix `#[ignore]` |
| **G11-dmg-sound** | **13/13** recorded | Blargg `dmg_sound` singles + all-in-one PASS; matrix `#[ignore]` |
| **G11-dmg-mooneye** | **25/25** recorded | curated acceptance board; PASS=25 FAIL=0 TIMEOUT=0 UNSUPPORTED=0 |
| **G11-cgb-mode** | unit green | `CompatSilicon::FastCgb` via `bus_from_cartridge` |
| **G11-cgb-sound** | **13/13** recorded | Blargg `cgb_sound` FastCgb PASS; matrix `#[ignore]` |
| **G11-cgb-mooneye** | **4/4** recorded | Mooneye `misc/*-C` / boot_regs-cgb PASS via FastCgb |
| **G11-frontend** | unit green | `.gb`/`.gbc` picker + 8-bit `.sav`; dual HostSession |
| **G11-docs** | this board | README/CHANGELOG; crate **0.1.2** |
| **G11-stretch** | deferred | CGB HDMA / KEY1 |

```bash
cargo test --test roms g11_dmg_cpu_instrs_matrix -- --ignored --nocapture
cargo test --test roms g11_dmg_sound_matrix -- --ignored --nocapture
cargo test --test roms g11_dmg_mooneye_matrix -- --ignored --nocapture
cargo test --test roms g11_cgb_sound_matrix -- --ignored --nocapture
cargo test --test roms g11_cgb_mooneye_matrix -- --ignored --nocapture
```

jsmolka arm/thumb/memory remain default-CI PASS. P12 supersede cutover is next.
