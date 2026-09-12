<!--
Cited: PHASES.md P8; 07-test-strategy.md §6; 11-test-apparatus.md §2; mgba-emu/suite
Note: living accuracy board — update thresholds when suite automation lands.
-->
# graycart-gba conformance board

Recorded pass thresholds for SoC-depth suites. Default CI does **not** run
`--ignored` rows. jsmolka arm/thumb/memory remain the default-CI PASS spine.

## P8 Timing (2026-09-12)

| Suite | Threshold (passes) | Status | Notes |
|-------|--------------------|--------|-------|
| mGBA **timing** | **0** | not run | `suite.gba` absent; unit G8-prefetch / disable / irq-delay / cycles green |
| mGBA **dma** | **0** | not run | same fixture gap; unit G8-dma-delay green |
| Full mGBA suite board | — | stretch | `#[ignore]` until pinned prebuilt + headless oracle |

Launch mode for future suite runs: **BiosHle** (record BIOS vs no-BIOS if that
changes timing numbers).

Update this table when `tests/fixtures/mgba-suite/suite.gba` is deliberately
vendored and `tests/roms/p8.rs` asserts `passes >= threshold`.
