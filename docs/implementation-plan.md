<!--
Cited: previous graycart-gba console log shape (gba-debug: key=value, AV report).
  Reimplement the lines. Do not copy the deleted src/debug module.
Cited: GBATEK, Pan Docs, Copetti, jsmolka gba-tests — see docs/provenance/SOURCES.md
Note: agreed direction from the greenfield session. Page 0 is not implemented.
-->

# GBA emulator, page by page

Greenfield `graycart-gba` (crate `0.0.1`, empty lib). Each page is one stop: the crate still builds, `cargo test` is green without a BIOS image or a commercial ROM, and you can run one headless command and read a debug report before the next page starts.

Sources are the pack in `graycart-gba/docs/`. GBATEK (Martin Korth) is the hardware primary. jsmolka’s suite, pinned at `a7113b67e63f83a9b321696ddd7042ccfad6c881`, is the early acceptance ROM. mGBA, NanoBoyAdvance, and Tonc (Jasper Vijn) are cross-checks. Do not copy the deleted 0.1.x tree. Do not port SUPER ZSNES into this core (`docs/03-super-zsnes.md`).

## How this matches the Game Boy crate

Same split as `graycart-gb`:

- The **library** is the machine. It speaks a framebuffer, PCM, and buttons. It does not open a window or an audio device.
- The **binary** owns the CLI from page 0, and winit/wgpu/egui/cpal only on the last page.
- One subsystem, one module: `cpu`, `bus`, `dma`, `timer`, `irq`, `ppu`, `apu`, `input`, `cart`, `bios`, `hw`, `compat`, `debug`. Tests live in `src/<module>/tests.rs`. ROM harnesses live in `tests/roms/` and fixtures in `tests/fixtures/`.
- Orchestration stays thin. Bus routes. CPU executes. PPU, APU, DMA, and timers do not get implemented inside the decoder.
- File headers credit GBATEK, ARM DDI 0210C, Pan Docs, or Copetti when a file depends on them.
- A finished page bumps the patch version (`0.0.2` through `0.0.14`). Page 13 is `0.1.0`, the first runnable host. No `1.0.0`.

## Stop ritual

One page, one version, then stop. Do not start the next page in the same change.

1. `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test`. Default CI stays green with no BIOS image and no commercial ROM. Page 0 adds that workflow; this tree does not have one yet.
2. Run the manual command printed for that page. A missing fixture skips. A skip is not a pass. A page that names a ROM is done only after that ROM is run locally and the debug line says pass.
3. Bump `[package].version` in `Cargo.toml`. That file is the only product version (`AGENTS.md`).
4. Add a `CHANGELOG.md` entry for that version. Page 0 creates the file. The `0.0.1` entry is the empty tree.
5. Stop for a manual read of the debug report. A git tag is created only when asked. If a tag exists, it is `v` plus the crate version.

## What you watch

Conformance ROMs are the GBA suites in the table below: one ROM, a visible pass or a failing test number. A `.gb` or `.gbc` dropped in `carts/` is a smoke run of the `graycart` machine after the handoff page.

From page 0, `--debug` writes the `gba-debug:` lines to stderr and to `gba-debug.log` in the working directory. The file is replaced at the start of the run. That is the file to keep open. The same lines, not a second format.

A summary line is written every 60 frames (`cpu`, `ppu`, `dma summary`, `irq summary`, `apu health`, `ppu health`), plus a line when something changes (ROM header, BIOS launch, `WAITCNT`, forced blank, layers, IE). `--debug=trace` adds instruction lines to that same log, still capped to the frame run. The AV report is on stdout and copied into the log.

A path that is a directory runs every `.gba`, `.gb`, and `.gbc` in it, one after another, with a banner per file in the same log. An empty directory is an error. `carts/` is that directory. It is empty in this tree. Drop dumps there. They stay gitignored. A cart log is `gba-debug: smoke file=<name> frames=<n> fault=<none|reason>`. It is never `PASS` or `FAIL`. `halt forever` and `StepError` are the lines that say it did not keep running.

## Suites

Accuracy ROMs live under `tests/fixtures/` and may be committed. Commercial dumps do not.

