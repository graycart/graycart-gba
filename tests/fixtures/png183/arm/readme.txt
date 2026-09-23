This version of arm.gba has been modified significantly from jsmolka's original version.

Changes:
- Many new tests have been added for R15 offsets on LDR/STR
- branches.asm, data_swap.asm, halfword_transfer.asm, and multiply.asm are no longer included in arm.asm, and are instead assembled separately. This was done because they test encodings that are listed as undefined, which may cause the tests to crash if emulated incorrectly.

branches.asm:
- Added tests for bits 5, 6, and 21 of BX, which all appear to be ignored by decode and execute stages
- Added tests for BX destination register field in bits 12-15. These tests also require Thumb mode to be checked during the decode stage, because the tests enter Thumb mode without a pipeline flush.
- Added tests for BX CPSR mask field in bits 16-19. These tests also rely on condition code 0xF being handled properly (always evaluates to false)

data_swap.asm:
- Added tests for R15 offsets
- Added tests for SWP bits 20, 21, and 23, which should all be ignored when decoding. Effect on execution is unknown, if any.
- Added tests for SWP timing. Should perform read cycle, write cycle, then idle cycle.

halfword_transfer.asm:
- Added tests for R15 offsets
- Added tests for future LDRD/STRD opcode space

multiply.asm:
- Added tests for R15 offsets
- Added tests for R15 as destination register (writing to R15 should fail)
- Added tests for bit 22 of instruction, which appears to be ignored by decode and execute stages
