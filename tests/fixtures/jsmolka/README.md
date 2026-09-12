<!--
Cited: jsmolka/gba-tests
URL: https://github.com/jsmolka/gba-tests
Note: provenance stub only; no ROMs vendored in P0.
-->
# jsmolka/gba-tests fixtures

**Status (P1):** license + path stubs — no `.gba` binaries committed yet.

| Field | Value |
|-------|-------|
| Upstream | [jsmolka/gba-tests](https://github.com/jsmolka/gba-tests) |
| License | MIT (Copyright 2019 Julian Smolka) — see [`LICENSE`](LICENSE) |
| Role | Primary early ARM/Thumb functional suite; also memory, PPU smoke, BIOS, saves |

## Layout

```text
jsmolka/
├── LICENSE
├── README.md          # this file
├── arm/README.md      # expects arm.gba when vendored — P1 exit
└── thumb/README.md    # expects thumb.gba when vendored — P1 exit
```

Prefer upstream **prebuilt** `.gba` (FASMARM), same clone-reliability posture as
graycart-gb fixtures:

- [`arm/arm.gba`](arm/) — **P1 exit** gate
- [`thumb/thumb.gba`](thumb/) — **P1 exit** gate
- `memory/memory.gba` (+ mirrors / `video_strb`) — P2
- `ppu/{hello,shades,stripes}.gba` — P4 entry
- `bios/bios.gba` — P6
- `save/{sram,flash64,flash128,none}.gba` — P6

Pass oracle: Mode 4 text (`r12 == 0` → “All tests passed”; else “Failed test NNN”).

CI runs license/path presence tests only. ROM matrices are `#[ignore]` until
vendored + interpreter green. Do not claim Graycart MIT covers these ROMs once
they land.
