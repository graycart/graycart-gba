<!--
Cited: jsmolka/gba-tests (MIT) PPU goldens
Note: final-frame SHA-256 of 240×160×3 RGB888 after idle + 3 frames.
First-party capture on graycart-gba P4 tip (no mGBA in CI image).
Rebake only with documented reference; never silent refresh beside behavior change.
-->
# jsmolka PPU goldens

| ROM | File |
|-----|------|
| hello | `hello.hash` |
| shades | `shades.hash` |
| stripes | `stripes.hash` |

Format:
```
frame=<N>
sha256=<64 hex>
```

Bake: `cargo test -p graycart-gba --test roms -- --ignored p4_bake`
