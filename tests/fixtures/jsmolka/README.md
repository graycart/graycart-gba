<!--
Cited: jsmolka/gba-tests
URL: https://github.com/jsmolka/gba-tests
Note: provenance stub only; no ROMs vendored in P0.
-->
# jsmolka/gba-tests fixtures

**Status (P0):** directory stub — no `.gba` files committed yet.

| Field | Value |
|-------|-------|
| Upstream | [jsmolka/gba-tests](https://github.com/jsmolka/gba-tests) |
| License | MIT (Copyright 2019 Julian Smolka) — see [`LICENSE`](LICENSE) |
| Role | Primary early ARM/Thumb functional suite; also memory, PPU smoke, BIOS, saves |

## Planned layout (when vendored)

Prefer upstream **prebuilt** `.gba` (FASMARM), same clone-reliability posture as
graycart-gb fixtures:

- `arm/arm.gba` — **P1 exit** gate
- `thumb/thumb.gba` — **P1 exit** gate
- `memory/memory.gba` (+ mirrors / `video_strb`) — P2
- `ppu/{hello,shades,stripes}.gba` — P4 entry
- `bios/bios.gba` — P6
- `save/{sram,flash64,flash128,none}.gba` — P6

Pass oracle: Mode 4 text (`r12 == 0` → “All tests passed”; else “Failed test NNN”).

Do not claim Graycart MIT covers these ROMs once they land.