| Suite | License | In this tree | How it is used |
|-------|---------|--------------|----------------|
| jsmolka gba-tests `a7113b67` | MIT | `tests/fixtures/jsmolka/` (no `unsafe.gba`) | Must gates, one page at a time. `r12 == 0` is pass; `thumb.gba` stores the id in `r7` |
| alyosha-tas/gba-tests `66f5f1d6` | MIT | `tests/fixtures/alyosha/` | Later edge tests. Not a page gate |
| png183/gba-tests `87937453` | MIT | `tests/fixtures/png183/` | Later edge tests. Not a page gate |
| mGBA suite `e6942030` | MIT | `tests/fixtures/mgba-suite/` (source, no `suite.gba`) | Timing and DMA stretch. No ROM yet, so the harness skips. A skip is not a pass |
| nba-emu/hw-test `fbc99140` | BSD-3-Clause | `tests/fixtures/nba/` | Later. Not a page gate |
| hades-emu/Hades-Tests `29274ce3` | GPL-2.0 | `tests/fixtures/hades/` | Later. License file stays with the suite |
| DenSinH/FuzzARM `a675329c` | GPL-3.0 | `tests/fixtures/fuzzarm/` | Later. License file stays with the suite |
| AGS and AGBEEG aging carts | Not ours to ship | Not in the tree | Factory cartridges |

## Debug, from page 0

Page 0 shipped as crate `0.0.2`. Page 1 shipped as crate `0.0.3`. Page 2 shipped as crate `0.0.4`. Page 3 shipped as crate `0.0.5`. Page 4 shipped as crate `0.0.6`. Page 5 shipped as crate `0.0.7`. Page 6 shipped as crate `0.0.8`. Page 7 shipped as crate `0.0.9`. Page 8 shipped as crate `0.0.10`. Page 9 shipped as crate `0.0.11`. Page 10 shipped as crate `0.0.12`. Page 11 shipped as crate `0.0.13`. Page 12 shipped as crate `0.0.14`. Page 13 shipped as crate `0.1.0`.

Audio and video debugging is text on stderr, in the same line format as the previous `graycart-gba` console log. A wav or a picture is optional and secondary. The thing a person or a model reads is one greppable line per fact, `key=value`, prefix `gba-debug:`. Do not paste the deleted `debug` module back in. Reimplement this contract.

The command line stays small:

```text
graycart-gba <rom> --frames <n> [--debug]
graycart-gba <rom> --debug [--frames <n>]
graycart-gba <directory> --frames <n> [--debug]
graycart-gba <directory> --debug [--frames <n>]
```

`--debug` stops when the result is pass or fail, and exits non-zero on fail. `--frames` caps that run. Without `--debug`, `--frames` is required. `--debug=trace` is the instruction firehose, still under the `gba-debug:` prefix. There is no `--trace`, `--disasm`, `--ppm`, or `--wav`. A picture or a sound file is not how we check a page. When a page is ready, the test command is written out in full so it can be copied as-is.

Every breadcrumb is one stderr line:

```text
gba-debug: <kind> <fields>
```

Kinds that stay stable for the whole rewrite:

- `cpu` — `frame` `pc` `region` `cpsr` `mode` `thumb` `i_mask` `ime` `ie` `if` `power`
- `ppu` — `frame` `mode` `vcount` `dispcnt` `forced_blank` `layers` `mosaic` `blend` `objwin`
- `apu health` — the audio line, below
- `apu host` — `frame` `pwm` `host` `ratio` `underrun_events` once a device exists
- `warn` — `halt forever`, `swi unhandled`, `openbus`, `ppu all-black`, `ppu backdrop-only`, `apu fifo`
- `irq` — `n` `handler` `ie` `if` `ime`
- `swi summary` — counts, not one line per call

End of `--debug --frames N` prints a short block on stdout, same headers as before:

```text
=== graycart-gba AV report (frame=N) ===
PPU mode=… dispcnt=0x…. blank=… layers=… mosaic=0x…. blend=… objwin=…
PPU pixels black=…% backdrop=…% other=… bd=0x…. writes vram=… oam=… pal=…
APU master=… pwm=…Hz fifoA=… fifoB=… underrun=…/… overrun=…/… empty_drain=…/… peak=[… …] dc≈[…,…] clip=…
SWI unhandled=(none)
```

