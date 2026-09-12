<!--
Cited: cajunpanda/gba-audio-test (MIT)
URL: https://github.com/cajunpanda/gba-audio-test
Note: P6 APU smoke / soft WAV gate; Nintendo boot logo in ROM header excluded.
-->
# gba-audio-test fixtures

Upstream: [cajunpanda/gba-audio-test](https://github.com/cajunpanda/gba-audio-test) (MIT).

| File | Status |
|------|--------|
| `gba-audio-test.gba` | **Absent** — opt-in `#[ignore]` matrix in `tests/roms/p6.rs` |
| `soft-ref.wav` | **Absent** — soft golden when capture policy signed |
| `directaudiotest.gba` | **Absent** — mGBA #1847 stretch (G6-fifo-timing) |

Do not vendor Nintendo BIOS. Boot logo bytes in upstream builds are Nintendo property — cite, don't redistribute if uncertain.
