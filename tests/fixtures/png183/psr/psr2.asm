format binary as 'gba'

include '../lib/constants.inc'
include '../lib/macros.inc'

header:
        include '../lib/header.asm'

main:
        m_test_init
        ; Reset test register
        mov     r12, 0

        ; turn off sound
        mov     r3, MEM_IO
        mov     r0, 0
        str     r0, [r3, REG_SNDCNT]
        str     r0, [r3, REG_SNDCNTX]
        str     r0, [r3, REG_IE]
        str     r0, [r3, REG_IME]
        mov     r1, MEM_EWRAM

        ; enter SYS mode
        mov     r2, 0x1F
        msr     cpsr_c, r2

t001:
        ; test reading/writing extension field (should always be zero)
        mov     r3, 0xff
        lsl     r3, 8
        mov     r2, 0xff
        lsl     r2, 8
        msr     cpsr_x, r2
        mrs     r1, cpsr
        and     r1, r3
        cmp     r1, r2
        beq     f001
t002:
        cmp     r1, 0
        bne     f002

t003:
        ; test reading/writing status field (should always be zero)
        lsl     r2, 8
        lsl     r3, 8
        msr     cpsr_s, r2
        mrs     r1, cpsr
        and     r1, r3
        cmp     r1, r2
        beq     f003
t004:
        cmp     r1, 0
        bne     f004

t005:
        ; test reading/writing undocumented CPSR flag bits (should always be zero)
        mov     r0, 0
        msr     cpsr_f, r0  ; clear flags before test
        lsl     r2, 8
        lsl     r3, 8
        msr     cpsr_f, r2
        mrs     r1, cpsr
        and     r1, r3
        cmp     r1, r2
        beq     f005
t006:
        lsr     r1, 24
        cmp     r1, 0xf0  ; upper 4 bits are written, lower 4 are discarded
        bne     f006

t007:
        ; test that bit 4 of CPSR mode is forced to 1,
        ; even if that mode value is nonsense
        mov     r2, 0x0c
        msr     cpsr_c, r2
        mrs     r1, cpsr
        and     r1, 0xFF
        cmp     r1, 0x1c
        bne     f007

        ; enter ABT mode
        mov     r2, 0x17
        msr     cpsr_c, r2

t008:
        ; test whether bit 4 of SPSR mode is forced to 1
        mov     r2, 0x03
        dw      0xE161F002  ; msr spsr_c, r2
        mrs     r1, spsr
        and     r1, 0xFF
        cmp     r1, 0x13
        bne     f008

        ; re-enter SYS mode
        mov     r2, 0x1F
        msr     cpsr_c, r2

t009:
        ; test MSR shift field
        mov     r0, 0
        msr     cpsr_f, r0  ; clear flags before test
        mov     r3, 0xf0
        lsl     r3, 24
        mov     r2, 1
        dw      0xE128FF82  ; msr cpsr_f, r2 lsl #31
        mrs     r1, cpsr
        mov     r0, 1
        lsl     r0, 31
        and     r0, r3
        and     r1, r3
        cmp     r1, r0
        bne     f009

t010:
        ; test MSR shift type field
        mov     r0, 0
        msr     cpsr_f, r0  ; clear flags before test
        mov     r3, 0xf0
        lsl     r3, 24
        mov     r2, 1
        dw      0xE128F162  ; msr cpsr_f, r2 ror 2
        mrs     r1, cpsr
        mov     r0, 1
        lsl     r0, 30
        and     r0, r3
        and     r1, r3
        cmp     r1, r0
        bne     f010

t011:
        ; test MSR with RRX
        mov     r0, 0
        msr     cpsr_f, r0  ; clear flags before test
        mov     r3, 0xf0
        lsl     r3, 24
        mov     r2, 0xf
        lsl     r2, 28
        dw      0xE128F062  ; msr cpsr_f, r2 rrx
        mrs     r1, cpsr
        mov     r0, 0x7
        lsl     r0, 28
        and     r0, r3
        and     r1, r3
        cmp     r1, r0
        bne     f011

t012:
        ; test reading/writing SPSR in invalid modes (should always contain 0x00000010)
        mov     r2, 0x0c
        msr     cpsr_c, r2
        dw      0xE161F002  ; msr spsr_c, r2
        lsl     r2, 28
        dw      0xE168F002  ; msr spsr_f, r2
        mrs     r1, spsr
        cmp     r1, 0x10
        bne     f012

t013:
        ; test writing R13 in invalid modes (should be read-only)
        mov     r0, 0
        mov     r1, r13
        mov     r13, 0xff
        mov     r2, r13
        mov     r13, r1
        cmp     r0, r2
        bne     f013

