<!--
Saved verbatim from the research pass, including its source list and the claims that failed verification.
Links and people are indexed in provenance/SOURCES.md. Open gaps are copied into 04-open-questions.md.
-->

# Research result

**Status: Partial**

A GBA emulator here is a separate ARM7TDMI machine, built only after a named hardware reference and a named acceptance ROM exist. Game Boy and Game Boy Color carts stay on the SM83 library from the GB emulator: that 8-bit mode never runs at the same time as the ARM core, and it is not reimplemented inside the GBA tree. The two projects meet as a play host plus a library, not as one Cargo workspace, and that dependency is specified but not wired. SUPER ZSNES v0.300 does not change the GBA plan; its new chips are already deferred SNES tracks, and its per-game special cases do not fit the SNES accuracy rules.

### When a task is allowed to start

GBA-native behavior starts from GBATEK, then a named test-ROM expectation, and only then secondary notes from mGBA, NanoBoyAdvance, and Tonc. A task that cannot name both its reference and its acceptance test is not ready to implement. The published guide points that research pack at the project-store phase list and implementation plan before phase work. [S1]

### Acceptance order

Gates run from the CPU outward. A missing fixture stays skipped or recorded as zero; it is not treated as a pass.

| Phases | What has to pass | Reference and gap |
| --- | --- | --- |
| P1–P2 | jsmolka arm.gba and thumb.gba (P1), then memory.gba (P2), under BiosHle with r12==0 after idle. Upstream pin a7113b67e63f83a9b321696ddd7042ccfad6c881. | GBATEK ARM CPU reference and ARM DDI 0210C, plus the GBA memory map. [S2] |
| P3 | Timers and IRQ. The named ROM is the in-house simple-irq test (G3-irq-rom), still ignored until simple-irq.gba exists. mGBA io-read and timer-irq are stretch stubs only. | GBATEK GBA Timers and Interrupt Control. [S3] |
| P4–P5 | Picture: jsmolka ppu/hello, shades, and stripes, hashed as the SHA-256 of a settled 240×160 RGB888 frame. DMA is unit-gated for VBlank, HBlank, FIFO special, and DMA3 Game Pak, with SRAM rejected (only DMA3 may read Game Pak ROM; SRAM is CPU byte-only). The mGBA dma ROM is not run because suite.gba is absent. | GBATEK LCD Video Controller and DMA Transfers. [S4] |
| P6–P8 | APU is four CGB-compatible PSG channels plus two DMA FIFOs, unit-gated because cajunpanda/gba-audio-test is absent. P7 musts are jsmolka bios plus saves none, sram, flash64, and flash128 under BiosHle. A real BIOS image is user-supplied and never committed. P8 records mGBA timing and dma passes as 0 until suite.gba is vendored. | GBATEK sound channels and the conformance board. [S5] |

### The 8-bit path is the existing SM83

CGB compatibility mode is a Z80/8080-style 8-bit CPU at 4.2 MHz or 8.4 MHz. DMG compatibility mode is that CPU at 4.2 MHz only. Those modes cannot run at the same time as the 16.78 MHz ARM7TDMI. [S6]

That CPU is the same Sharp SM83 used on the Game Boy, present only for backwards compatibility, and it is the core the GB library already implements. [S7]

The GBA always boots in GBA mode. The BIOS switches to CGB mode when WAITCNT 4000204h bit 15 reads the cartridge-shape switch, which also changes supply from 3.3 V to 5 V. DISPCNT bit 3 prepares the switch, and a write to HALTCNT 4000301h applies it. [S8]

Disabling the CGB boot ROM with GBA port 4000800h bit 3 skips the CGB intro, matches the CGB ROMDIS pin, and unlocks SM83 ports FF60h, FF61h, and FF63h. The cart must then provide an entry at 0000h and select CGB or DMG mode itself. [S9]

After a native color boot, both CGB and AGB leave A=$11. The AGB/AGB0 boot ROM leaves B=$01 while the CGB boot ROM leaves B=$00, so bit 0 of B distinguishes a GBA from a CGB. CGB colors use a different intensity ramp on GBA, and medium CGB brightness appears black. [S10]

### How the two projects join

The GBA crate is required to reuse the GB crate for 8-bit behavior and is forbidden to reimplement the SM83 machine. Version 0.0.1 has an empty dependency table and a stub library, so no module is linked or wrapped, and there is still no GBA CPU, bus, PPU, APU, or host. [S11][S15][S16]

