<!--
Cited: Graycart family README; graycart-gb README and docs/architecture.md; graycart-gba AGENTS.md
Note: host/library split. A claim that this boundary by itself “presents play without folding the machines together” was not supported. See provenance/CLAIMS.md.
-->

# One host, two machines

The family README assigns roles:

- `graycart-gba` is the long-term **play host** for GBA and for DMG/CGB.
- `graycart-gb` is the DMG/CGB **library** (crate name `graycart`) and a maintenance host. Its app left daily-target status at the P12 cutover.

New 8-bit and GBA play goes to this binary. The SM83 library stays a dependency. `AGENTS.md` in this repo forbids reimplementing that machine here.

## Boundary that already exists

`graycart-gb` `docs/architecture.md` splits lib and binary. The library is the machine and speaks framebuffer, PCM, and buttons. The binary owns the window, the UI, and the audio device. This crate should grow the same way: GBA core in the library, host in a frontend, compat as a call into `graycart` after the mode switch in [01-sm83-reuse.md](./01-sm83-reuse.md).

An independent check did **not** accept the stronger claim that this split, by itself, guarantees the two machines stay unmerged. The split only places pixels and samples on one side and the window on the other. Keeping the ARM7TDMI and the SM83 in separate crates is a separate rule, and it has to be held in the dependency graph.

## What “one interface” means

One binary, one file picker, one save location, one input map. Internally the file extension and the cartridge header choose a machine:

- `.gba` → ARM7TDMI core in this crate
- `.gb` / `.gbc` → `graycart` after the compatibility handoff

Do not fold both execute loops into one CPU type. Do not move SM83 opcodes into the ARM decoder. The umbrella repo stays a parent of submodules, not a Cargo workspace that compiles both machines as one crate.

`Cargo.toml` at 0.0.1 has an empty dependency table. Adding `graycart` is the step that makes the host real.
