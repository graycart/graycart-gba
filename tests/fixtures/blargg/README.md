# Blargg fixtures

Game Boy hardware test ROMs by **Shay Green** (“Blargg”).

## Provenance

- **Author:** Shay Green \<gblargg@gmail.com\>
- **Common mirror used here:** [retrio/gb-test-roms](https://github.com/retrio/gb-test-roms)
- **Historical hosts (per upstream readme):** originally `http://blargg.parodius.com/gb-tests/`; later noted as `http://blargg.8bitalley.com/parodius/gb-tests/`

Vendored suite docs retained under this tree (for example `cgb_sound/readme.txt`). See [`LICENSE`](LICENSE) for licensing status relative to Graycart MIT.

## Targets

- `cpu_instrs/` — CPU arithmetic / flags (green)
- `dmg_sound/` — APU conformance (Phase 8H)
- `cgb_sound/` — CGB APU (retrio `cgb_sound`; Fast Native CGB)
- Also present: `instr_timing/`, `mem_timing/`, `mem_timing-2/`, `interrupt_time/`, `oam_bug/`, `halt_bug.gb`

```bash
cargo test --test roms blargg_dmg_sound_matrix -- --ignored --nocapture
cargo test --test roms blargg_cgb_sound_matrix -- --ignored --nocapture
```
