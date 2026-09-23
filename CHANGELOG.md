# Changelog

## 0.0.13

Real Game Pak N/S/I waitstates and the opcode prefetch buffer. The wait line reports WS0 N/S (reset 4/2), stall cycles since boot, and prefetch hits.

## 0.0.12

Cartridge saves (none / sram / flash64 / flash128 / eeprom), `<rom>.sav` sidecar load/flush, BIOS prefetch latch already on this page, and one `gba-debug: warn cart gpio unsupported` when a test marks GPIO present. `--debug` stops on pass or fail and exits non-zero on fail. `--frames` caps that run and is required when `--debug` is absent.

## 0.0.11

Four PSG channels, two DMA FIFOs, and the mixer at 32768 Hz in the core. SRAM is not a FIFO source. The audio-test ROM gate is skipped when the ROM is absent.

## 0.0.10

DMA immediate, vblank, hblank, FIFO special, DMA3 Game Pak; SRAM DMA rejected; CPU stalls for the copy.

## 0.0.9

Windows, blend, and mosaic. X1>X2 follows GBATEK (empty range). Hardware wrap stays ignored.

## 0.0.8

Text and affine backgrounds, sprites, and locked `shades.gba` / `stripes.gba` frame hashes. VCOUNT reads return the current scanline.

## 0.0.7

Bitmap modes 3–5, `hello.gba` settled-frame SHA-256, and DISPSTAT vblank IRQ on rising edge.

## 0.0.6

Timers 0–3, IE/IF/IME, keypad, and halt (SWI 2 / HALTCNT) that wakes on an enabled IRQ. `--debug` prints live IE/IF/IME and timer counters.

## 0.0.5

Bus and memory map: mirrors, open bus, BIOS prefetch data reads, SRAM 8-bit region, video byte-store rules, SIO unlinked stub, waitstate counters (still 1). `memory.gba` reaches idle with `r12 == 0`.

## 0.0.4

Thumb interpreter (DDI 0210C formats 1–19). `thumb.gba` reaches idle with `r7 == 0`. `arm.gba` still passes.

## 0.0.3

ARM7TDMI interpreter. `arm.gba` reaches idle with `r12 == 0`. Thumb encodings that ROM does not execute still fault.

## 0.0.2

Scaffold. Empty module owners, a headless CLI, and the `gba-debug:` report. No CPU.

## 0.0.1

Empty tree.
