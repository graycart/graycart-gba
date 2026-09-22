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
- A finished page bumps the patch version (`0.0.2`, `0.0.3`, …). No `1.0.0`.

Missing fixtures **skip**. A skip is not a pass. A page that names a ROM is done only after you run that ROM locally and the debug line says pass.

## Debug, from page 0

Page 0 has not been implemented. Start there.

Audio and video debugging is text on stderr, in the same line format as the previous `graycart-gba` console log. A wav or a picture is optional and secondary. The thing a person or a model reads is one greppable line per fact, `key=value`, prefix `gba-debug:`. Do not paste the deleted `debug` module back in. Reimplement this contract.

The command line stays small:

```text
graycart-gba <rom> --frames <n> [--debug]
graycart-gba <rom> --frames <n> --debug=trace
```

`--frames` is headless. `--debug` is the summary. `--debug=trace` is the instruction firehose, capped to the frame run, still under the `gba-debug:` prefix. There is no `--trace`, `--disasm`, `--ppm`, or `--wav`. A picture or a sound file is not how we check a page. When a page is ready, the test command is written out in full so it can be copied as-is.

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

jsmolka pass/fail is also one line: `gba-debug: cpu result=PASS r12=0` or `result=FAIL` with `pc` and the mnemonic. No invented test names. If the pinned suite README documents a result buffer, print those names as more `key=value` fields on that line.

## What stays out of every page

- A second SM83. Game Boy mode calls the `graycart` crate (`docs/01-sm83-reuse.md`). Cycle-identity with that core is an open question (`docs/04-open-questions.md`); do not claim it.
- Whether the ARM7TDMI keeps running after CGB mode. GBATEK says that is unknown. The switch halts the ARM side until a source says otherwise.
- Title hacks, commercial ROMs, and `gba_bios.bin` in git.
- SUPER ZSNES chips, widescreen, and replacement audio. Those are SNES or host filters.
- mGBA `suite.gba` thresholds treated as passes while the ROM is absent. Record them as 0.

## Page 0 — Scaffold and the debug contract

**Docs:** `docs/00-bring-up-order.md` scaffold row, `AGENTS.md`.

**Build:** declare the modules above as empty owners. Public surface is `Bus`, `Cpu`, `StepError`, `MachineDebug`, `format_trace_line`. CLI parses the flags. `--frames` with no ROM exits with a clear error. `--debug` on a stub machine prints every section as `absent`.

**Manual:** one command, printed in full at test time: no ROM must exit with a clear error; a tiny file with `--debug --frames 1` must print the `gba-debug:` lines and the AV report with empty sections, and must not spam unimplemented opcodes.

**Done:** `cargo fmt`, `clippy -D warnings`, `cargo test`. Version `0.0.2`.

## Page 1 — ARM7TDMI and a tiny BIOS HLE

**Docs:** bring-up ARM row. GBATEK ARM CPU Reference. ARM DDI 0210C.

**Build:** registers, CPSR, ARM and Thumb decode, a 3-stage pipeline model that is correct about PC+8 / PC+4 and about flushing on taken branches. Exceptions enough for the tests (reset, SWI, IRQ stub). Unimplemented encodings return `StepError` and increment the fault counter instead of guessing.

BiosHle is only what `arm.gba` and `thumb.gba` need to reach idle with `r12 == 0`: the SWIs those ROMs call, and the idle halt. It is not the full BIOS catalogue, and it is not a copy of the deleted HLE. A real BIOS file is never read on this page.

Vendor the two ROMs under `tests/fixtures/jsmolka/` with upstream license and the pin in the fixture README. Tests skip if the files are absent.

**Manual:** two commands, printed in full at test time, each `cargo run --release -- --debug --frames 30` on `arm.gba` and `thumb.gba`. Pass is `result=PASS r12=0`. A fail run is the same command with `--debug=trace`. The first unimplemented mnemonic is the line to read.

**Done:** both ROMs pass locally. Default `cargo test` asserts them when the fixtures are present and skips when they are not. Version `0.0.3`.