t014:
        ; test writing R12 in invalid modes (should be writable)
        mov     r0, 0
        mov     r1, r12
        mov     r12, 0xff
        mov     r2, r12
        mov     r12, r1
        cmp     r0, r2
        beq     f014

        ; re-enter SYS mode
        mov     r2, 0x1F
        msr     cpsr_c, r2

t015:
        ; test MRS with bits 7-5 set (should have no effect)
        mov     r0, 0
        mov     r1, 0xf0
        lsl     r1, 24
        msr     cpsr_f, r1
        dw      0xE10F00E0  ; mrs     r0, cpsr
        and     r0, r1
        cmp     r0, r1
        bne     f015

t016:
        ; MSR imm with bit 21 clear (should read from GPR)
        mov     r0, 0
        mov     r1, pc
        dw      0xE30F0000  ; msr r0, r15
        add     r1, 4
        cmp     r0, r1
        bne     f016

t017:
        ; MSR GPR-to-GPR with different Rn
        mov     r0, 0
        mov     r1, 42
        dw      0xE3010000  ; mov r0, r1
        cmp     r0, 42
        bne     f017

t018:
        ; MSR GPR-to-GPR with different Rd
        mov     r0, 42
        mov     r1, 0
        dw      0xE3001000  ; "mov r1, r0"?
        cmp     r1, 42
        bne     f018

t019:
        ; MSR GPR-to-GPR with different Rm (unused)
        mov     r0, 0xFE
        mov     r1, 0
        mov     r2, 42
        dw      0xE3001002  ; "mov r1, r0"?
        cmp     r1, 0xFE
        bne     f019

t020:
        ; MSR GPR-to-GPR all fields test
        mov     r0, 0xFE
        mov     r1, 1
        mov     r2, 42
        mov     r3, 0
        dw      0xE34031F2  ; "mov r3, r0"?
        cmp     r3, 0xFE
        bne     f020
        cmp     r2, 42
        bne     f020
        cmp     r1, 1
        bne     f020
        cmp     r0, 0xFE
        bne     f020

        bl      eval

f001:
        mov     r1, 0x1F
        msr     cpsr_c, r1
        mov     r12, 1
        bl      eval

f002:
        mov     r1, 0x1F
        msr     cpsr_c, r1
        mov     r12, 2
        bl      eval

f003:
        mov     r1, 0x1F
        msr     cpsr_c, r1
        mov     r12, 3
        bl      eval

f004:
        mov     r1, 0x1F
        msr     cpsr_c, r1
        mov     r12, 4
        bl      eval

f005:
        mov     r1, 0x1F
        msr     cpsr_c, r1
        mov     r12, 5
        bl      eval

f006:
        mov     r1, 0x1F
        msr     cpsr_c, r1
        mov     r12, 6
        bl      eval

f007:
        mov     r1, 0x1F
        msr     cpsr_c, r1
        mov     r12, 7
        bl      eval

f008:
        mov     r1, 0x1F
        msr     cpsr_c, r1
        mov     r12, 8
        bl      eval

f009:
        mov     r1, 0x1F
        msr     cpsr_c, r1
        mov     r12, 9
        bl      eval

f010:
        mov     r1, 0x1F
        msr     cpsr_c, r1
        mov     r12, 10
        bl      eval

f011:
        mov     r1, 0x1F
        msr     cpsr_c, r1
        mov     r12, 11
        bl      eval

f012:
        mov     r1, 0x1F
        msr     cpsr_c, r1
        mov     r12, 12
        bl      eval

f013:
        mov     r1, 0x1F
        msr     cpsr_c, r1
        mov     r12, 13
        bl      eval

f014:
        mov     r1, 0x1F
        msr     cpsr_c, r1
        mov     r12, 14
        bl      eval

f015:
        mov     r1, 0x1F
        msr     cpsr_c, r1
        mov     r12, 15
        bl      eval

f016:
        mov     r1, 0x1F
        msr     cpsr_c, r1
        mov     r12, 16
        bl      eval

f017:
        mov     r1, 0x1F
        msr     cpsr_c, r1
        mov     r12, 17
        bl      eval

f018:
        mov     r1, 0x1F
        msr     cpsr_c, r1
        mov     r12, 18
        bl      eval

f019:
        mov     r1, 0x1F
        msr     cpsr_c, r1
        mov     r12, 19
        bl      eval

f020:
        mov     r1, 0x1F
        msr     cpsr_c, r1
        mov     r12, 20
        bl      eval

eval:
        m_vsync
        m_test_eval r12

idle:
        b       idle

include '../lib/text.asm'

main_end:
