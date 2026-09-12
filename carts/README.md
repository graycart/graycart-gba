# Local cartridges (`carts/`)

Place your own `.gba` dumps here for optional play / commercial smoke.

- **Never commit** commercial ROMs or Nintendo BIOS (see `.gitignore`).
- CI **skips** when this directory is empty — absence must not fail the build (G9-smoke).
- Sidecar battery saves (`.sav`) are also gitignored.

Obtain dumps from media you own. Firmware: place a user-supplied `gba_bios.bin`
outside the repo (or under a local `bios/` cache) for BiosLle; default play uses
BiosHle and does not require BIOS images.
