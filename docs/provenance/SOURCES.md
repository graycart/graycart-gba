<!--
Note: bibliography for docs in the parent folder. Titles and URLs are the ones the research pass and the follow-up page reads actually used.
-->

# Sources

Credit the person or project at the top of any file that depends on these. A link in this list is not a substitute for that header.

## Primary hardware documents

| Who | Work | Link |
|-----|------|------|
| Martin Korth (nocash) | GBATEK — GBA/NDS Technical Info. Technical data, memory map, LCD, sound, timers, DMA, keypad, interrupts, system control, cartridges, BIOS, ARM and Thumb opcode reference | https://problemkaputt.de/gbatek.htm |
| Martin Korth (nocash) | GBATEK — GBA Backwards Compatibility, CGB mode | https://problemkaputt.de/gbatek-gba-backwards-compatibility-cgb-mode.htm |
| gbdev, continuing Pan Docs (originally Pan of Anthrox) | Pan Docs — Power-Up Sequence. AGB versus CGB boot registers | https://gbdev.io/pandocs/Power_Up_Sequence.html |
| ARM Ltd | ARM Architecture Reference Manual and the ARM7TDMI technical reference (DDI 0210). CPU primary next to GBATEK’s ARM chapter. The old `cpu` module cited DDI 0210C | https://developer.arm.com/documentation/ddi0210/c |

## Secondary writing and tutorials

| Who | Work | Link | How we use it |
|-----|------|------|----------------|
| Rodrigo Copetti | Game Boy Advance Architecture — A Practical Analysis. Confirms the Sharp SM83, the two clocks, and that the CPUs do not run together. Also Dave Jaggar and the Thumb set | https://www.copetti.org/writings/consoles/game-boy-advance/ | Secondary overview. GBATEK wins on register bits |
| Jasper Vijn (cearn) | Tonc — GBA programming tutorial | https://www.coranac.com/tonc/text/toc.htm | Secondary. Named by `AGENTS.md` |
| endrift | mGBA and the mGBA test suite | https://mgba.io/ and https://github.com/mgba-emu/suite | Cross-check and later ROM gates. Not an oracle over GBATEK |
| fleroviux | NanoBoyAdvance | https://github.com/nba-emu/NanoBoyAdvance | Cross-check only |

Copetti’s article also names, as background rather than as GBA register sources: Dave Jaggar (ARM7TDMI and Thumb), Robin Saxby (ARM Ltd), and the Patterson RISC paper. Those are history of the CPU, not acceptance tests.

## Test ROMs

| Who | Work | Link |
|-----|------|------|
| jsmolka | gba-tests. `arm.gba`, `thumb.gba`, `memory.gba`, PPU hello/shades/stripes, BIOS and save ROMs. Old tree pinned commit `a7113b67e63f83a9b321696ddd7042ccfad6c881` | https://github.com/jsmolka/gba-tests |
| cajunpanda | gba-audio-test. Recorded as absent when APU was unit-gated | https://github.com/cajunpanda/gba-audio-test |

## SUPER ZSNES

| Who | Work | Link |
|-----|------|------|
| SUPER ZSNES team | v0.300 notes: SA-1, Cx4, SuperFX3, per-game overclock, SNES Mouse and Super Scope. The page says the original ZSNES developers rewrote it and does not name those two handles in the welcome text | https://www.zsnes.com/ |
| Randy Linden | Thanked on that page for SuperFX3 emulation | https://www.zsnes.com/ |
| tssf | Thanked on that page for Chrono Trigger audio enhancement | https://www.zsnes.com/ |
| fatbrowne | Thanked on that page for save-state help on the Zelda enhancement | https://www.zsnes.com/ |

## This repository family

Read these in the checkout. They are project decisions, not hardware oracles.

| Path | What it settled |
|------|-----------------|
| `README.md` (umbrella) | `graycart-gba` is the play host; `graycart-gb` remains the SM83 library |
| `graycart-gb/README.md` | Same cutover. Package name `graycart` |
| `graycart-gb/docs/architecture.md` | Library versus frontend binary |
| `graycart-gba/AGENTS.md` | GBATEK, then a test, then secondary notes. No second SM83 |
| `graycart-gba/README.md` and `Cargo.toml` | 0.0.1 has no `graycart` dependency yet |
| `graycart-snes/docs/AGENTS.md` | No title hacks. Filters stay in the host |
| `graycart-snes/docs/00-architecture-overview.md` | MSU-1 is not a v1 must |
| `graycart-snes/docs/06-cart-mapping-chips.md` | SA-1, SuperFX, Cx4 are deferred and must fail closed |
| `graycart-snes/docs/05-timing-irq-joypad-wram.md` | Standard pad is required; mouse is stretch; Super Scope is later |

The independent check of the old GBA changelog and jsmolka fixture README used a second checkout that still has the 0.1.x tree:

`C:\Users\axolatl-tank\New Projects\graycart\graycart-gba\`

That tree is not this working copy. This copy had those files removed on purpose.
