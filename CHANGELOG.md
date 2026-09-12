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
