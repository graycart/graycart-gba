# Contributing to Graycart (`graycart-gba`)

Game Boy Advance emulator in the [Graycart family](https://github.com/graycart/graycart).

Engineering norms: [`AGENTS.md`](./AGENTS.md). File-header credits: [`ATTRIBUTION.md`](./ATTRIBUTION.md).

This tree is a greenfield rewrite at **0.0.1**. There is no machine to regress yet.

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Do not commit commercial ROMs, `.sav` / state files, or Nintendo BIOS / boot firmware. Local dumps belong under `carts/` (gitignored) or stay outside the tree.

Code in this repository is MIT (see [`LICENSE`](./LICENSE)).
