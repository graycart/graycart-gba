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

t001:
        ; test value of DMA enable flag after immediate mode repeating transfer
        mov     r4, MEM_IO
        mov     r3, MEM_IWRAM
        str     r3, [r4, REG_DMA0SAD]
        str     r3, [r4, REG_DMA0DAD]
        str     r3, [r4, REG_DMA0CNT]
        m_word  r0, 0x83400001  ; fixed src/dst, len 1, repeating, immediate mode
        str     r0, [r4, REG_DMA0CNT]
        ; give the DMA some time to finish
        nop
        nop
        nop
        nop
        nop
        ; ensure that the DMA enable bit was cleared
        ldr     r1, [r4, REG_DMA0CNT]
        m_word  r2, 0x03400000  ; enable bit should be cleared even though repeat mode was enabled
        cmp     r1, r2
        bne     f001
        b       t002

f001:
        m_exit  001

t002:
        ; same as test 1, but use HBLANK mode
        mov     r4, MEM_IO
        mov     r3, MEM_IWRAM
        str     r3, [r4, REG_DMA0SAD]
        str     r3, [r4, REG_DMA0DAD]
        str     r3, [r4, REG_DMA0CNT]
        m_word  r0, 0xa3400001  ; fixed src/dst, len 1, repeating, HBLANK mode
        str     r0, [r4, REG_DMA0CNT]
        ; give the DMA several opportunities to run
dma_wait_loop_1:
        ldrh    r5, [r4, 0x06]
        cmp     r5, 160
        bne     dma_wait_loop_1
dma_wait_loop_2:
        ldrh    r5, [r4, 0x06]
        cmp     r5, 159
        bne     dma_wait_loop_2
        ; ensure that the DMA enable bit was not cleared
        ldr     r1, [r4, REG_DMA0CNT]
        m_word  r2, 0xa3400000  ; enable bit should be set
        cmp     r1, r2
        bne     f002
        b       dma_repeat_immediate_passed

f002:
        m_exit  002

dma_repeat_immediate_passed:
eval:
        m_vsync
        m_test_eval r12

idle:
        b       idle

include '../lib/text.asm'