The audio line a model should grep is exactly:

```text
gba-debug: apu health frame=<n> master=<0|1> pwm=<hz>Hz fifoA=<route> underrun=<a>/<b> overrun=<a>/<b> empty=<a>/<b> lag=<a>/<b> dma_req=<a>/<b> peak=[<min>..<max>] dc≈[<l>,<r>] clip=<n> extreme=<n> psg_on=0x<n> psg_nr50=0x<vv> fifoB=<route>
```

Until the APU exists, that line still prints, with zeros and `master=0`, so the format never changes. A warn is a separate line, not a paragraph:

```text
gba-debug: warn apu fifo empty-drain/underrun period empty=<n> underrun=<n> lag=<a>/<b> (held last sample — pops/silence/stuck tone)
```

Instruction lines, when `--debug=trace` is on, use the same prefix plus `pc`, the mnemonic, `r12`, and CPSR, for both Thumb and ARM.

jsmolka pass/fail is also one line: `gba-debug: cpu result=PASS r12=0 r7=<n>` or `result=FAIL r12=<n> r7=<n>` with `pc` and the mnemonic. `r12` is the test number the ROM writes (`thumb.gba` stores the id in `r7`). No invented test names. Upstream also draws that number in mode 4. From the picture page, add `screen=<n>` on the same line when the framebuffer shows it.

## What stays out of every page

- A second SM83. Game Boy mode calls the `graycart` crate (`docs/01-sm83-reuse.md`). Cycle-identity with that core is an open question (`docs/04-open-questions.md`); do not claim it.
- Whether the ARM7TDMI keeps running after CGB mode. GBATEK says that is unknown. The switch halts the ARM side until a source says otherwise.
- Title hacks, commercial ROMs, and `gba_bios.bin` in git.
- SUPER ZSNES chips, widescreen, and replacement audio. Those are SNES or host filters.
- mGBA `suite.gba` thresholds treated as passes while the ROM is absent. Record them as 0.

## Page 0 — Scaffold and the debug contract

**Docs:** `docs/00-bring-up-order.md` scaffold row, `AGENTS.md`.

**Build:** declare the modules above as empty owners. Public surface is `Bus`, `Cpu`, `StepError`, `MachineDebug`, `format_trace_line`. CLI parses the flags. `--frames` with no ROM exits with a clear error. `--debug` on a stub machine prints each section as `absent`, except `apu health`, which always prints with zeros and `master=0`. Add `.github/workflows/ci.yml` for the three commands in the stop ritual. Create `CHANGELOG.md`.

**Manual:** one command, printed in full at test time: no ROM must exit with a clear error; a tiny file with `--debug --frames 1` must print the `gba-debug:` lines and the AV report with empty sections, and must not spam unimplemented opcodes.

**Done:** `cargo fmt`, `clippy -D warnings`, `cargo test`. Version `0.0.2`.

## Page 1 — ARM

**Docs:** bring-up ARM row. GBATEK ARM CPU Reference. ARM DDI 0210C (ARM7TDMI). DDI 0100 is the architecture manual, not a second spec for this page.

**Build:** registers, CPSR, ARM decode, a 3-stage pipeline model that is correct about PC+8 and about flushing on taken branches. Exceptions enough for the test (reset, SWI, IRQ stub). Thumb encodings return `StepError` and increment the fault counter. Unimplemented ARM encodings do the same. Do not guess.

BiosHle is only what `arm.gba` needs to reach idle with `r12 == 0`: the SWIs that ROM calls, and the idle halt. It is not the full BIOS catalogue, and it is not a copy of the deleted HLE. A real BIOS file is never read on this page.

The ROM is already at `tests/fixtures/jsmolka/arm/arm.gba`. Tests skip if it is absent.

**Manual:** `cargo run --release -- --debug --frames 30` on `arm.gba`. Pass is `result=PASS r12=0`. A fail run is the same command with `--debug=trace`. The first unimplemented mnemonic is the line to read.

**Done:** `arm.gba` passes locally. Default `cargo test` asserts it when the fixture is present and skips when it is not. Version `0.0.3`.