The family assigns the graycart-gba crate and binary as the long-term play host for both GBA and DMG/CGB, and graycart-gb as the DMG/CGB library plus the maintenance host that left daily-target status at P12. This is not a Cargo workspace; each emulator is tested on its own. [S12]

The P12 cutover keeps one preferred play target without replacing that library: new 8-bit and GBA play goes to graycart-gba, while graycart-gb remains the intentional SM83 library. Its package is graycart 0.11.5, and its app is no longer the daily play target. [S13][S14]

### What SUPER ZSNES v0.300 does not transfer

v0.300 adds SA-1, Cx4, and SuperFX3 emulation, SNES Mouse and Super Scope from per-game config, and per-game overclock settings. [S17]

SA-1, SuperFX/GSU-1/2, and Cx4 are already deferred SNES chip tracks and must not block P0–P10. Until they are implemented, an unsupported chip fails as UnsupportedCartFeature at load, with no partial math. [S19]

The SNES accuracy plan forbids title-specific core hacks and game-specific special cases, keeps shade-to-RGB and display filters in the host, and treats other emulators as cross-checks only. [S18]

MSU-1 is an explicit v1 non-goal — optional later, a fan path, not OEM — so making it a must-have conflicts with that plan. [S20]

## Sources
- [S1] "Agent guidelines — Graycart (graycart-gba)" — "C:\\Users\\axolatl-tank\\Projects\\graycart\\graycart-gba\\AGENTS.md"
- [S2] "jsmolka/gba-tests fixtures (P1/P2 musts)" — "C:\\Users\\axolatl-tank\\New Projects\\graycart\\graycart-gba\\tests\\fixtures\\jsmolka\\README.md"
- [S3] "P3 timers / IRQ / input harness" — "C:\\Users\\axolatl-tank\\New Projects\\graycart\\graycart-gba\\tests\\roms\\p3.rs" (independently checked against "P3 timers / IRQ harness and GBATEK timer/IRQ citations" — "C:\\Users\\axolatl-tank\\New Projects\\graycart\\graycart-gba\\tests\\roms\\p3.rs")
- [S4] "GBATEK — GBA LCD Video Controller, Memory Map, DMA Transfers" — "https://problemkaputt.de/gbatek.htm" (independently checked against "graycart-gba P4/P5 gates and GBATEK LCD/DMA" — "C:\\Users\\axolatl-tank\\New Projects\\graycart\\graycart-gba\\tests\\roms\\p4.rs")
- [S5] "graycart-gba conformance board (P7–P8)" — "C:\\Users\\axolatl-tank\\New Projects\\graycart\\graycart-gba\\docs\\conformance.md" (independently checked against "graycart-gba conformance board and P6/P7 harnesses" — "C:\\Users\\axolatl-tank\\New Projects\\graycart\\graycart-gba\\docs\\conformance.md")
- [S6] "GBATEK GBA/NDS Technical Info" — "https://problemkaputt.de/gbatek.htm"
- [S7] "Game Boy Advance Architecture (Copetti)" — "https://www.copetti.org/writings/consoles/game-boy-advance/" (independently checked against "Game Boy Advance Architecture (Copetti); graycart-gb README" — "https://www.copetti.org/writings/consoles/game-boy-advance/")
- [S8] [S9] "GBATEK GBA Backwards Compatibility CGB Mode" — "https://problemkaputt.de/gbatek-gba-backwards-compatibility-cgb-mode.htm"
- [S10] "Pan Docs Power-Up Sequence" — "https://gbdev.io/pandocs/Power_Up_Sequence.html"
- [S11] "graycart-gba README" — "C:\\Users\\axolatl-tank\\Projects\\graycart\\graycart-gba\\README.md" (independently checked against "graycart-gba README, AGENTS.md, Cargo.toml, lib.rs" — "C:\\Users\\axolatl-tank\\Projects\\graycart\\graycart-gba\\README.md")
- [S12] [S13] "Graycart family README" — "C:\\Users\\axolatl-tank\\Projects\\graycart\\README.md"
- [S14] "graycart-gb README" — "C:\\Users\\axolatl-tank\\Projects\\graycart\\graycart-gb\\README.md"
- [S15] "graycart-gba AGENTS.md" — "C:\\Users\\axolatl-tank\\Projects\\graycart\\graycart-gba\\AGENTS.md"
- [S16] "graycart-gba Cargo.toml" — "C:\\Users\\axolatl-tank\\Projects\\graycart\\graycart-gba\\Cargo.toml" (independently checked against "graycart-gba Cargo.toml and src/lib.rs" — "C:\\Users\\axolatl-tank\\Projects\\graycart\\graycart-gba\\Cargo.toml")
- [S17] "SUPER ZSNES - SNES Emulator (Latest Release Notes v0.300)" — "https://www.zsnes.com/"
- [S18] "Agent guidelines -- Graycart (graycart-snes)" — "C:\\Users\\axolatl-tank\\Projects\\graycart\\graycart-snes\\docs\\AGENTS.md"
- [S19] "06 — Cart mapping / chips" — "C:\\Users\\axolatl-tank\\Projects\\graycart\\graycart-snes\\docs\\06-cart-mapping-chips.md"
- [S20] "graycart-snes — Architecture overview (non-goals)" — "C:\\Users\\axolatl-tank\\Projects\\graycart\\graycart-snes\\docs\\00-architecture-overview.md"

