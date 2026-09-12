<!--
Cited: jsmolka/gba-tests
URL: https://github.com/jsmolka/gba-tests
Note: MIT prebuilts vendored for P1/P2 gates; Graycart MIT does not cover these ROMs.
-->
# jsmolka/gba-tests fixtures

**Status (workstream C):** MIT prebuilts for `arm` / `thumb` / `memory` are
committed in-tree (small). Matrices stay `#[ignore]` until load+oracle (D/E).

| Field | Value |
|-------|-------|
| Upstream | [jsmolka/gba-tests](https://github.com/jsmolka/gba-tests) |
| License | MIT (Copyright 2019 Julian Smolka) — see [`LICENSE`](LICENSE) |
| Pin | [`a7113b67e63f83a9b321696ddd7042ccfad6c881`](https://github.com/jsmolka/gba-tests/commit/a7113b67e63f83a9b321696ddd7042ccfad6c881) |
| Role | Primary early ARM/Thumb functional suite; also memory, PPU smoke, BIOS, saves |

## Vendored binaries (P1/P2 musts)

| Path | Size | sha256 |
|------|------|--------|
| [`arm/arm.gba`](arm/arm.gba) | 8824 | `77ee88662552bdc885c1080c0172ff119d54db791bd73b21808cf1ff1fe5b40e` |
| [`thumb/thumb.gba`](thumb/thumb.gba) | 3680 | `b5cb2291df4ab314b31c598acd9bff2ccfa0b38efff29daadfe97422ce369b67` |
| [`memory/memory.gba`](memory/memory.gba) | 2172 | `21024fb6aae6343f5f0466dd54e3149de1fbeb23f78e7d85a015c983684d2f87` |

Re-fetch (optional): [`fetch.sh`](fetch.sh) — `JSMOLKA_REF=<sha> ./fetch.sh`.
Legacy sketch: [`fetch-sketch.sh`](fetch-sketch.sh) (prints curl examples only).

## Layout

```text
jsmolka/
├── LICENSE
├── README.md
├── fetch.sh / fetch-sketch.sh
├── arm/{README.md,arm.gba}       # P1 exit
├── thumb/{README.md,thumb.gba}   # P1 exit
└── memory/{README.md,memory.gba} # P2 exit
```

Harness matrix: [`tests/roms/jsmolka.rs`](../../roms/jsmolka.rs) — `#[ignore]`;
stub runner still returns **SKIPPED** (no accuracy claim until D/E).

Not vendored yet: `ppu/*`, `bios`, `save/*` (later phases). Do not claim
Graycart MIT covers these ROMs. No Nintendo BIOS / commercial carts.
