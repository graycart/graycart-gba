# Conformance fixtures

Each folder is someone else's suite. `LICENSE` in that folder is the license for those files. The `graycart-gba` crate stays MIT.

| Folder | Upstream | Commit | License |
|--------|----------|--------|---------|
| `jsmolka/` | [jsmolka/gba-tests](https://github.com/jsmolka/gba-tests) | `a7113b67e63f83a9b321696ddd7042ccfad6c881` | MIT |
| `alyosha/` | [alyosha-tas/gba-tests](https://github.com/alyosha-tas/gba-tests) | `66f5f1d60cd7030b600606886c76f5c821ecaf9d` | MIT |
| `png183/` | [png183/gba-tests](https://github.com/png183/gba-tests) | `87937453b780e7fafc64eef80b89293ead9c636c` | MIT |
| `mgba-suite/` | [mgba-emu/suite](https://github.com/mgba-emu/suite) | `e6942030d25ffe3ba76c72b73a86da073ec857cc` | MIT |
| `nba/` | [nba-emu/hw-test](https://codeberg.org/nba-emu/hw-test) | `fbc99140e06f083c0a47612467cfbb02470e56dc` | BSD-3-Clause |
| `hades/` | [hades-emu/Hades-Tests](https://github.com/hades-emu/Hades-Tests) | `29274ce31005f45bf5ca214bbb599377633a511e` | GPL-2.0 |
| `fuzzarm/` | [DenSinH/FuzzARM](https://github.com/DenSinH/FuzzARM) | `a675329cd57da48e3e406216ba2d79dd7e09ee20` | GPL-3.0 |

`mgba-suite/` is the source tree. It has no built `suite.gba`. A harness skips until that ROM exists.

`fuzzarm/` does not include the bundled FASMARM assembler. That directory had no license file.
