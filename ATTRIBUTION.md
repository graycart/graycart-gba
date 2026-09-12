# Attribution — graycart-gba

Mandatory for in-repo Rust and Markdown that depend on external code or documentation.

Also stated in [`AGENTS.md`](./AGENTS.md). Provenance archives (when present under `docs/` or research packs) remain required for research excerpts — they do **not** replace per-file credits on sources that use those materials.

## Rule

If a file references someone else’s **code** or **internet documentation** (GBATEK, Pan Docs, ARM TRM, blogs, another emulator, SDK notes, etc.), put **credit at the top of that file**:

| Field | Required |
|-------|----------|
| What | Document / project / section used |
| Where | URL and/or canonical name |
| How | Brief note: *cited* / *inspired by* / *ported from* / *cross-checked against* |

Applies to:

- Rust modules (`//!` module docs or a short header comment)
- Markdown in this repo when the page quotes, ports, or structurally follows an external source

Does **not** replace:

- Minimal excerpts + license/terms in provenance folders
- Fixture LICENSE tables under `tests/fixtures/` (when the crate exists)

Never commit full manuals, commercial ROMs, or Nintendo BIOS/boot images.

## Example — Rust (`.rs`)

```rust
//! Prefetch buffer fill/drain stub for Game Pak ROM.
//!
//! Cited: GBATEK — Gamepak Prefetch Buffer
//!   https://problemkaputt.de/gbatek.htm
//! Cross-check: NanoBoyAdvance prefetch notes (secondary; behavior TBD until P8).
```

## Example — Markdown (`.md`)

HTML comment (keeps the visible title clean):

```markdown
<!--
Cited: GBATEK — LCD Video Controller
URL: https://problemkaputt.de/gbatek.htm
Note: timing numbers for HDraw/HBlank; not a full reprint.
-->
# PPU timings
```

Or a visible header block under the title:

```markdown
# Interrupt control notes

title: GBATEK Interrupt Control
URL: https://problemkaputt.de/gbatek.htm
retrieved: 2026-09-12
license/terms: GBATEK site terms — minimal excerpt only
why cited: IE/IF/IME bit layout
```

## Checklist before coding a behavior change

1. Name the primary reference (usually GBATEK) and the acceptance test.
2. Add or update the **file-top credit** in every touched source that relied on that material.
3. If you paste or closely paraphrase an excerpt into research docs, also land a provenance file with the standard header block.

## Related

- [`AGENTS.md`](./AGENTS.md) — full agent norms
- [`CONTRIBUTING.md`](./CONTRIBUTING.md) — short contributor blurb
