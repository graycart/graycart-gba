format binary as 'gba'

include '../lib/constants.inc'
include '../lib/macros.inc'

macro m_exit test {
        m_half  r12, test
        b       eval
}

header:
        include '../lib/header.asm'

main:
        m_test_init

        ; Reset test register
        mov     r12, 0

halfword_transfer:
        ; Tests for the halfword data transfer instruction
        mem     equ r11
        mov     mem, MEM_IWRAM
        add     mem, 0x1500

t400:
        ; ARM 8: Store halfword
        mvn     r0, 0
        strh    r0, [mem]
        lsr     r0, 16
        ldr     r1, [mem]
        cmp     r1, r0
        bne     f400

        add     mem, 32
        b       t401

f400:
        m_exit  400

t401:
        ; ARM 8: Load halfword
        mvn     r0, 0
        str     r0, [mem]
        lsr     r0, 16
        ldrh    r1, [mem]
        cmp     r1, r0
        bne     f401

        add     mem, 32
        b       t402

f401:
        m_exit  401

t402:
        ; ARM 8: Load unsigned halfword
        mov     r0, 0x7F00
        strh    r0, [mem]
        ldrsh   r1, [mem]
        cmp     r1, r0
        bne     f402

        add     mem, 32
        b       t403

f402:
        m_exit  402

t403:
        ; ARM 8: Load signed halfword
        mov     r0, 0xFF00
        strh    r0, [mem]
        mvn     r0, 0xFF
        ldrsh   r1, [mem]
        cmp     r1, r0
        bne     f403

        add     mem, 32
        b       t404

f403:
        m_exit  403

t404:
        ; ARM 8: Load unsigned byte
        mov     r0, 0x7F
        strb    r0, [mem]
        ldrsb   r1, [mem]
        cmp     r1, r0
        bne     f404

        add     mem, 32
        b       t405

f404:
        m_exit  404

t405:
        ; ARM 8: Load signed byte
        mov     r0, 0xFF
        strb    r0, [mem]
        mvn     r0, 0
        ldrsb   r1, [mem]
        cmp     r1, r0
        bne     f405

        add     mem, 32
        b       t406

f405:
        m_exit  405

t406:
        ; ARM 8: Indexing, writeback and offset types
        mov     r0, 32
        mov     r1, 4
        mov     r2, mem
        strh    r0, [r2], 4
        ldrh    r3, [r2, -r1]!
        cmp     r3, r0
        bne     f406
        cmp     r2, mem
        bne     f406

        add     mem, 32
        b       t407

f406:
        m_exit  406

t407:
        ; ARM 8: Aligned store halfword
        mov     r0, 32
        strh    r0, [mem, 1]
        ldrh    r1, [mem]
        cmp     r1, r0
        bne     f407

        add     mem, 32
        b       t408

f407:
        m_exit  407

t408:
        ; ARM 8: Misaligned load halfword (rotated)
        mov     r0, 32
        strh    r0, [mem]
        ldrh    r1, [mem, 1]
        cmp     r1, r0, ror 8
        bne     f408

        add     mem, 32
        b       t409

f408:
        m_exit  408

t409:
        ; ARM 8: Misaligned load signed halfword
        mov     r0, 0xFF00
        strh    r0, [mem]
        mvn     r0, 0
        ldrsh   r1, [mem, 1]
        cmp     r1, r0
        bne     f409

        add     mem, 32
        b       t410

f409:
        m_exit  409

t410:
        ; ARM 8: Store writeback same register
        mov     r0, mem
        dw      0xE1E000B4  ; strh r0, [r0, 4]!
        add     r1, mem, 4
        cmp     r1, r0
        bne     f410

        ldr     r1, [r0]
        mov     r2, mem
        bic     r2, 0xFF000000
        bic     r2, 0xFF0000
        cmp     r2, r1
        bne     f410

        add     mem, 32
        b       t411

f410:
        m_exit  410

t411:
        ; ARM 8: Store writeback same register
        mov     r0, mem
        dw      0xE0C000B4  ; strh r0, [r0], 4
        sub     r0, 4
        cmp     r0, mem
        bne     f411

        ldr     r1, [r0]
        mov     r2, mem
        bic     r2, 0xFF000000
        bic     r2, 0xFF0000
        cmp     r2, r1
        bne     f411

        add     mem, 32
        b       t412

f411:
        m_exit  411

t412:
        ; ARM 8: Load writeback same register
        mov     r0, mem
        mov     r1, 32
        str     r1, [r0], -4
        dw      0xE1F000B4  ; ldrh r0, [r0, 4]!
        cmp     r0, 32
        bne     f412

        add     mem, 32
        b       t413

f412:
        m_exit  412

t413:
        ; ARM 8: Load writeback same register
        mov     r0, mem
        mov     r1, 32
        strh    r1, [r0]
        dw      0xE0D000B4  ; ldrh r0, [r0], 4
        cmp     r0, 32
        bne     f413

        add     mem, 32
        b       t414

f413:
        m_exit  413