## Page 2 — Bus and memory

**Docs:** bring-up bus row. GBATEK memory map, mirrors, Game Pak waitstates, open bus.

**Build:** `bus` routes BIOS, IWRAM, EWRAM, I/O, palette, VRAM, OAM, ROM, SRAM. Bad widths and unused bits return the open-bus value and log the address once in `--debug`. Waitstate counters exist even if every region is still 1 cycle; the summary shows them so page 8 has somewhere to put real N/S/I costs.

**Manual:** one printed command, `--debug --frames` on `memory.gba`. Pass is `r12=0` and no unexpected `openbus` line. A unit test pokes a bad address and the summary names the region.

**Done:** `memory.gba` passes locally. Version `0.0.4`.

## Page 3 — Timers, IRQ, keypad

**Docs:** bring-up timers/IRQ row. GBATEK Timers, Interrupt Control, Keypad Input.

**Build:** timers 0–3, IE/IF/IME, the keypad register. IRQ entry uses the CPU exception path from page 1. `--debug` prints IE, IF, IME, and each timer’s counter and cascade bit.

The named ROM gate is an in-house `simple-irq.gba`. Until that file exists the harness skips and says so. mGBA `io-read` and `timer-irq` stay stretch stubs, not the must.

**Manual:** `cargo test` for overflow, cascade, and one IRQ. If `simple-irq.gba` exists, one printed `--debug --frames` command. The `irq` line shows IF when a button is down. No extra key flag.

**Done:** unit tests cover overflow, cascade, and one IRQ taken. `simple-irq` passes when the ROM is added. Version `0.0.5`.

## Page 4 — PPU, and a picture you can open

**Docs:** bring-up PPU row. GBATEK LCD Video Controller.

**Build:** modes 0–5, scanline timing, backgrounds, sprites, windows, blend, mosaic. Frame is 240×160. Core stores the hardware pixel. The AV report prints the SHA-256 of one settled frame and the non-zero pixel count. No picture-file flag.

jsmolka `ppu/hello`, `shades`, and `stripes` are the gate. Hash the settled frame.

Windows: implement GBATEK’s rule, and leave the `X1 > X2` wrap disagreement (`docs/04-open-questions.md`) as a named failing or ignored test. Do not silently copy the old tree’s wrap behavior.

**Manual:** three printed commands, `--debug --frames 5` on hello, shades, and stripes. The AV report hash matches the fixture.

**Done:** three hashes match. Version `0.0.6`.

## Page 5 — DMA

**Docs:** bring-up DMA row. GBATEK DMA Transfers.

**Build:** immediate, VBlank, HBlank, special FIFO, DMA3 Game Pak. SRAM as a DMA source or dest fails clearly. CPU is stalled for the transfer and the summary shows channel, word count, and reason.

mGBA dma ROM stays off while `suite.gba` is absent. The gate is unit tests.

**Manual:** `cargo test` for the five paths. The debug line shows words moved, and `sram-dma: rejected` when a test asks for SRAM DMA. A ROM command is printed only if a fixture for that path exists.

**Done:** unit tests cover the five paths. Version `0.0.7`.

## Page 6 — APU, and a wav you can play

**Docs:** bring-up APU row. GBATEK Sound Controller.

**Build:** four PSG channels plus two DMA FIFOs. Mixing to PCM stays in the core at the hardware rate. Resample and the device stay in the host, same as `graycart-gb` `frontend/audio`. The debug record is the `gba-debug: apu health` line and the `APU` row of the AV report.

`cajunpanda/gba-audio-test` is the stretch ROM. If it is not vendored, the live gate is the unit tests, and a `gba-debug:` line says that ROM gate was skipped.

**Manual:** one printed command, `--debug --frames 120` on a ROM that plays a tone. Read `apu health` and any `warn apu` line. Underrun, empty-drain, clip, and `dc≈` are the fields that say what is wrong.

**Done:** unit tests for PSG enable, FIFO refill, and “SRAM is not a FIFO source”. Version `0.0.8`.

## Page 7 — Cartridge, saves, BIOS surface

