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

data_swap:
        ; Tests for the data swap instruction
        mem     equ r11
        mov     mem, MEM_IWRAM
        add     mem, 0x3000

t450:
        ; ARM 10: Swap word
        mvn     r0, 0
        str     r0, [mem]
        swp     r1, r0, [mem]
        cmp     r1, r0
        bne     f450
        ldr     r1, [mem]
        cmp     r1, r0
        bne     f450

        add     mem, 32
        b       t451

f450:
        m_exit  450

t451:
        ; ARM 10: Swap byte
        mvn     r0, 0
        str     r0, [mem]
        swpb    r1, r0, [mem]
        cmp     r1, 0xFF
        bne     f451
        ldr     r1, [mem]
        cmp     r1, r0
        bne     f451

        add     mem, 32
        b       t452

f451:
        m_exit  451

t452:
        ; ARM 10: Misaligned swap
        mov     r0, 32
        mov     r1, 64
        str     r1, [mem]
        add     r2, mem, 1
        swp     r3, r0, [r2]
        cmp     r3, r1, ror 8
        bne     f452
        ldr     r3, [mem]
        cmp     r3, r0
        bne     f452

        add     mem, 32
        b       t453

f452:
        m_exit  452

t453:
        ; ARM 10: Same source and destination
        mov     r0, 32
        str     r0, [mem]
        mov     r0, 64
        swp     r0, r0, [mem]
        cmp     r0, 32
        bne     f453
        ldr     r0, [mem]
        cmp     r0, 64
        bne     f453

        add     mem, 32
        b       t454

f453:
        m_exit  453

t454:
        ; ARM 10: Swap PC + 4
        dw      0xE10B209F  ; swp     r2, pc, [mem]
        mov     r0, pc
        ldr     r1, [mem]
        cmp     r1, r0
        bne     f454

        add     mem, 32
        b       t455

f454:
        m_exit  454

t455:
        ; ARM 10: Swap to load value into PC
        mov     r0, pc
        add     r0, 16
        str     r0, [mem]
        dw      0xE10BF090  ; swp     r15, r0, [mem]
        b       f455
        b       f455
        b       t456  ; PC moves here after swp
        b       f455
        b       f455
        b       f455

f455:
        m_exit  455

t456:
        ; ARM 10: Swap at PC address
        mov     r2, 0
        mov     r0, 0;
        dw      0xE10F1090  ; swp     r1, r0, pc  ; since this test executes from ROM, the SWP cannot overwrite instructions despite using the address in pc
        add     r2, 1
        add     r2, 2  ; this instruction should be loaded into r1
        add     r2, 4
        and     r1, 0xff  ; isolate immediate field of the opcode loaded into r1
        cmp     r1, 2
        bne     f456

        add     mem, 32
        b       t457

f456:
        m_exit  456

t457:
        ; ARM 10: Bits 23, 21, and 20 are ignored when decoding SWP
        mov     r1, 0x42
        mov     r0, 0x11
        mov     r2, 0x7F
        strb    r2, [mem]
        dw      0xE1FB1090  ; swpb    r1, r0, [mem]
        cmp     r1, r2
        bne     f457
        b       t458

f457:
        m_exit  457

t458:
        ; ARM 10: Verify that a value was stored (ensures this is SWPB and not LDRB)
        ldrb    r2, [mem]
        cmp     r2, r0
        bne     f458

        add     mem, 32
        b       t459

f458:
        m_exit  458

t459:
        ; ARM 10: Check SWP timings (should perform read, write, then idle cycle)
        mov     r4, 0x80
        lsl     r4, 16
        mov     r2, 0
        mov     r3, MEM_IO
        add     r3, REG_TIM0CNT
        ; reset timer
        str     r2, [r3]
        ; perform SWP on TIM0CNT to start timer
        swp     r0, r4, [r3]
        ; load timer value after SWP
        ldr     r5, [r3]
        ; reset timer again
        str     r2, [r3]
        ; check timer value
        m_half  r6, 0xffff
        and     r5, r6
        cmp     r5, 0x08
        bne     f459

        b       data_swap_passed

f459:
        m_exit  459

data_swap_passed:
        restore mem

eval:
        m_vsync
        m_test_eval r12

idle:
        b       idle

include '../lib/text.asm'
