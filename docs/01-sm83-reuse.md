<!--
Cited: GBATEK — GBA Technical Data and GBA Backwards Compatibility CGB Mode
  https://problemkaputt.de/gbatek.htm
  https://problemkaputt.de/gbatek-gba-backwards-compatibility-cgb-mode.htm
Cited: Pan Docs — Power-Up Sequence
  https://gbdev.io/pandocs/Power_Up_Sequence.html
Cited: Rodrigo Copetti, Game Boy Advance Architecture, “Becoming a Game Boy Color”
  https://www.copetti.org/writings/consoles/game-boy-advance/
Note: summary of the compatibility handoff. The SM83 implementation stays in graycart-gb.
-->

# SM83 reuse

The GBA does not run Game Boy software on the ARM7TDMI. It boots in GBA mode and, for a Game Boy cartridge, switches to a separate 8-bit CPU. That CPU is the one `graycart-gb` already implements.

## What the documents say

Martin Korth, GBATEK technical data:

- CGB mode: Z80/8080-style 8-bit CPU at 4.2 MHz or 8.4 MHz.
- DMG mode: that CPU at 4.2 MHz only.
- ARM mode: ARM7TDMI at 16.78 MHz.
- The 8-bit mode and the ARM mode do not run at the same time.

Rodrigo Copetti describes that 8-bit CPU as the same Sharp SM83 used in the Game Boy, present only for backwards compatibility, at those two clocks. `graycart-gb` is the SM83 library. Do not write a second SM83 in this repo.

## How the switch works

GBATEK, backwards compatibility:

1. The GBA always boots in GBA mode.
2. The BIOS switches to CGB mode when `WAITCNT` (`4000204h`) bit 15 reads the cartridge-shape switch. That switch also changes the supply from 3.3 V to 5 V.
3. `DISPCNT` bit 3 prepares the switch. A write to `HALTCNT` (`4000301h`) applies it.
4. `4000800h` bit 3 disables the CGB boot ROM. That matches the CGB `ROMDIS` pin and unlocks SM83 ports `FF60h`, `FF61h`, and `FF63h`. The cartridge must then provide an entry at `0000h` and select CGB or DMG mode itself.

Copetti adds the physical side: the shape detector redirects voltage, joypad, cartridge, and WRAM buses. Setting GBC mode from `DISPCNT` is checked against the BIOS program counter during boot. The Game Boy Micro has no legacy slot but still has the SoC; that is a hardware fact, not a feature this emulator needs to fake.

## What `graycart-gb` must still get right

Pan Docs, power-up sequence, after a native color boot:

| Register | CGB boot ROM | AGB / AGB0 boot ROM |
|----------|--------------|---------------------|
| A | `$11` | `$11` |
| B | `$00` | `$01` |

Bit 0 of B is how software tells a GBA from a CGB. GBATEK also says the CGB color intensity ramp differs on GBA, and that medium CGB brightness appears black there. Palette math for GBA-hosted CGB is not “just use the DMG shader.”

## What to link, not rewrite

Link the `graycart` crate for the SM83 machine, its PPU, APU, timer, and mappers. This repo owns:

- the ARM7TDMI and the GBA bus
- the mode switch (`WAITCNT` bit 15, `DISPCNT` bit 3, `HALTCNT`)
- the GBA-side boot decision
- the AGB boot-register and color-ramp differences around the existing core

Version 0.0.1 does not depend on `graycart` yet. That dependency is the compat slice, not a merge of the two CPUs into one execute loop.
