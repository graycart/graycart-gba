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

- **jsmolka MIT prebuilts (workstream C):** vendored
  `tests/fixtures/jsmolka/{arm,thumb,memory}/*.gba` pinned to upstream
  `a7113b67e63f83a9b321696ddd7042ccfad6c881` + `fetch.sh`; LICENSE intact.
  Ignored matrices still return **SKIPPED** until load+oracle (D/E). No BIOS /
  commercial ROMs.
- **Test harness scaffolding (apparatus only — no accuracy claim):** shared
  `tests/roms/{harness,jsmolka,main}.rs` with `GbaTestRom` + outcomes
  (`PASS`/`FAIL`/`TIMEOUT`/`UNSUPPORTED`/`SKIPPED`); ignored jsmolka
  arm/thumb/memory matrix; LICENSE/path stubs kept; optional
  `tests/fixtures/jsmolka/fetch-sketch.sh` (does not download). Default CI
  stays ROM-free (no `--ignored`).
- **P2 bus/memory (in progress):** region map + mirrors, WAITCNT waitstates, video STRB / open-bus stubs, DMA register file + Immediate (shared branch `dev/p2-bus`).
- **P0 scaffold (in progress):** fixture layout under `tests/fixtures/` with a
  mandatory upstream license table (jsmolka MIT, mGBA suite MIT, FuzzARM
  GPL-3.0, Tonc examples CC0). No commercial ROMs or BIOS images.
- Repository MIT `LICENSE` (Graycart source/docs only; fixtures keep upstream
  terms).