## Page 2 — Thumb

**Docs:** same ARM row. GBATEK Thumb. ARM DDI 0210C.

**Build:** Thumb decode and execute on the pipeline from page 1. PC+4. Flush on taken branches. BiosHle grows only by the SWIs `thumb.gba` calls.

The ROM is already at `tests/fixtures/jsmolka/thumb/thumb.gba`.

**Manual:** `cargo run --release -- --debug --frames 30` on `thumb.gba`. Pass is idle and `r7 == 0`. `r7` in `1..=999` is a `thumb.gba` failure id; `arm.gba`'s leftover `r7` is outside that range, so the `PASS` word follows `r12` unless `r7` is a small id. A fail run adds `--debug=trace`.

**Done:** `thumb.gba` passes locally, and `arm.gba` still passes. Version `0.0.4`.

## Page 3 — Bus and memory

**Docs:** bring-up bus row. GBATEK memory map, mirrors, Game Pak waitstates, open bus.

**Build:** `bus` routes BIOS, IWRAM, EWRAM, I/O, palette, VRAM, OAM, ROM, SRAM. Bad widths and unused bits return the open-bus value and log the address once in `--debug`. A BIOS read that is not an opcode fetch returns the open-bus value GBATEK specifies (the last prefetched BIOS opcode). Waitstate counters exist even if every region is still 1 cycle; the summary shows them so page 11 has somewhere to put real N/S/I costs.

SIO (`SIOCNT`, `RCNT`, and the data registers) reads as no cable and an idle transfer. One `gba-debug: warn sio unlinked` line per run. Multiplayer, wireless, and Joy Bus are not this page and not a later page in this plan. GPIO cart devices (RTC, rumble, tilt, solar) are page 10.

**Manual:** one printed command, `--debug --frames` on `memory.gba`. Pass is `r12=0` and no unexpected `openbus` line. A unit test pokes a bad address and the summary names the region.

**Done:** `memory.gba` passes locally. Version `0.0.5`.

## Page 4 — Timers, IRQ, keypad

**Docs:** bring-up timers/IRQ row. GBATEK Timers, Interrupt Control, Keypad Input.

**Build:** timers 0–3, IE/IF/IME, the keypad register. IRQ entry uses the CPU exception path from page 1. An enabled, pending IRQ wakes a halted CPU. `gba-debug: warn halt forever` is only for a halt that IME and IE cannot wake. `--debug` prints IE, IF, IME, and each timer’s counter and cascade bit.

This plan does not build `simple-irq.gba`. The must is the unit tests. If that ROM is added later, the harness runs it; until then it skips and says so. A skip is not a pass. mGBA `io-read` and `timer-irq` stay stretch stubs, not the must.

**Manual:** `cargo test` for overflow, cascade, one IRQ, and halt wake. If `simple-irq.gba` exists, one printed `--debug --frames` command. The `irq` line shows IF when a button is down. No extra key flag.

**Done:** unit tests cover overflow, cascade, one IRQ taken, and halt wake. `simple-irq` passes when the ROM is added. Version `0.0.6`.

## Page 5 — Picture

**Docs:** bring-up PPU row. GBATEK LCD Video Controller.

**Build:** scanline timing, backdrop, bitmap modes 3–5. Frame is 240×160. Core stores the hardware pixel. The AV report prints the SHA-256 of one settled frame and the non-zero pixel count. No picture-file flag. Tile modes, sprites, windows, blend, and mosaic are later pages.

jsmolka `ppu/hello` is the page 5 hash gate (mode 4 via `text_init`: mode 4 | BG2); `shades` and `stripes` move to page 6. Hash the settled frame. `screen=` on the jsmolka fail line is omitted until the framebuffer digits can be read.

**Manual:** one printed command, `--debug --frames 5` on hello. The AV report hash matches the fixture.

**Done:** hello hash matches. Version `0.0.7`.

## Page 6 — Tiles and sprites

**Docs:** GBATEK LCD Video Controller, backgrounds and OBJ.

**Build:** modes 0–2, including affine backgrounds, and sprites. The picture hashes from page 5 still match.

