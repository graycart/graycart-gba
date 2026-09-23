<!--
Cited: the verification pass saved in research-report.md
Note: these items were explicitly left unresolved. Do not treat them as decided.
-->

# Open questions

The research report is partial. These gaps are from that pass. They are not permission to guess.

- The project-store `PHASES.md` and `08-implementation-plan.md` were not in this workspace. Must-versus-stretch wording came from the old changelog, the harnesses, and `AGENTS.md`.
- This working tree is the greenfield wipe. The phase table in [00-bring-up-order.md](./00-bring-up-order.md) is the prior contract, read from the New Projects checkout, not from code that still lives here.
- GBATEK is the primary, and it is not complete silicon truth. The old changelog records GBATEK’s window `X1 > X2` clamp as an oversimplification versus hardware wrap.
- Nothing lists which `graycart-gb` modules can be linked unchanged and which need a compatibility wrapper.
- Nintendo’s AGB manual calls the compatibility processor an 8-bit CISC CPU and does not use the name SM83. Cycle-for-cycle identity with the `graycart-gb` core is not established by GBATEK or Copetti.
- Pan Docs says the GBA APU mixes digitally and withholds extra wave RAM and DMA FIFOs from CGB programs. GBATEK still calls the four channels analogue CGB-compatible and only mentions sound BIAS as a CGB-mode register. Those two descriptions have not been reconciled here.
- GBATEK says it is unknown whether the ARM7TDMI can keep running after the console enters CGB mode.
- GBATEK’s note that retail GBA dropped a built-in infrared port is about a GBA-mode prototype, not CGB `FF56` / RP behavior.
- No type in this crate yet shows how one binary chooses between a future GBA core and the `graycart` library.
- `graycart-linux` cites `product-decision-supersede-gb.md` as the canonical supersede note. That file is not in this workspace and was not read.
- `graycart-gb`’s `Cargo.toml` still lists winit, egui, and cpal on the package. Nothing says a downstream dependency on the library avoids those host crates.
- The official SUPER ZSNES page that was read only publishes v0.300. Older notes (MSU-1, an earlier Super FX) were not on that page, so they are not sources here.
- v0.300 says “SuperFX3” and credits Randy Linden. `graycart-snes` only inventories retail SuperFX / GSU-1 / GSU-2. Neither source says whether SuperFX3 is that silicon or something else.
- No `graycart-snes` document mentions SUPER ZSNES. Alignment in [03-super-zsnes.md](./03-super-zsnes.md) is with the written SNES plan, not a recorded decision to adopt that emulator.
- jsmolka `ppu/hello`, `shades`, and `stripes` do not say which video mode they draw in their READMEs; the sources confirmed shades and stripes are mode 0. Page 5 treats hello as the bitmap hash gate. Pages 6 and 7 own tiles, sprites, windows, blend, and mosaic. If a fixture README shows one of those three is a tile or sprite test, that ROM’s gate moves to page 6.
