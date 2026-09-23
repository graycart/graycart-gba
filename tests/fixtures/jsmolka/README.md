# jsmolka gba-tests

Vendored from [jsmolka/gba-tests](https://github.com/jsmolka/gba-tests) at `a7113b67e63f83a9b321696ddd7042ccfad6c881`. License: MIT (`LICENSE`). Upstream readme: `UPSTREAM.md`.

`unsafe/unsafe.gba` is not here. Upstream says those checks fail on hardware and were left out of the suite.

Pass is `r12 == 0`, except `thumb.gba`, which stores the id in `r7`. Idle with `r12 == 0` and `r7` in `1..=999` is `result=FAIL` (a thumb failure). `arm.gba`'s leftover `r7` sits outside that range. The `m_exit` macro moves the ARM test number into `r12`. A failing ROM is `gba-debug: cpu result=FAIL r12=<n> r7=<n>` with `pc` and the mnemonic. Upstream also draws that number in background mode 4.

| ROM | Role |
|-----|------|
| `arm/arm.gba`, `thumb/thumb.gba` | CPU pages |
| `memory/memory.gba` | Bus page |
| `ppu/hello.gba`, `shades.gba`, `stripes.gba` | Picture page |
| `bios/bios.gba`, `save/none.gba`, `sram.gba`, `flash64.gba`, `flash128.gba` | Cart and BIOS page |
| `nes/nes.gba` | In the suite. Not a gate until a page names it. Not a NES emulator test. |
