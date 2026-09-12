<!--
Cited: gbadev-org/libtonc-examples
URL: https://github.com/gbadev-org/libtonc-examples
Note: provenance stub only; no demo ROMs in P0.
Tutorial text at https://gbadev.net/tonc/ is CC BY-NC-SA 4.0 — cite, don’t paste.
-->
# Tonc example fixtures

**Status (P0):** directory stub — no `.gba` demos committed yet.

| Field | Value |
|-------|-------|
| Upstream demos | [gbadev-org/libtonc-examples](https://github.com/gbadev-org/libtonc-examples) |
| License (demos) | CC0-1.0 — see [`LICENSE`](LICENSE) |
| Tutorial (cite only) | [gbadev.net/tonc](https://gbadev.net/tonc/) — CC BY-NC-SA 4.0 |
| Role | Visual / PPU golden frames (P4+) |

## Planned selection

Prefer demos that stress distinct PPU paths (exact names verified at vendor time):

1. Mode 3 bitmap  
2. Mode 4 pageflip  
3. Regular sprites / OAM  
4. Affine BG or sprite  
5. Blend / window  

Automation = fixed `--frames N` + framebuffer hash (not a pass/fail string suite).

Do not wholesale copy Tonc tutorial prose into this tree.