t414:
        ; ARM 8: Store halfword PC + 4
        dw      0xE04BF0B0  ; strh    pc, [mem]
        mov     r0, pc
        ldrh    r1, [mem]
        lsl     r0, 16
        lsr     r0, 16  ; fills top half of r0 with zeroes
        cmp     r1, r0
        bne     f414

        add     mem, 32
        b       t415

f414:
        m_exit  414

t415:
        ; ARM 8: Store halfword PC + 4, with register offset
        mov     r2, 0
        dw      0xE00BF0B2  ; strh    pc, [mem, r0]
        mov     r0, pc
        ldrh    r1, [mem]
        lsl     r0, 16
        lsr     r0, 16  ; fills top half of r0 with zeroes
        cmp     r1, r0
        bne     f415

        add     mem, 32
        b       t416

f415:
        m_exit  415

t416:
        ; ARM 8: Load halfword immediate offset with pre-increment and writeback to PC
        dw      0xE1FF00B4  ; ldrh    r0, [pc, 4]!
        b       f416
        b       f416
        b       f416
        b       t417  ; unlike word, halfword does use +4 offset
        b       f416
        b       f416

f416:
        m_exit  416

t417:
        ; ARM 8: Same as t416, but with register offset
        mov     r4, 4
        dw      0xE1BF00B4  ; ldrh    r0, [pc], r4!
        b       f417
        b       f417
        b       f417
        b       t418  ; unlike word, halfword does use +4 offset
        b       f417
        b       f417

f417:
        m_exit  417

t418:
        ; ARM 8: Load signed half immediate with pre-increment and writeback to PC
        dw      0xE1FF00F4  ; ldrsh   r0, [pc, 4]!
        b       f418
        b       f418
        b       f418
        b       t419  ; new pc location
        b       f418
        b       f418

f418:
        m_exit  418

t419:
        ; ARM 8: Same as t418, but with post-increment
        dw      0xE0FF00F4  ; ldrsh   r0, [pc], 4!
        b       f419
        b       f419
        b       f419
        b       t420  ; new pc location
        b       f419
        b       f419

f419:
        m_exit  419

t420:
        ; ARM 8: Same as t418, but with register offset
        mov     r4, 4
        dw      0xE1BF00F4  ; ldrsh   r0, [pc, r4]!
        b       f420
        b       f420
        b       f420
        b       t421  ; new pc location
        b       f420
        b       f420

f420:
        m_exit  420

t421:
        ; ARM 8: Same as t418, but with decrement
        dw      0xE17F00F4  ; ldrsh   r0, [pc, -4]!
        b       f421
        b       t422  ; new pc location
        b       f421
        b       f421
        b       f421
        b       f421

f421:
        m_exit  421

t422:
        ; ARM 8: Store Signed halfword, immediate offset (undocumented, equivalent to STRH)
        mov     r1, 0xEE
        mvn     r0, 0
        str     r0, [mem]  ; clear section of memory
        mov     r2, 0x42
        dw      0xE1CB20F0  ; strsh   r2, [mem, 0]
        m_word  r2, 0xFFFF0042
        ldr     r1, [mem]
        cmp     r1, r2
        bne     f422

        add     mem, 32
        b       t423

f422:
        m_exit  422

t423:
        ; ARM 8: Store Signed halfword, register offset (undocumented, equivalent to STRH)
        mov     r1, 0xEE
        mvn     r0, 0
        str     r0, [mem]  ; clear section of memory
        mov     r0, 0
        mov     r2, 0x42
        dw      0xE18B20F0  ; strsh   r2, [mem, r0]
        m_word  r2, 0xFFFF0042
        ldr     r1, [mem]
        cmp     r1, r2
        bne     f423

        add     mem, 32
        b       t424

f423:
        m_exit  423

t424:
        ; ARM 8: Same as t422, but with negative value
        mov     r1, 0xEE
        mvn     r0, 0
        str     r0, [mem]  ; clear section of memory
        mvn     r2, 0x42
        dw      0xE1CB20F0  ; strsh   r2, [mem, 0]
        ldr     r1, [mem]
        cmp     r1, r2
        bne     f424

        add     mem, 32
        b       t425

f424:
        m_exit  424

t425:
        ; ARM 8: Same as t424, but with STRSB
        mov     r1, 0xEE
        mvn     r0, 0
        str     r0, [mem]  ; clear section of memory
        mvn     r2, 0x42
        dw      0xE1CB20D0  ; strsb   r2, [mem, 0]
        ldr     r1, [mem]
        cmp     r1, r2
        bne     f425

        add     mem, 32
        b       t426

f425:
        m_exit  425

t426:
        ; ARM 8: Misaligned STRSH
        mov     r0, 0
        str     r0, [mem]
        mov     r2, 0xC3
        dw      0xE1CB20F1  ; strsh   r2, [mem, 1]
        ldrh    r1, [mem]
        cmp     r1, r2
        bne     f426

        add     mem, 32
        b       halfword_transfer_passed

f426:
        m_exit  426

halfword_transfer_passed:
        restore mem

eval:
        m_vsync
        m_test_eval r12

idle:
        b       idle

include '../lib/text.asm'
