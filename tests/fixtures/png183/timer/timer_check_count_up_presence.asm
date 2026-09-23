format binary as 'gba'

include '../lib/constants.inc'
include '../lib/macros.inc'

macro m_exit test {
        mov     r12, test
        b       eval
}

header:
        include '../lib/header.asm'

main:
        m_test_init

        ; Reset test register
        mov     r12, 0
        
        ; Use r3 to access IO
        mov     r3, MEM_IO
        
        ; Use r4 as a zero register
        mov     r4, 0
        
        ; Use r5 for expected value of first 3 tests
        m_word  r5, 0x00040000

t001:
        mov     r0, 0
        m_word  r1, 0x003C0000
        str     r1, [r3, REG_TIM3CNT]
        
        ; Check count-up bit is set
        ldr     r0, [r3, REG_TIM3CNT]
        and     r0, r1
        
        cmp     r0, r5
        bne     f001
        b       t002

f001:
        m_exit  1

t002:
        mov     r0, 0
        m_word  r1, 0x003C0000
        str     r1, [r3, REG_TIM2CNT]
        
        ; Check count-up bit is set
        ldr     r0, [r3, REG_TIM2CNT]
        and     r0, r1
        
        cmp     r0, r5
        bne     f002
        b       t003

f002:
        m_exit  2

t003:
        mov     r0, 0
        m_word  r1, 0x003C0000
        str     r1, [r3, REG_TIM1CNT]
        
        ; Check count-up bit is set
        ldr     r0, [r3, REG_TIM1CNT]
        and     r0, r1
        
        cmp     r0, r5
        bne     f003
        b       t004

f003:
        m_exit  3

t004:
        mov     r0, 0
        m_word  r1, 0x003C0000
        str     r1, [r3, REG_TIM0CNT]
        
        ; Check count-up bit is clear
        ldr     r0, [r3, REG_TIM0CNT]
        and     r0, r1
        
        cmp     r0, r4
        bne     f004
        b       eval

f004:
        m_exit  4

eval:
        m_vsync
        m_test_eval r12

idle:
        b       idle

include '../lib/text.asm'
