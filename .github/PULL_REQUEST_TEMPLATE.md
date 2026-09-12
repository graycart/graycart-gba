<!--
Credits: adapted from graycart-gb `.github/PULL_REQUEST_TEMPLATE.md`
https://github.com/graycart/graycart-gb/blob/main/.github/PULL_REQUEST_TEMPLATE.md
-->

## Summary

<!-- What changed and why (1–3 sentences). -->

## Checklist

- [ ] `cargo fmt --check`
- [ ] `cargo clippy --all-targets --all-features -- -D warnings`
- [ ] `cargo test`
- [ ] No `carts/*.gba`, `carts/*.gb`, `.sav`, `.gcs*`, BIOS/boot firmware, or secrets committed
- [ ] No title-specific hacks; fix the owning subsystem when changing emulation
- [ ] File-top credit headers when citing external docs/code (see `ATTRIBUTION.md` / `AGENTS.md`)
