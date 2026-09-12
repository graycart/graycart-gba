<!--
Cited: product-decision-supersede-gb.md (Dave, 2026-09-12, amended)
  Project store: `internal/graycart-gba/product-decision-supersede-gb.md`
Cited: PHASES.md P12 · 08-implementation-plan.md §3.13
  Project store: `docs/graycart-gba/PHASES.md`
Note: dual-run → default-gba cutover; supersede the app, keep lib reuse.
-->
# Supersede cutover (P12)

**Status:** cutover documented (P12).  
**Authority:** Dave product decision 2026-09-12 (amended same day) — see Project store
`internal/graycart-gba/product-decision-supersede-gb.md`.

## What changes

| Role | Before (P11) | After (P12) |
|------|--------------|-------------|
| **Daily play host (8-bit + GBA)** | graycart-gb for DMG/CGB; graycart-gba for GBA (+ compat) | **graycart-gba** for both |
| **graycart-gb app** | Daily DMG/CGB development / play target | Leaves daily-target status — maintenance / lib host |
| **graycart-gb library (`graycart` crate)** | Source of SM83/PPU/APU/cart | **Stays** — intentional dep of graycart-gba compat |
| **GBA-native cores** | Greenfield in graycart-gba | Unchanged |

Supersede the **app**, not the library. Do not remove the whole-crate `graycart`
dependency to “clean up”; a future `gb-core` extract (S12) may narrow the dep
without changing this product default.

## Dual-run → default-gba

1. **Dual-run (through P11):** both binaries can play 8-bit carts. graycart-gb
   remained the daily DMG/CGB target while gba grew native + compat accuracy.
2. **Default-gba (P12):** recommend **graycart-gba** for new 8-bit **and** GBA
   play. Frontend Help/empty copy claims the shipping dual host.
3. **Maintenance:** graycart-gb stays available for SM83/core work, historical
   packaging, and as the dependency pin until/unless S12 extract lands.
4. **Family / linux:** keep preferring the library (or future `*-core`) for
   non-desktop hosts — cutover does not invent a second SM83.

## Dave-signed cutover

This document records the cutover under the existing product decision:

> Ship both GBA software and 8-bit DMG/CGB; eventually supersede graycart-gb as
> the shipping **app**; prefer reuse for DMG/CGB; GBA-native stays greenfield.

No second conflicting policy. Stretch (non-blocking): archive / stop packaging
the graycart-gb desktop binary for *new* 8-bit play while keeping the lib.

## Tree audit (intentional reuse)

| Check | Expectation |
|-------|-------------|
| `Cargo.toml` `graycart` git dep | Present (whole-crate interim OK) |
| `src/compat/dep.rs` pin | Matches Cargo.toml rev |
| In-tree SM83/PPU/APU reinvent | Forbidden |
| `eframe` / `egui` / `cpal` / `rfd` in `src/compat/**` or other lib cores | Forbidden — host-only |
| Default CI jsmolka arm+thumb+memory | Stay **PASS** |

## SemVer

P12 ships as crate **0.1.3** (patch: product cutover docs + UX copy). Git tag
`v0.1.3` must equal the crate version when tagged.