**Manual:** `cargo test` for one tile background and one sprite. The debug line names the mode and the sprite count. No new ROM is required.

**Done:** those unit tests pass, hello still hashes, and shades and stripes hashes are locked. Version `0.0.8`.

## Page 7 — Windows, blend, mosaic

**Docs:** GBATEK LCD Video Controller, windows, blend, mosaic.

**Build:** windows, blend, and mosaic. Implement GBATEK’s window rule. Leave the `X1 > X2` wrap disagreement (`docs/04-open-questions.md`) as a named failing or ignored test. Do not silently copy the old tree’s wrap behavior.

**Manual:** `cargo test` for one window, one blend, mosaic, and the named `X1 > X2` case.

**Done:** those unit tests pass, and the page 5 hashes still match. Version `0.0.9`.

## Page 8 — DMA

**Docs:** bring-up DMA row. GBATEK DMA Transfers.

**Build:** immediate, VBlank, HBlank, special FIFO, DMA3 Game Pak. SRAM as a DMA source or dest fails clearly. CPU is stalled for the transfer and the summary shows channel, word count, and reason.

mGBA dma ROM stays off while `suite.gba` is absent. The gate is unit tests.

**Manual:** `cargo test` for the five paths. The debug line shows words moved, and `sram-dma: rejected` when a test asks for SRAM DMA. A ROM command is printed only if a fixture for that path exists.

**Done:** unit tests cover the five paths. Version `0.0.10`.

## Page 9 — APU

**Docs:** bring-up APU row. GBATEK Sound Controller.

**Build:** four PSG channels plus two DMA FIFOs, in this crate. Do not link the `graycart-gb` APU. The GBA channels are mixed with the FIFOs and `SOUNDBIAS`; that is not the Game Boy mixer. Mixing to PCM stays in the core at the hardware rate. Resample and the device stay in the host, same as `graycart-gb` `frontend/audio`. The debug record is the `gba-debug: apu health` line and the `APU` row of the AV report.

`cajunpanda/gba-audio-test` is the stretch ROM. If it is not vendored, the live gate is the unit tests, and a `gba-debug:` line says that ROM gate was skipped.

**Manual:** one printed command, `--debug --frames 120` on a ROM that plays a tone. Read `apu health` and any `warn apu` line. Underrun, empty-drain, clip, and `dc≈` are the fields that say what is wrong.

**Done:** unit tests for PSG enable, FIFO refill, and “SRAM is not a FIFO source”. Version `0.0.11`.

## Page 10 — Cartridge, saves, BIOS surface

**Docs:** bring-up cart/BIOS row. GBATEK Cartridges and BIOS Functions.

**Build:** ROM waitstate mirrors, SRAM, flash 64K/128K, EEPROM. EEPROM size (512 bytes or 8 KiB) comes from the bus protocol, not from the ROM title. Header parse with a clear error on a truncated file. Save sidecar next to the ROM, gitignored. GPIO cart devices (RTC, rumble, tilt, solar) are unsupported: one `gba-debug: warn cart gpio unsupported` line, and the access fails clearly.

Grow BiosHle from the page-1 subset to the SWIs the jsmolka BIOS and save ROMs call. Still no committed `gba_bios.bin`. An optional local BIOS path is LLE later, off by default.

**Manual:** five printed commands, `--debug --frames 30` on bios, none, sram, flash64, and flash128. The report names the save kind. `r12=0` on each.

**Done:** those five pass locally, plus unit tests for EEPROM 512 and 8 KiB and for one rejected GPIO access. Version `0.0.12`.

## Page 11 — Prefetch and real waitstates

**Docs:** bring-up timing row. GBATEK Game Pak prefetch and waitstates.

**Build:** N/S/I costs and the prefetch buffer. IRQ delay if GBATEK specifies it for this slice. `--debug` prints stall cycles and prefetch hits, so a slow ROM is measurable before a frontend exists.

mGBA timing and dma numbers stay 0 until `suite.gba` is built. Do not mark them passed.

**Manual:** the same printed `memory.gba` and `arm.gba` commands as pages 3 and 1. They still pass. The waitstate field is non-zero on ROM fetches. If one regresses, rerun that command with `--debug=trace` and fix the earliest wrong memory cycle.

