# Conformance ROMs

Accuracy fixtures live under [`tests/fixtures/`](../tests/fixtures/). Commercial dumps stay in `carts/` (gitignored).

## Suites

| Suite | Path | Upstream | How scored |
|-------|------|----------|------------|
| jsmolka gba-tests | `jsmolka/` | [jsmolka/gba-tests](https://github.com/jsmolka/gba-tests) @ `a7113b67` | **Must-gate.** Idle + `r12==0` (thumb: `r7==0`); PPU frame hashes |
| alyosha-tas/gba-tests | `alyosha/` | [alyosha-tas/gba-tests](https://github.com/alyosha-tas/gba-tests) @ `66f5f1d6` | Ignored matrix; same r12/r7 protocol |
| png183/gba-tests | `png183/` | [png183/gba-tests](https://github.com/png183/gba-tests) @ `87937453` | Ignored matrix; same protocol |
| nba-emu/hw-test | `nba/` | [nba-emu/hw-test](https://codeberg.org/nba-emu/hw-test) @ `fbc99140` | Ignored matrix; MGBA printf when used, else on-screen console ? UNSUPPORTED |
| Hades-Tests | `hades/` | [hades-emu/Hades-Tests](https://github.com/hades-emu/Hades-Tests) @ `29274ce3` | Ignored matrix; libgba on-screen console ? UNSUPPORTED until tile scrape |
| FuzzARM | `fuzzarm/` | [DenSinH/FuzzARM](https://github.com/DenSinH/FuzzARM) @ `a675329c` | Presence only (interactive) |
| mGBA suite | `mgba-suite/` | [mgba-emu/suite](https://github.com/mgba-emu/suite) @ `e6942030` | Source only until `suite.gba` exists |

`unsafe/` trees are excluded from matrices (upstream: fail on hardware / flash-card noise).

## Commands

```bash
# Always-on: fixture presence + jsmolka must-gate (CPU/bus/BIOS/save/PPU hashes)
cargo test --test roms

# jsmolka only, with rows
cargo test --test roms jsmolka_must_matrix -- --nocapture

# Edge suites (expect FAIL until accuracy catches up)
cargo test --test roms alyosha_matrix -- --ignored --nocapture
cargo test --test roms png183_matrix -- --ignored --nocapture
cargo test --test roms nba_matrix -- --ignored --nocapture
cargo test --test roms hades_matrix -- --ignored --nocapture
```

Outcomes: `PASS` / `FAIL` / `TIMEOUT` / `UNSUPPORTED`.
