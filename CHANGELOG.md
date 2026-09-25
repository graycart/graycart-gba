# Changelog

## 0.2.8

CGB color uses the 1.7 brightness curve on the SM83 present path only. A 16-bit DMA from unused I/O keeps both halves of the DMA open-bus latch for the next CPU read. A 32-bit DMA from unused I/O samples the CPU data latch after the enabling instruction's own load. An ARM instruction fetch leaves the pipeline prefetch at PC+8 in that latch, so a following unused halfword read does not replace it. DMA latches addresses only on CNT_H enable 0→1. Changing start timing to Immediate while already enabled starts a transfer with the latched addresses and leaves Enable set. I/O and OAM 32-bit accesses cost two halfwords; DMA I/O stays one beat. DMA destination writes land after their access beat. Timer CNT_H rising enable is latched until the next tick (pending value readable); falling enable stops immediately. ARM STRH pays two elapsed cycles so a halfword DMACNT enable starts DMA on the same cycle as a word enable. ARM LDRH pays the trailing internal cycle. A higher-priority DMA that preempts on the cycle HBlank was armed still pays the two-cycle startup before it reads. CpuSet/CpuFastSet elapse approximate BIOS loop overhead (not cycle-counted from a BIOS dump) so mid-SWI DMA sees live timers. I/O and OAM accesses end the Game Pak sequential burst. ARM B/BL/BX pay the remaining 1N+1S pipeline refill after the opcode fetch. ARM LDR pays a trailing internal cycle. A non-sequential 32-bit Game Pak access is N+1+S. A taken Thumb branch pays the remaining non-sequential refill cycle. A higher-priority DMA waits until the active unit finishes before it preempts. HBlank that arrives during a read is armed and runs at the next safe point. Thumb STR, STRB, and STRH cost 2N. A timer waits one cycle after its start edge before the first increment. HBlank on a later DMA unit boundary reads before that cycle's timer tick; HBlank during the first unit still waits until the write finishes. HBlank on the destination beat of a non-final unit reads before that beat's timer tick, and HBlank on the following source beat reads on the idle before it. An internal DMA preempt leaves the parent's next Game Pak read sequential. A Game Pak byte read latches the aligned halfword in both bus halves. A DMA mode change to Immediate pads with a sequential Game Pak word. DMA into EWRAM does not insert an extra idle between units. SOUNDCNT_H bits 4-7 read as zero. MRS SPSR in User or System mode reads CPSR. Thumb code fills the prefetch buffer during I/O and internal cycles. An empty Thumb PUSH stores PC+6 and drops SP by one word. An empty Thumb POP adds 64 to SP and makes the following opcode fetches non-sequential until the prefetch buffer refills. Display IRQs still latch when a DMA transfer crosses the VBlank, HBlank, or VCount edge. ARM code fills the prefetch buffer during I/O the same way Thumb does. A CPU timer enable does not count the opcode fetch that already finished, and enabling a timer whose counter is 0xFFFF overflows once before the new reload is used. HBlank DMA waits until that instruction finishes, then takes two startup cycles before it reads.

## 0.2.7

A 32-bit Game Pak access costs N+S (non-sequential) or S+S (sequential). Prefetch word hit still costs 1. Flash64/flash128 save ROM harness cap raised from 30 to 60 frames.

## 0.2.6

DMA3 timing 3 copies on the video-capture scanline. Channels 0–2 with timing 3 still do not copy. SRAM DMA still rejected.

## 0.2.5

Affine objects draw. `hello.gba` hash unchanged.

## 0.2.4

BIOS sqrt, arctan, affine set, BitUnPack, SoundBias, SoftReset, and RegisterRamReset. Music and multiboot SWIs warn and return.

## 0.2.3

BIOS LZ77, Huffman, RLE, and diff filters.

## 0.2.2

BIOS CpuSet and CpuFastSet copy and fill.

## 0.2.1

BIOS IntrWait and VBlankIntrWait watch `0x03007FF8`.

## 0.2.0

The play window is the Game Boy shell: same menus, settings, input configurator, audio device list, and debug monitor. Extension `.gba` drives this crate’s ARM machine; `.gb` / `.gbc` drive `graycart`. Headless `--frames` still does not link the window.

## 0.1.0

One play window with a file picker: `.gba` runs the ARM machine, `.gb` / `.gbc` run through `graycart`. Headless `--frames` does not link the window (window crates sit behind the `frontend` feature).

## 0.0.14

Game Boy carts run on the `graycart` SM83. This crate only switches: boot stays in GBA mode, WAITCNT bit 15 is the cart-shape sense, DISPCNT bit 3 prepares, a HALTCNT stop applies it, and `0x04000800` bit 3 disables the CGB boot ROM. After the switch the ARM core does not execute. A `.gb` / `.gbc` file is handed over with A=`$11` and B=`$01`. The CGB-audio disagreement and the GBA brightness ramp stay unimplemented.

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
