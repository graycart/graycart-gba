<!--
Cited: nba-emu/hw-test (GitHub archive) + Codeberg active mirror
URL: https://github.com/nba-emu/hw-test
URL: https://codeberg.org/nba-emu/hw-test
Note: LICENSE/path stub only — no binaries; next SoC-depth stretch after mGBA suite.
-->
# NBA hw-test fixtures

**Status:** directory stub — **no** `.gba` binaries committed (sources need
devkitARM; avoid huge trees unless a small MIT/BSD prebuilt is curated later).

| Field | Value |
|-------|-------|
| Upstream (active) | [codeberg.org/nba-emu/hw-test](https://codeberg.org/nba-emu/hw-test) |
| Upstream (GitHub archive) | [nba-emu/hw-test](https://github.com/nba-emu/hw-test) |
| License | **BSD-3-Clause** (Copyright 2021, fleroviux) — see [`LICENSE`](LICENSE) |
| Role | Bus/DMA/IRQ/PPU/timer/HALTCNT edge corpus — stretch after mGBA suite progress ([11-test-apparatus](https://github.com/graycart/graycart) Project store) |

## Planned contents (not yet)

Selected small prebuilts **or** a fetch script + pin — never wholesale archive dumps.
Subdirs of interest upstream: `bus/`, `dma/`, `irq/`, `ppu/`, `timer/`, `haltcnt/`.

Harness: ignored matrix in `tests/roms/nba_hw_test.rs` until a curated ROM +
oracle land. Default CI must not require these binaries.

Do not claim Graycart MIT covers hw-test binaries once they land.