**Docs:** bring-up cart/BIOS row. GBATEK Cartridges and BIOS Functions.

**Build:** ROM waitstate mirrors, SRAM, flash 64K/128K, EEPROM. Header parse with a clear error on a truncated file. Save sidecar next to the ROM, gitignored.

Grow BiosHle from the page-1 subset to the SWIs the jsmolka BIOS and save ROMs call. Still no committed `gba_bios.bin`. An optional local BIOS path is LLE later, off by default.

**Manual:** five printed commands, `--debug --frames 30` on bios, none, sram, flash64, and flash128. The report names the save kind. `r12=0` on each.

**Done:** those five pass locally. Version `0.0.9`.

## Page 8 — Prefetch and real waitstates

**Docs:** bring-up timing row. GBATEK Game Pak prefetch and waitstates.

**Build:** N/S/I costs and the prefetch buffer. IRQ delay if GBATEK specifies it for this slice. `--debug` prints stall cycles and prefetch hits, so a slow ROM is measurable before a frontend exists.

mGBA timing and dma numbers stay 0 until `suite.gba` is vendored. Do not mark them passed.

**Manual:** the same printed `memory.gba` and `arm.gba` commands as pages 2 and 1. They still pass. The waitstate field is non-zero on ROM fetches. If one regresses, rerun that command with `--debug=trace` and fix the earliest wrong memory cycle.

**Done:** previous ROM gates still pass, unit tests cover one N and one S access. Version `0.0.10`.

## Page 9 — Game Boy carts through `graycart`

**Docs:** `docs/01-sm83-reuse.md`, `docs/02-one-host-two-machines.md`, `docs/04-open-questions.md`.

**Build:** depend on the `graycart` crate for the SM83 machine. This crate owns the switch only: boot in GBA mode, `WAITCNT` bit 15 as the cart-shape sense, `DISPCNT` bit 3 to prepare, `HALTCNT` to apply, `4000800h` bit 3 for CGB boot-ROM disable. After the switch the ARM core does not execute. A `.gb` / `.gbc` file is handed to `graycart` with the AGB boot registers from Pan Docs (A=`$11`, B=`$01`).

Do not reimplement SM83, the Game Boy PPU, or the Game Boy APU. Do not resolve the Pan Docs vs GBATEK CGB-audio disagreement on this page; the Game Boy machine keeps the `graycart` APU, and the debug line says which description is unimplemented.

Check the open Cargo question before the dependency lands: `graycart`’s package manifest still lists winit, egui, and cpal. The GBA **library** must not use them. If a normal dependency pulls those into the lib, depend in a way that keeps them on the binary only, or stop and fix the GB crate’s manifest first.

**Manual:** three printed commands, `--debug --frames` on one local `.gb`, one local `.gbc`, and one `.gba`. The report says `machine=sm83` or `machine=arm7` and the handoff registers. No window.

**Done:** a small ROM of each kind reaches a frame without an ARM opcode running in SM83 mode. Version `0.0.11`.

## Page 10 — One play host

**Docs:** `docs/02-one-host-two-machines.md`, `graycart-gb` `docs/architecture.md`.

**Build:** frontend binary only. eframe / winit / wgpu / egui / cpal, same roles as `graycart-gb` `frontend/`. One file picker. Extension and header choose ARM vs `graycart`. Shade-to-RGB and device resample live here. The debug summary stays available as a panel fed by `MachineDebug`, not a second logger.

`--frames` remains headless and must not link the window.

**Manual:** open one `.gba` and one `.gbc` from the window. Then two printed commands, `--debug --frames 60` on those same files, with no window. Audio device follows the GB rule: chosen device, else OS default, else a named fallback, else silent. The `apu health` line still prints.

**Done:** both paths show a frame in the window and the headless command still works. Version `0.1.0` only if this is the first runnable host; otherwise the next patch.

## After each page

Same three commands, from `graycart-gba/`:

```text
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Then the manual command for that page. If the summary’s first fault is in an earlier subsystem, fix that subsystem. Do not patch the test ROM’s title.