## Coverage and uncertainty
- "Question 1 uncertainty: The Project-store PHASES.md and 08-implementation-plan.md were not present in this workspace, so must-versus-stretch wording is taken from the as-built changelog, harnesses, and GitHub AGENTS.md rather than that checklist."
- "Question 1 uncertainty: The workspace graycart-gba tree is a greenfield wipe whose AGENTS.md says not to copy the old 0.1.x phases back; the sequence above is the prior contract in New Projects and on GitHub, not code in the current tree."
- "Question 1 uncertainty: GBATEK is the named primary, but it is not treated as complete silicon truth: the changelog records GBATEK’s window X1>X2 clamp as an oversimplification versus hardware wrap."
- "Question 2 uncertainty: No graycart-gba source lists which graycart-gb modules (cpu, bus, cart, hw, timer, ppu, apu, input, boot) can be linked unchanged versus wrapped."
- "Question 2 uncertainty: Nintendo's AGB manual calls the compatibility processor an 8-bit CISC CPU and does not use the name SM83; cycle-identity with graycart-gb's SM83 core is not established by the sources above."
- "Question 2 uncertainty: Pan Docs says the GBA APU mixes digitally and withholds extra wave RAM and DMA FIFOs from CGB programs, while GBATEK still calls the four channels analogue CGB-compatible and only mentions sound BIAS as a CGB-mode register."
- "Question 2 uncertainty: GBATEK says it is unknown whether the ARM CPU can keep running after entering CGB mode."
- "Question 2 uncertainty: Retail GBA dropped a built-in infrared port, but that GBATEK note is about a GBA-mode prototype, not CGB FF56/RP behavior."
- "Question 3 uncertainty: No in-tree type or dispatch path shows how one graycart-gba play binary would select between a future GBA core and the graycart SM83 library."
- "Question 3 uncertainty: graycart-linux docs cite product-decision-supersede-gb.md as canonical for the supersede; that file is not in this workspace and was not read."
- "Question 3 uncertainty: graycart-gb Cargo.toml still lists winit, egui, and cpal as package dependencies even though frontend is only part of the binary; nothing states whether a downstream dependency on the graycart library is free of those host crates."
- "Question 4 uncertainty: The fetched zsnes.com page only publishes v0.300 notes. MSU-1 support and “initial Super FX” (v0.200b / v0.230b) are reported by RetroRGB and Time Extension, not restated on that official page."
- "Question 4 uncertainty: v0.300 names “SuperFX3” and credits Randy Linden; graycart-snes inventories retail SuperFX/GSU-1/2 only, and neither source defines whether SuperFX3 is that silicon or a faster non-retail variant."
- "Question 4 uncertainty: No graycart-snes document mentions SUPER ZSNES, so “worth adopting” is alignment with the written plan, not a recorded project decision."
- "Claim claim-1 was excluded by verification: P0 and P2–P8 match the changelog blurbs, but P1 is only a later ARM7TDMI bugfix gate, not a shipped phase between P0 and P2, and P9–P12 are not “host and DMG/CGB compat, not more native silicon”: P10 adds native WAITCNT.bit15, HALTCNT, and Mode-8 handoff, and P12 is a product cutover.."
- "Claim claim-16 was excluded by verification: The lib-versus-binary split is documented, but no source says that boundary presents play without folding machines together.."
- "Claim claim-19 was excluded by verification: The site does specify a GPU PPU for hi-res Mode 7 and per-game enhancements, but the parenthetical list is the separate Super Enhancement Engine, including overclock and uncompressed audio replacement, and the page never says the PPU is not stock S-PPU output; enhancements can be disabled individually.."
- "Claim claim-23 was excluded by verification: A standard pad is Must and Super Scope/Justifier are Later, but extra controllers are not uniformly scheduled later and not rejected: the mouse is Stretch, and P10 allows multitap/mouse to be explicit unsupported.."
