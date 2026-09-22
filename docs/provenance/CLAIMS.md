<!--
Note: outcome of the research verification pass. Supported claims are safe to build on. Rejected claims are written here so they are not reused as fact.
-->

# Checked claims

Twenty-four candidate claims were gathered. Twenty survived an independent read of the cited source. Four did not. The rejected ones are listed first so they are not rebuilt into the plan.

## Rejected

1. **Native phase list.** “P0 scaffold, P1 ARM7TDMI, P2 bus, … P8 timing, then P9–P12 are only host and DMG/CGB compat.” The changelog supports P0 and P2–P8. P1 in that file is a later ARM7TDMI bugfix gate, not a phase shipped between P0 and P2. P10 adds native `WAITCNT` bit 15, `HALTCNT`, and the Mode-8 handoff. P12 is a product cutover. Source that was checked: the old `CHANGELOG.md` in the New Projects checkout.

2. **Lib-versus-binary implies unmerged machines.** The architecture doc does split framebuffer/PCM/buttons from the window. It does not say that split is what keeps the ARM core and the SM83 core apart. Source: `graycart-gb/docs/architecture.md`.

3. **SUPER ZSNES GPU PPU equals the whole enhancement list.** The site does describe a GPU PPU for hi-res Mode 7 and per-game enhancements. Overclock and uncompressed audio replacement are in the Super Enhancement Engine section, and enhancements can be disabled one by one. The page does not say the PPU “is not stock S-PPU output.” Source: https://www.zsnes.com/

4. **Every extra SNES controller is simply later.** A standard pad is required. Super Scope and Justifier are later. The mouse is a stretch, and the P10 note allows multitap and mouse to be explicit unsupported. Source: `graycart-snes/docs/05-timing-irq-joypad-wram.md`.

## Supported

Hardware and compat:

- Research rule: GBATEK, then a named test ROM, then mGBA, NanoBoyAdvance, and Tonc. No reference and no acceptance test means no implementation. Source: `graycart-gba/AGENTS.md`.
- jsmolka `arm.gba` and `thumb.gba` are the P1 exit, `memory.gba` is the P2 exit, PASS is BiosHle with `r12 == 0` after idle, suite pin `a7113b67e63f83a9b321696ddd7042ccfad6c881`. CPU cites GBATEK and ARM DDI 0210C. Source: old `tests/fixtures/jsmolka/README.md`.
- Timers and IRQ cite GBATEK Timers and Interrupt Control. The in-house IRQ ROM stays ignored until `simple-irq.gba` exists. mGBA io-read and timer-irq are stretch stubs. Source: old `tests/roms/p3.rs`.
- P4 hashes jsmolka PPU hello, shades, and stripes as a settled 240×160 RGB888 frame against GBATEK’s LCD controller. P5 DMA unit-tests VBlank, HBlank, FIFO special, DMA3 Game Pak, and SRAM rejection. The mGBA dma ROM is not run while `suite.gba` is absent. Source: old `tests/roms/p4.rs` and https://problemkaputt.de/gbatek.htm
- P6’s live gate is the unit suite because the cajunpanda audio ROM is absent. P7’s must list is jsmolka bios plus save none, sram, flash64, and flash128 under BiosHle. LLE BIOS is user-supplied and uncommitted. P8 records mGBA timing and dma passes as 0 until `suite.gba` exists. Source: old `docs/conformance.md`.
- CGB mode is a Z80/8080-style CPU at 4.2 or 8.4 MHz. DMG mode is 4.2 MHz. ARM mode is a 16.78 MHz ARM7TDMI. The 8-bit and 32-bit modes do not run together. Source: https://problemkaputt.de/gbatek.htm
- Copetti identifies that 8-bit CPU as the Sharp SM83 at those clocks, only for compatibility, which is the `graycart-gb` library. Source: https://www.copetti.org/writings/consoles/game-boy-advance/
- Boot is always GBA mode. BIOS enters CGB mode when `4000204h` sees a Game Boy cart. Voltage goes from 3.3 V to 5 V. `DISPCNT` bit 3 prepares the switch. `HALTCNT` `4000301h` applies it. Source: https://problemkaputt.de/gbatek-gba-backwards-compatibility-cgb-mode.htm
- `4000800h` bit 3 disables the CGB boot ROM, matches CGB `ROMDIS`, unlocks `FF60h` / `FF61h` / `FF63h`, and the cart then needs an entry at `0000h` and must select CGB or DMG. Same GBATEK page.
- After native color boot, CGB and AGB both leave A=`$11`. CGB leaves B=`$00`. AGB leaves B=`$01`. Medium CGB brightness appears black on GBA because the ramps differ. Sources: https://gbdev.io/pandocs/Power_Up_Sequence.html and GBATEK.

Repository shape:

- This tree says to reuse `graycart-gb` and not reimplement the SM83. Version 0.0.1 has no such dependency and no compat wrapper yet. Sources: `README.md`, `AGENTS.md`, `Cargo.toml`.
- The umbrella and `graycart-gb` agree: this binary is the play host; `graycart` stays the SM83 library.
- `graycart-snes` forbids title hacks, keeps filters in the host, and treats other emulators as cross-checks.
- SA-1, SuperFX/GSU, and Cx4 are deferred chip tracks through the base SNES phases and must error as an unsupported cart feature.
- MSU-1 is excluded from SNES v1.
- SUPER ZSNES v0.300 adds SA-1, Cx4, SuperFX3, per-game overclock, and SNES Mouse and Super Scope from the per-game config. Source: https://www.zsnes.com/
