<!--
Cited: graycart-gb CHANGELOG.md (SemVer / Unreleased posture)
URL: https://github.com/graycart/graycart-gb/blob/main/CHANGELOG.md
Note: P0 scaffold stub; versioning follows graycart-gba plan (0.1.0 at first tagged milestone).
-->
# Changelog

All notable changes to this project will be documented in this file.

The format is inspired by [Keep a Changelog](https://keepachangelog.com/), and
this project follows [Semantic Versioning](https://semver.org/) on the `0.x`
line (crate version = product version), matching graycart-gb posture.

Initial tagged runnable milestone target: **`0.1.0`**. Intermediate merges may
stay untagged until a playable or gate-complete slice ships.

## Unreleased

### Added

- **P11 Compat accuracy (`walter/p11-compat-accuracy-398a`):** FastCgb launch on
  `CompatMachine`; vendored Blargg cpu_instrs/dmg_sound/cgb_sound + Mooneye
  acceptance/misc; P11 DMG/CGB ignored suite matrices + recorded thresholds;
  frontend loads `.gb`/`.gbc` with 8-bit `.sav` UX; docs/conformance P11 board;
  crate **0.1.2**. Unit gates G11-*; stretch HDMA/KEY1 ignored. jsmolka
  arm+thumb+memory stay default-CI PASS. P12 supersede cutover still deferred.
- **P10 DMG/CGB compat bring-up (`walter/p10-compat-8eeb`):** interim whole-crate
  `graycart` dep; WAITCNT.bit15 / load-path detect; Mode-8/HALTCNT handoff +
  CGB-AGB user-supplied boot slot; `CompatMachine` wrapper (load/run/FB/PCM/
  buttons/battery); IO bridge (L/R → stretch, not `FF00`); Blargg
  `01-special.gb` smoke + fixture LICENSE dirs; docs/conformance P10 board;
  crate **0.1.1**. Unit gates G10-*; full Blargg/Mooneye boards deferred to P11.
  jsmolka arm+thumb+memory stay default-CI PASS.
- **P9 Frontend (`walter/p9-frontend-ca20`):** windowed host via eframe
  (winit/wgpu/egui) + cpal; ROM picker, pause/reset, `.sav` sidecar flush;
  lib host seam (`set_buttons`, `battery_sav` / `load_battery_sav`); headless
  `--frames` path unchanged; `carts/` smoke skip-if-missing; docs/conformance
  P9 board; crate **0.1.0**. Unit gates G9-*; GUI window smoke `#[ignore]`.
  jsmolka arm+thumb+memory stay default-CI PASS. DMG/CGB UX deferred to P10+.
- **P8 Timing (`walter/p8-timing-e812`):** Game Pak prefetch FSM (8×16 fill/drain/
  hit); Prefetch Disable Bug latch; IRQ recognition delay (7 cycles, Halt wake
  immediate); coarse N/S/I waitstate step + `run_cycles` as cycle budget; DMA
  Enable 0→1 startup delay (2 cycles) + post-DMA force-N. Unit gates G8-*;
  `docs/conformance.md` records mGBA timing/dma thresholds (0 / not run until
  `suite.gba`); `tests/roms/p8.rs` hygiene + `#[ignore]` matrices. jsmolka
  arm+thumb+memory stay default-CI PASS.
- **P7 Cart/BIOS/saves (`walter/p7-cart-bios-saves-c42d`):** optional BiosLle
  (`GBA_BIOS` / `load_bios`, never vendored); PC-gated BIOS open-bus latch
  (jsmolka SoftReset/SWI/IRQ residues); BiosHle SoftReset/Sqrt/CpuSet + IRQ
  trampoline with register save; cart header parse; save detect + SRAM/Flash/
  EEPROM backends + `.sav` round-trip; MachineMem routes backup window.
  Vendored MIT jsmolka `bios` + `save/{none,sram,flash64,flash128}` assert
  PASS in default CI. Unit gates G7-*; LLE stretch `#[ignore]`.
  jsmolka arm+thumb+memory stay default-CI PASS.
- **P6 APU (shared `walter/p6-apu-052e`):** SOUNDCNT_* / SOUNDBIAS / Wave RAM
  register file with master-enable clear of PSG `060`–`081`; PSG ch1–4 (AGB
  wave dual-bank + 75% volume); FIFO A/B with TM0/TM1 sample clock + half-empty
  DMA1/2 request; digital mixer + PWM truncate; host PCM ring + soft WAV
  (`--audio-out`). MMIO routes sound ports; DMA dest→FIFO feeds APU. Unit
  gates G6-regs/psg/fifo/mixer/pcm/glue; `tests/roms/p6.rs` +
  `tests/fixtures/gba-audio-test/` stubs (`#[ignore]` until ROM/golden).
  jsmolka arm+thumb+memory stay default-CI PASS.
- **P5 DMA full (shared `walter/p5-dma-full-980e`):** VBlank / HBlank starts (HBlank
  Interval Free for OAM), DMA1/2 Special FIFO 4×32-bit refill, DMA3 Special
  video-capture window stub (VCOUNT 2..=161), completion IRQs, DMA3 Game Pak
  yes / SRAM never. `Gba` step hooks PPU edges + APU FIFO request stub; MMIO
  routes `040000B0`–`DF`. Unit gates G5-*; mGBA dma progress logged
  (`tests/roms/p5.rs`); alyosha DMA stubs `#[ignore]`. jsmolka
  arm+thumb+memory stay default-CI PASS.
- **P4 PPU (shared `dev/p4-ppu`):** scanline timing 1232×228; DISPSTAT
  VBlank/HBlank/VCounter + IRQs; LCD I/O register file; modes 0–5 + OBJ
  basics; windows/blend/affine functional; headless `--frames N --hash-out`
  (SHA-256 of 240×160 RGB888). Vendored jsmolka MIT `ppu/{hello,shades,stripes}`
  + golden hash path; Tonc ≥3 demos stay `#[ignore]` until CC0 binaries land.
  jsmolka arm+thumb+memory stay default-CI PASS.
- **P3 kickoff (timers / IRQ / input):** shared branch `dev/p3-timers-irq`;
  harness gates wired early (`tests/roms/p3.rs`) — in-house simple IRQ ROM
  stub + mGBA `io-read` / `timer-irq` stretch stubs (`#[ignore]` until
  fixtures). Unit gates (timers / IE·IF·IME / Halt wake / KEYINPUT) land on
  exclusive streams. `Gba` step wires halt wake → timer/keypad → IRQ sample;
  `mmio::MachineMem` dispatches IE/IF/IME, timers, KEY*, HALTCNT/POSTFLG/
  WAITCNT/SIO. jsmolka arm+thumb+memory stay default-CI PASS.
- **Suite apparatus harden:** README/CI matrix documents that default `ci.yml`
  (ubuntu/macOS/windows, `fail-fast: false`) asserts jsmolka **arm+thumb+memory
  PASS**; `--ignored` stays opt-in (no nightly job yet). Stubbed ignored rows +
  LICENSE paths for next SoC-depth gates: `tests/roms/mgba_suite.rs` (MIT) and
  `tests/roms/nba_hw_test.rs` + `tests/fixtures/nba-hw-test/` (BSD-3-Clause).
  No large binaries vendored.

### Fixed

- **jsmolka `arm.gba` P1 gate (honest green):** ARM7TDMI PC+12 when R15 is
  Rn/Rm under a register-specified shift (`mov r0, pc, lsl r0` / #224–225);
  S=1 + Rd=15 restores SPSR even for TST/TEQ/CMP/CMN without flushing (#234–235);
  LDM/STM `^` user-bank transfers, empty Rlist ±0x40, and ARMv4 STM base-in-rlist
  NEW/OLD base rules. Default CI asserts arm+thumb+memory PASS.

### Previously

- **jsmolka load+oracle (workstream D/E):** BiosHle cart load + r12/idle oracle;
  default CI asserts thumb+memory (then arm after PC+12 fix).
- **jsmolka MIT prebuilts (workstream C):** vendored
  `tests/fixtures/jsmolka/{arm,thumb,memory}/*.gba` pinned to upstream
  `a7113b67e63f83a9b321696ddd7042ccfad6c881` + `fetch.sh`; LICENSE intact.
  No BIOS / commercial ROMs.
- **Test harness scaffolding (apparatus only):** shared
  `tests/roms/{harness,jsmolka,main}.rs` with `GbaTestRom` + outcomes
  (`PASS`/`FAIL`/`TIMEOUT`/`UNSUPPORTED`/`SKIPPED`); LICENSE/path stubs.
- **P2 bus/memory:** region map + mirrors, WAITCNT waitstates, video STRB /
  open-bus stubs, DMA register file + Immediate.
- **P0 scaffold:** fixture layout under `tests/fixtures/` with a mandatory
  upstream license table (jsmolka MIT, mGBA suite MIT, FuzzARM GPL-3.0, Tonc
  examples CC0). Repository MIT `LICENSE` (Graycart source/docs only; fixtures
  keep upstream terms).
