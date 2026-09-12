<!--
Cited: jsmolka/gba-tests memory/
URL: https://github.com/jsmolka/gba-tests/tree/master/memory
Note: path/license stub for P2 memory gate ROM; no binary vendored in this commit.
-->
# jsmolka memory fixtures

**Expected file (when vendored):** `memory.gba`  
**Upstream:** [jsmolka/gba-tests `memory/`](https://github.com/jsmolka/gba-tests/tree/master/memory)  
**License:** MIT — see [`../LICENSE`](../LICENSE)  
**Phase gate:** **P2 exit** (bus map / mirrors / waitstates; DMA Immediate covered by unit tests)

Related upstream ROMs (also not vendored here): mirrors / `video_strb` under the same suite.

Do not commit Nintendo BIOS or `.gba` binaries in this stub commit. Prefer upstream prebuilt `.gba` (FASMARM) when the harness is ready.
