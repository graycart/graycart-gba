<!--
Cited: DenSinH/FuzzARM
URL: https://github.com/DenSinH/FuzzARM
Note: GPL-3.0 fixture notice; no ROMs or generator source in P0.
-->
# FuzzARM fixtures

**Status (P0):** directory stub — no `.gba` files and **no generator source**
committed.

| Field | Value |
|-------|-------|
| Upstream | [DenSinH/FuzzARM](https://github.com/DenSinH/FuzzARM) |
| License | **GPL-3.0** — see [`LICENSE`](LICENSE) / [`NOTICE`](NOTICE) |
| Role | Seeded random ARM/Thumb soak after jsmolka green |

## License rules

- Upstream is **GPL-3.0**. Keep the notice; **do not relicense** into Graycart MIT.
- Prefer **download or git submodule** for ROMs over copying the Python/FASMARM
  generator into this MIT crate tree.
- Graycart’s root MIT does **not** cover these fixtures.

## Planned ROMs (when opted in)

Upstream prebuilts such as `ARM_DataProcessing.gba`, `THUMB_DataProcessing.gba`,
`ARM_Any.gba`, `THUMB_Any.gba`, `FuzzARM.gba` — regenerate with a fixed `--S`
seed for reproducibility.
