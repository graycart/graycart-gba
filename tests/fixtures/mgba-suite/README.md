<!--
Cited: mgba-emu/suite
URL: https://github.com/mgba-emu/suite
Note: provenance stub only; no suite.gba in P0.
-->
# mGBA suite fixtures

**Status:** LICENSE/path stub + ignored harness row in `tests/roms/mgba_suite.rs`
— no `suite.gba` committed yet (next SoC-depth gate after jsmolka).

| Field | Value |
|-------|-------|
| Upstream | [mgba-emu/suite](https://github.com/mgba-emu/suite) |
| License | MIT (Copyright 2015 Jeffrey Pfau / endrift) — see [`LICENSE`](LICENSE) |
| Role | System / timing accuracy board (memory, DMA, timers, shifter, video, …) |

## Planned contents

- Built `suite.gba` (devkitARM + libgba) **or** a documented build script
- Prefer vendored prebuilt for clone reliability once license copy is retained

Sub-suites of interest: shifter / carry / multiply-long (P1–P2), memory / DMA
(P2–P7), timers / IRQ (P3→P7), timing (P7), BIOS math (P6), video (P4→P7).

Headless automation is TBD upstream (UI-driven today); prefer SRAM `savprintf`
or mGBA debug `BEGIN`/`END` lines when wired.

Do not claim Graycart MIT covers suite binaries once they land.
