<!--
Cited: SUPER ZSNES — https://www.zsnes.com/ (v0.300 release notes, Key Features, Super Enhancement Engine)
Cited: graycart-snes docs (AGENTS, architecture non-goals, cart chips, timing)
Note: enhancement features are a host or a later SNES chip track. They are not GBA core behavior.
-->

# SUPER ZSNES v0.300

Source: the project site, [zsnes.com](https://www.zsnes.com/). The page says the two original ZSNES developers rewrote the emulator and run a GPU PPU. It names Randy Linden in the SuperFX3 thanks, and thanks tssf (Chrono Trigger audio) and fatbrowne (save-state help). It does not print the original authors’ handles in the welcome text, so this pack does not invent them.

## What v0.300 actually adds

Checked against the release notes:

- Chip emulation: SA-1, Cx4, SuperFX3
- Per-game overclock
- SNES Mouse and Super Scope, enabled from the per-game config, with joypad override
- SPC700 timing, SuperFX fixes, mid-screen sprite updates, timing and open-bus fixes

The Super Enhancement Engine is a separate list, and each item can be turned off: manual high-resolution drawing, texture and normal maps, overclock, widescreen where the game already has some widescreen code, uncompressed audio replacement, and Mode 7 tiles replaced with 3D height data. A claim that this whole list *is* the GPU PPU, and that the PPU is specified as “not stock S-PPU output,” was **rejected**. The page puts hi-res Mode 7 and per-game enhancements on the GPU PPU line, and puts overclock and audio replacement on the Enhancement Engine.

## What graycart should take from that

| Idea | Where it belongs | Why |
|------|------------------|-----|
| SA-1, SuperFX / GSU, Cx4 | `graycart-snes` chip tracks, after the base console | Already deferred there. Unsupported chips must fail as an unsupported cart feature, not run partial math. They must not block the base phases |
| Mouse | SNES input, later | The SNES timing doc treats a standard pad as required. Mouse is a stretch, not a must. A claim that every extra controller is simply “later” was rejected |
| Super Scope | SNES input, later still | Listed as later in that same doc |
| GPU hi-res, widescreen, texture replacement, uncompressed audio swap | Host or an explicit enhancement mode | `graycart-snes` keeps shade-to-RGB and display filters in the host and forbids title hacks in the core |
| MSU-1 style audio | Not a v1 goal | The SNES architecture doc excludes MSU-1 from v1 |

None of these chips or enhancement passes are part of the GBA ARM7TDMI, PPU, or SM83 path. Do not port the SUPER ZSNES GPU renderer into `graycart-gba`.