**Done:** previous ROM gates still pass, unit tests cover one N and one S access. Version `0.0.13`.

## Page 12 — Game Boy carts through `graycart`

**Docs:** `docs/01-sm83-reuse.md`, `docs/02-one-host-two-machines.md`, `docs/04-open-questions.md`.

**Build:** depend on the `graycart` crate for the SM83 machine. This crate owns the switch only: boot in GBA mode, `WAITCNT` bit 15 as the cart-shape sense, `DISPCNT` bit 3 to prepare, `HALTCNT` to apply, `4000800h` bit 3 for CGB boot-ROM disable. After the switch the ARM core does not execute. A `.gb` / `.gbc` file is handed to `graycart` with the AGB boot registers from Pan Docs (A=`$11`, B=`$01`).

Do not reimplement SM83, the Game Boy PPU, or the Game Boy APU. Do not resolve the Pan Docs vs GBATEK CGB-audio disagreement on this page; the Game Boy machine keeps the `graycart` APU, and the debug line says which description is unimplemented. The GBA CGB brightness ramp (`docs/01-sm83-reuse.md`) is also unimplemented here. The debug line says so. Palette correction is not this page.

Check the open Cargo question before the dependency lands: `graycart`’s package manifest still lists winit, egui, and cpal. The GBA **library** must not use them. If a normal dependency pulls those into the lib, depend in a way that keeps them on the binary only, or stop and fix the GB crate’s manifest first.

**Manual:** three printed commands, `--debug --frames` on one local `.gb`, one local `.gbc`, and one `.gba`. The report says `machine=sm83` or `machine=arm7` and the handoff registers. No window.

**Done:** a small ROM of each kind reaches a frame without an ARM opcode running in SM83 mode. Version `0.0.14`.

## Page 13 — One play host

**Docs:** `docs/02-one-host-two-machines.md`, `graycart-gb` `docs/architecture.md`.

**Build:** frontend binary only. eframe / winit / wgpu / egui / cpal, same roles as `graycart-gb` `frontend/`. One file picker. Extension and header choose ARM vs `graycart`. Shade-to-RGB and device resample live here. The debug summary stays available as a panel fed by `MachineDebug`, not a second logger.

`--frames` remains headless and must not link the window.

**Manual:** open one `.gba` and one `.gbc` from the window. Then two printed commands, `--debug --frames 60` on those same files, with no window. Audio device follows the GB rule: chosen device, else OS default, else a named fallback, else silent. The `apu health` line still prints.

**Done:** both paths show a frame in the window and the headless command still works. Version `0.1.0`.

## Page 14 — Same shell as graycart-gb

Shipped as crate `0.2.0`. The play window is the Game Boy shell (File, Emulation, Video, Input, Audio, Help, Debug). See `docs/superpowers/plans/2026-09-23-complete-emulator.md`.

## Page 15 — IntrWait

Shipped as crate `0.2.1`. BIOS IntrWait / VBlankIntrWait watch `0x03007FF8`.

## Page 16 — CpuSet

Shipped as crate `0.2.2`. BIOS CpuSet / CpuFastSet copy and fill.

## Page 17 — Decompress

Shipped as crate `0.2.3`. BIOS LZ77, Huffman, RLE, and diff filters (WRAM and VRAM widths).

## Page 18 — Math and affine SWIs

Shipped as crate `0.2.4`. BIOS sqrt, arctan, affine set, BitUnPack, SoundBias, SoftReset, and RegisterRamReset. Music and multiboot SWIs warn and return.

## Page 19 — Affine objects

Shipped as crate `0.2.5`. Affine objects draw; `hello.gba` hash unchanged.

## Page 20 — DMA3 video capture

Shipped as crate `0.2.6`. DMA3 timing 3 copies on the video-capture scanline; channels 0–2 with timing 3 still do not copy; SRAM DMA still rejected. Continue with page 21 (`0.2.7`).

## After each page

Same three commands, from `graycart-gba/`:

```text
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Then the manual command for that page. If the summary’s first fault is in an earlier subsystem, fix that subsystem. Do not patch the test ROM’s title.
