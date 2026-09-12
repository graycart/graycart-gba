# Contributing to Graycart (`graycart-gba`)

Thanks for helping with the Game Boy Advance emulator in the [Graycart family](https://github.com/graycart/graycart).

For engineering norms (project goal, hardware-first, **file-header attribution**, SemVer, validation), see [`AGENTS.md`](./AGENTS.md).

## Current status

P1/P2 jsmolka gates (**arm** / **thumb** / **memory**) assert **PASS** on default
CI (`0.0.1`). Deeper suites are ignored stubs. Design and phase gates live in the
Graycart Project research pack (`docs/graycart-gba/`). Do not start **P3+** until
those gates stay green and the phase freeze in Project store `12-test-gates.md`
allows it.

## Validation

Rust stable (see CI). Before calling a change done:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

CI matrix: ubuntu / macOS / windows (`fail-fast: false`). Default `cargo test`
must keep jsmolka arm+thumb+memory green. Do **not** add `cargo test -- --ignored`
to the default workflow.

Do **not** commit commercial ROMs, `.sav` / state files, or Nintendo BIOS / boot firmware. Local dumps belong under `carts/` (gitignored) or stay outside the tree.

## Attribution

If your change references external docs or other projects' code, credit them **at the top of the file** you edit -- see [`ATTRIBUTION.md`](./ATTRIBUTION.md) and the Attribution section in [`AGENTS.md`](./AGENTS.md). Provenance indexes alone are not enough.

## License

Code in this repository is MIT (see [`LICENSE`](./LICENSE)). Test fixtures keep their **upstream** licenses and are **not** covered by the repo MIT grant.
