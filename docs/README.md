<!--
Cited: GBATEK (Martin Korth); Pan Docs (gbdev); Rodrigo Copetti, Game Boy Advance Architecture;
  SUPER ZSNES release notes; Graycart family and graycart-gb docs.
URLs: see provenance/SOURCES.md
Note: index for the greenfield GBA research pack. Not a reprint of those sources.
-->

# graycart-gba research pack

Reference for the greenfield rewrite. This folder records the bring-up order, what may be reused from `graycart-gb`, and every source the research pass actually used.

The 0.1.x machine was removed from this working tree. These notes are the gate for the next implementation. A task that cannot name a reference and an acceptance test is not ready to code.

| Doc | What it decides |
|-----|-----------------|
| [implementation-plan.md](./implementation-plan.md) | Pages 0–13. Page 7 is `0.0.9`. Next work is page 8. CLI and `gba-debug:` lines live here |
| [00-bring-up-order.md](./00-bring-up-order.md) | Hardware-first phases, primary docs, and the ROM or unit gate for each slice |
| [01-sm83-reuse.md](./01-sm83-reuse.md) | GBA Game Boy mode versus the SM83 core in `graycart-gb` |
| [02-one-host-two-machines.md](./02-one-host-two-machines.md) | One play app, two machines. Do not merge the cores |
| [03-super-zsnes.md](./03-super-zsnes.md) | What SUPER ZSNES v0.300 actually adds, and what does not belong in a GBA core |
| [04-open-questions.md](./04-open-questions.md) | Gaps the check could not close |
| [research-report.md](./research-report.md) | Verbatim cited report from the research pass |
| [provenance/SOURCES.md](./provenance/SOURCES.md) | People, projects, and links |
| [provenance/CLAIMS.md](./provenance/CLAIMS.md) | Which research claims survived an independent check |

Start with GBATEK, then a named test, then secondary notes (mGBA, NanoBoyAdvance, Tonc, Copetti). Commercial titles are smoke, not the accuracy test.
