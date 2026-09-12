# graycart-gba

Game Boy Advance emulator in the [Graycart family](https://github.com/graycart/graycart).

**Goal:** native GBA software, plus **DMG/CGB** via the GBA hardware compatibility path (prefer reusing [graycart-gb](https://github.com/graycart/graycart-gb)). This repo will eventually supersede graycart-gb as the shipping **app**; GBA-native cores stay greenfield.

## Status

Phase 0 scaffold: crate `graycart-gba` **0.0.1** (lib + headless bin stubs), CI, fixtures stubs. No subsystem behavior yet - do not start P1 CPU work until Phase 0 exit.

| Doc | What |
|-----|------|
| [`AGENTS.md`](./AGENTS.md) | How agents navigate: build/test, modules, SemVer, no BIOS/ROMs in git, parallel ownership |
| [`ATTRIBUTION.md`](./ATTRIBUTION.md) | **Mandatory** file-header credit rule (what / URL / inspired-by note) |
| [`CONTRIBUTING.md`](./CONTRIBUTING.md) | Short contributor blurb |

Research, phases, and provenance live in the Graycart Project store under `docs/graycart-gba/` until linked or copied here. Family core API: Project store `docs/graycart-family/` (especially `01-core-api.md`).

## Build / test

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Default CI stays green without BIOS images, commercial ROMs, or `cargo test -- --ignored`.

## SemVer

`Cargo.toml` `[package].version` is the only product version; git tag `vX.Y.Z` must equal it. Scaffold is **0.0.1**; first tagged runnable milestone targets **0.1.0** (Project store `08-implementation-plan.md` section 5). Do not ship `1.0.0` until native GBA (and agreed compat) are stable enough.

## Legal / dumps

- Never commit Nintendo BIOS/boot firmware or commercial ROMs.
- Local dumps belong under `carts/` (gitignored) or outside the tree.
- Code is MIT (see [`LICENSE`](./LICENSE)). Fixture suites keep upstream licenses.

## License

[MIT](LICENSE) - Copyright (c) 2026 Graycart.
