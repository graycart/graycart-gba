# Changelog

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
