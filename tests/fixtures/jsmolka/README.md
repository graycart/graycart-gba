<!--
Cited: jsmolka/gba-tests
URL: https://github.com/jsmolka/gba-tests
Note: MIT prebuilts vendored for P1/P2 gates; Graycart MIT does not cover these ROMs.
-->
# jsmolka/gba-tests fixtures

**Status (workstream D/E + ARM PC+12):** BiosHle load + step + r12/idle oracle are wired.
**arm**, **thumb**, and **memory** assert **PASS** in default CI.

| Field | Value |
|-------|-------|
| Upstream | [jsmolka/gba-tests](https://github.com/jsmolka/gba-tests) |
| License | MIT (Copyright 2019 Julian Smolka) — see [`LICENSE`](LICENSE) |
| Pin | [`a7113b67e63f83a9b321696ddd7042ccfad6c881`](https://github.com/jsmolka/gba-tests/commit/a7113b67e63f83a9b321696ddd7042ccfad6c881) |
| Role | Primary early ARM/Thumb functional suite; also memory, PPU smoke, BIOS, saves |

## Pass criteria (exact)

Aligned with apparatus §4.3 / upstream `lib/macros.inc` `m_test_eval`:

| Signal | Meaning |
|--------|---------|
| **`r12 == 0` after idle** | **PASS** (authoritative). Aggregator clears `r12` then `m_exit` only on fail. |
| **`r12 == N` after idle** | **FAIL** test `N` (ARM blocks start at 1/50/100/…; Thumb/memory use their own numbering). |
| **IWRAM `[0],[4],[8]`** | Fail-path Div scratch (hundreds/tens/ones) — **not** a pass magic; only written when `r12 != 0` before drawing digits. Needs SWI `0x06` Div HLE. |
| **I/O `DISPCNT`** | After `m_test_init` → Mode 4 + BG2 (`0x0404`). Harness requires this before scoring idle so early hangs are TIMEOUT. |
| **LCD / Mode 4 VRAM** | Draws `"All tests passed"` or `"Failed test NNN"` via glyphs. Optional secondary: FB hash or glyph scan (not required while r12 works). |
| **Budget exhaust** | **TIMEOUT** (honest red) — never SKIPPED once the ROM is present. |

Launch: **BiosHle** soft entry `0x08000000` (no Nintendo BIOS in git). Fail-digit rendering needs Div HLE; the PASS path does not.

## Suite status (this tip)

| ROM | Default CI | Tip result |
|-----|------------|------------|
| `thumb/thumb.gba` | asserts **PASS** | executes end-to-end |
| `memory/memory.gba` | asserts **PASS** | executes (needs bus video STRB) |
| `arm/arm.gba` | asserts **PASS** | PC+12 Rs-shift + LDM/STM ^ / empty Rlist |

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

Harness matrix: [`tests/roms/jsmolka.rs`](../../roms/jsmolka.rs) — arm/thumb/memory
assert PASS in default CI. No fake green.

Not vendored yet: (none for P7 musts). PPU smoke
(`ppu/hello`, `shades`, `stripes`) is vendored for P4 goldens.
`bios` + `save/{none,sram,flash64,flash128}` vendored for P7 gates.
Do not claim Graycart MIT covers these ROMs. No Nintendo BIOS / commercial carts.
