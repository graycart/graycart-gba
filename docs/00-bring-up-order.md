<!--
Cited: GBATEK — GBA Technical Data, LCD Video Controller, Timers, Interrupt Control, DMA, Sound, Cartridges, BIOS
  https://problemkaputt.de/gbatek.htm
Cited: ARM Architecture Reference Manual DDI 0100 (ARMv4 / ARM7TDMI material is the CPU primary alongside GBATEK’s ARM CPU Reference)
Note: phase order corrected against the old graycart-gba changelog. The “P1 sits between P0 and P2, and P9–P12 add no native silicon” claim was rejected. See provenance/CLAIMS.md.
-->

# Bring-up order

Same posture as `graycart-gb`: research names the reference and the acceptance test, then the owning subsystem gets its own module and tests. The library is the machine. A window, shader, or audio device does not belong in that library.

GBATEK (Martin Korth) is the primary GBA document. Secondary emulators and Tonc are cross-checks. They do not override GBATEK or a failing test ROM.

## Order

The previous tree’s changelog does **not** support a clean “P1 ARM core, then P2 bus, then P9–P12 are only a host.” An independent check of that changelog rejected that summary. What did match:

| Slice | Own this | Primary reference | Acceptance gate that was actually recorded |
|-------|----------|-------------------|-----------------------------------------------|
| Scaffold | Crate, module boundaries, no BIOS or commercial ROMs in git | This pack and `AGENTS.md` | `cargo test` green with no firmware |
| ARM7TDMI | ARM and Thumb, exceptions, the three-stage pipeline | GBATEK ARM CPU Reference; ARM DDI 0100 / the ARM7TDMI datasheet cited by the old CPU module | jsmolka `arm.gba` and `thumb.gba` PASS under BiosHle, `r12 == 0` after idle. Pin the suite at `a7113b67e63f83a9b321696ddd7042ccfad6c881` |
| Bus and memory | Region map, mirrors, Game Pak waitstates, open bus | GBATEK Memory Map and Gamepak Waitstates | jsmolka `memory.gba` PASS, same BIOS posture |
| Timers, IRQ, input | Timer 0–3, IE/IF/IME, keypad | GBATEK Timers; GBATEK Interrupt Control; GBATEK Keypad Input | In-house `simple-irq` ROM (ignored until the ROM exists). mGBA `io-read` and `timer-irq` are stretch stubs, not the must |
| PPU | Modes 0–5, scanline, sprites, windows, blend, mosaic | GBATEK LCD Video Controller | jsmolka `ppu/hello`, `shades`, `stripes`: SHA-256 of one settled 240×160 RGB888 frame |
| DMA | Immediate, VBlank, HBlank, FIFO special, DMA3 Game Pak; no SRAM DMA | GBATEK DMA Transfers | Unit tests for those paths. mGBA `suite.gba` dma gate stays off until that ROM is vendored |
| APU | Four PSG channels plus two DMA FIFOs, resample in the host | GBATEK Sound Controller | Unit tests. `cajunpanda/gba-audio-test` was absent, so it was not the live gate |
| Cart, BIOS, saves | SRAM, Flash 64/128, EEPROM, BIOS SWI surface | GBATEK Cartridges; GBATEK BIOS Functions | jsmolka `bios`, `save/none`, `sram`, `flash64`, `flash128` under BiosHle. A real `gba_bios.bin` is user-supplied and never committed |
| Timing | Prefetch, IRQ delay, N/S/I waitstates | GBATEK Gamepak Prefetch and waitstates | mGBA timing and dma thresholds stay at 0 until `suite.gba` is vendored |
| Mode switch | `WAITCNT` bit 15, `DISPCNT` bit 3, `HALTCNT` | GBATEK Backwards Compatibility (see [01-sm83-reuse.md](./01-sm83-reuse.md)) | A named GB/CGB cart handoff test, not a title hack |
| Host | Window, shader, audio device, one library picker | Family README; `graycart-gb` `docs/architecture.md` | Headless `--frames` still runs with no window |

Do not invent a phase past what a reference and a test can hold. Do not copy the deleted 0.1.x sources back in to “save time.”

## Test ROM authors

| Suite | Who | Role |
|-------|-----|------|
| [jsmolka/gba-tests](https://github.com/jsmolka/gba-tests) | jsmolka | Must gates for CPU, memory, a first picture, BIOS, and saves |
| [mgba-emu/suite](https://github.com/mgba-emu/suite) | endrift (mGBA) | Later accuracy stretch. Not vendored in the old tree |
| gba-audio-test | cajunpanda | APU ROM stretch. Absent when P6 was gated on units |
| Tonc | Jasper Vijn (cearn), [coranac.com/tonc](https://www.coranac.com/tonc/text/toc.htm) | Secondary tutorial and examples. Not the oracle |
| NanoBoyAdvance | fleroviux | Secondary cross-check only |

Commercial ROMs stay under `carts/` and never enter git.
