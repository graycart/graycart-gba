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

- **P2 bus/memory (in progress):** region map + mirrors, WAITCNT waitstates, video STRB / open-bus stubs, DMA register file + Immediate (shared branch `dev/p2-bus`).
- **P0 scaffold (in progress):** fixture layout under `tests/fixtures/` with a
  mandatory upstream license table (jsmolka MIT, mGBA suite MIT, FuzzARM
  GPL-3.0, Tonc examples CC0). Suite dirs are stubs — **no** commercial ROMs,
  BIOS images, or conformance `.gba` binaries committed yet.
- Repository MIT `LICENSE` (Graycart source/docs only; fixtures keep upstream
  terms).
