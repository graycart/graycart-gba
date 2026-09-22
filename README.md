# graycart-gba

Game Boy Advance emulator in the [Graycart family](https://github.com/graycart/graycart).

Greenfield rewrite. The previous 0.1.x machine was removed from this working tree. Nothing here runs a ROM yet.

Peer library for DMG/CGB: [graycart-gb](https://github.com/graycart/graycart-gb). Reuse that crate for 8-bit behavior when a compatibility path exists. Do not reimplement the SM83 machine inside this repo.

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Local dumps belong under `carts/` and stay untracked. Never commit commercial ROMs or Nintendo BIOS.

MIT — Copyright (c) 2026 Graycart.
