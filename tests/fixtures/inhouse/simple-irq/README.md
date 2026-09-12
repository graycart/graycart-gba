<!--
Cited: graycart-gba PHASES P3 exit / 11-test-apparatus §4 (simple IRQ ROM)
  Project store: docs/graycart-gba/PHASES.md, docs/graycart-gba/11-test-apparatus.md
Note: in-house fixture stub — no .gba yet; harness row is #[ignore] until present.
-->
# In-house simple IRQ ROM (P3 gate)

**Status:** path/LICENSE stub only — **no `.gba` committed yet**.  
**Phase gate:** **G3-irq-rom** (P3 must) — harness: `tests/roms/p3.rs`  
**License:** Graycart-authored fixture will be MIT (same as crate) when added.

## Intended oracle (when ROM lands)

Minimal homebrew that:

1. Enables IME + a timer (or keypad) IRQ in IE.
2. Acknowledges IF (W1C).
3. Signals PASS via agreed harness (prefer r12/`0` idle like jsmolka, or IWRAM flag — document here when chosen).

Do **not** commit Nintendo BIOS or commercial ROMs. Until the binary exists, the harness row stays `#[ignore]` (honest — not fake green).
