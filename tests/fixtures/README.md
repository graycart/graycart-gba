<!--
Cited: graycart-gb tests/fixtures/README.md (layout + license-table posture)
URL: https://github.com/graycart/graycart-gb/blob/main/tests/fixtures/README.md
Note: adapted for GBA suites from docs/graycart-gba/07-test-strategy.md §4–§7
  and 11-test-apparatus.md §3–§4; harness lives under tests/roms/.
-->
# Test ROM fixtures

Commercial carts stay under `carts/` (local only, gitignored). Nintendo BIOS /
`gba_bios.bin` is **never** committed. Conformance fixtures live here.

Shared runner + matrices: [`tests/roms/`](../roms/) (`harness.rs`,
`jsmolka.rs`, `mgba_suite.rs`, `nba_hw_test.rs`). jsmolka **arm** + **thumb** +
**memory** assert PASS in default CI — **no fake green**.

## Licensing / provenance

**These fixtures are not covered by this repository’s MIT license.**

Graycart’s root [`LICENSE`](../../LICENSE) applies to Graycart source and docs
only. Vendored test ROMs keep their **upstream** copyright and license terms.

| Suite | Path | Upstream | License | Notes |
|-------|------|----------|---------|-------|
| jsmolka/gba-tests | [`jsmolka/`](jsmolka/) | [jsmolka/gba-tests](https://github.com/jsmolka/gba-tests) | MIT | `arm`/`thumb`/`memory` prebuilts in-tree (pin in suite README); PPU/bios/save later |
| In-house (Graycart) | [`inhouse/`](inhouse/) | this repo | MIT | P3 **G3-irq-rom** simple IRQ stub (`simple-irq/`); no BIOS |
| mGBA suite | [`mgba-suite/`](mgba-suite/) | [mgba-emu/suite](https://github.com/mgba-emu/suite) | MIT | LICENSE stub + ignored harness row; P3 stretch `io-read` / `timer-irq` stubs; no `suite.gba` yet |
| NBA hw-test | [`nba-hw-test/`](nba-hw-test/) | [nba-emu/hw-test](https://codeberg.org/nba-emu/hw-test) | BSD-3-Clause | LICENSE stub + ignored stretch row; no binaries |
| FuzzARM | [`fuzzarm/`](fuzzarm/) | [DenSinH/FuzzARM](https://github.com/DenSinH/FuzzARM) | **GPL-3.0** | Do not relicense; prefer download/submodule over copying generator source |
| Tonc examples | [`tonc/`](tonc/) | [gbadev-org/libtonc-examples](https://github.com/gbadev-org/libtonc-examples) | CC0-1.0 | Golden-frame demos (P4+); tutorial text is CC BY-NC-SA — cite, don’t paste |

### Secondary (not stubbed as ROM trees yet)

| Asset | Role | License | Caveat |
|-------|------|---------|--------|
| [SingleStepTests/ARM7TDMI](https://github.com/SingleStepTests/ARM7TDMI) | JSON single-step CPU vectors | MIT | Secondary oracle (NBA-generated); not HW truth |
| [destoer/armwrestler-gba-fixed](https://github.com/destoer/armwrestler-gba-fixed) | Visual ARM grid | upstream | Some LDM writeback cases fail on real HW |
| [cajunpanda/gba-audio-test](https://github.com/cajunpanda/gba-audio-test) | PSG + DirectSound path | MIT | Soft audio gate until dump policy exists |

Do not add commercial cartridges or BIOS dumps under this tree.

## Layout

```text
tests/
├── fixtures/
│   ├── README.md          # this license table (mandatory)
│   ├── jsmolka/           # MIT — LICENSE + arm/thumb/memory .gba (pinned)
│   ├── mgba-suite/        # MIT — LICENSE stub; suite.gba later
│   ├── nba-hw-test/       # BSD-3 — LICENSE stub; curated ROM later
│   ├── fuzzarm/           # GPL-3.0 notice — empty until opted-in ROMs
│   └── tonc/              # CC0 — empty until selected demos
└── roms/
    ├── harness.rs         # Outcome + RomLaunchMode + GbaTestRom
    ├── jsmolka.rs         # arm/thumb/memory (PASS in default CI)
    ├── mgba_suite.rs      # ignored SoC-depth stub
    ├── nba_hw_test.rs     # ignored stretch stub
    └── main.rs            # CI-blocking apparatus smoke
```

Default `cargo test` runs jsmolka arm/thumb/memory gates (vendored MIT prebuilts).
Additional conformance matrices (`mgba_suite`, `nba_hw_test`, full printout) stay
`#[ignore]` / `cargo test -- --ignored` opt-in — not default CI, no nightly yet.

Outcomes: `PASS` / `FAIL` / `TIMEOUT` / `UNSUPPORTED` / `SKIPPED`
(apparatus or absent ROM).
