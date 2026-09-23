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

multiply:
        ; Tests for multiply operations

t300:
        ; ARM 5: Multiply
        mov     r0, 4
        mov     r1, 8
        mul     r0, r1, r0
        cmp     r0, 32
        bne     f300

        b       t301

f300:
        m_exit  300

t301:
        mov     r0, -4
        mov     r1, -8
        mul     r0, r1, r0
        cmp     r0, 32
        bne     f301

        b       t302

f301:
        m_exit  301

t302:
        mov     r0, 4
        mov     r1, -8
        mul     r0, r1, r0
        cmp     r0, -32
        bne     f302

        b       t303

f302:
        m_exit  302

t303:
        ; ARM 5: Multiply accumulate
        mov     r0, 4
        mov     r1, 8
        mov     r2, 8
        mla     r0, r1, r0, r2
        cmp     r0, 40
        bne     t303f

        b       t304

t303f:
        m_exit  303

t304:
        mov     r0, 4
        mov     r1, 8
        mov     r2, -8
        mla     r0, r1, r0, r2
        cmp     r0, 24
        bne     f304

        b       t305

f304:
        m_exit  304

t305:
        ; ARM 6: Unsigned multiply long
        mov     r0, 4
        mov     r1, 8
        umull   r2, r3, r0, r1
        cmp     r2, 32
        bne     f305
        cmp     r3, 0
        bne     f305

        b       t306

f305:
        m_exit  305

t306:
        mov     r0, -1
        mov     r1, -1
        umull   r2, r3, r0, r1
        cmp     r2, 1
        bne     f306
        cmp     r3, -2
        bne     f306

        b       t307

f306:
        m_exit  306

t307:
        mov     r0, 2
        mov     r1, -1
        umull   r2, r3, r0, r1
        cmp     r2, -2
        bne     f307
        cmp     r3, 1
        bne     f307

        b       t308

f307:
        m_exit  307

t308:
        ; ARM 6: Unsigned multiply long accumulate
        mov     r0, 4
        mov     r1, 8
        mov     r2, 8
        mov     r3, 4
        umlal   r2, r3, r0, r1
        cmp     r2, 40
        bne     f308
        cmp     r3, 4
        bne     f308

        b       t309

f308:
        m_exit  308

t309:
        mov     r0, -1
        mov     r1, -1
        mov     r2, -2
        mov     r3, 1
        umlal   r2, r3, r0, r1
        cmp     r2, -1
        bne     f309
        cmp     r3, -1
        bne     f309


        b       t310

f309:
        m_exit  309

t310:
        ; ARM 6: Signed multiply long
        mov     r0, 4
        mov     r1, 8
        smull   r2, r3, r0, r1
        cmp     r2, 32
        bne     f310
        cmp     r3, 0
        bne     f310

        b       t311

f310:
        m_exit  310

t311:
        mov     r0, -4
        mov     r1, -8
        smull   r2, r3, r0, r1
        cmp     r2, 32
        bne     f311
        cmp     r3, 0
        bne     f311

        b       t312

f311:
        m_exit  311

t312:
        mov     r0, 4
        mov     r1, -8
        smull   r2, r3, r0, r1
        cmp     r2, -32
        bne     f312
        cmp     r3, -1
        bne     f312

        b       t313

f312:
        m_exit  312

t313:
        ; ARM 6: Signed multiply long accumulate
        mov     r0, 4
        mov     r1, 8
        mov     r2, 8
        mov     r3, 4
        smlal   r2, r3, r0, r1
        cmp     r2, 40
        bne     f313
        cmp     r3, 4
        bne     f313

        b       t314

f313:
        m_exit  313

t314:
        mov     r0, 4
        mov     r1, -8
        mov     r2, 32
        mov     r3, 0
        smlal   r2, r3, r0, r1
        cmp     r2, 0
        bne     f314
        cmp     r3, 0
        bne     f314

        b       t315

f314:
        m_exit  314

t315:
        ; ARM 6: Negative flag
        mov     r0, 2
        mov     r1, 1
        umulls  r2, r3, r0, r1
        bmi     f315

        mov     r0, 2
        mov     r1, -1
        smulls  r2, r3, r0, r1
        bpl     f315

        b       t316

f315:
        m_exit  315

t316:
        ; ARM 5: Not affecting carry and overflow
        msr     cpsr_f, 0
        mov     r0, 1
        mov     r1, 1
        mul     r0, r1, r0
        bcs     f316
        bvs     f316

        b       t317

f316:
        m_exit  316

t317:
        msr     cpsr_f, FLAG_C or FLAG_V
        mov     r0, 1
        mov     r1, 1
        mul     r0, r1, r0
        bcc     f317
        bvc     f317

        b       t318

f317:
        m_exit  317

t318:
        ; ARM 6: Not affecting carry and overflow
        msr     cpsr_f, 0
        mov     r0, 1
        mov     r1, 1
        umull   r2, r3, r0, r1
        bcs     f318
        bvs     f318

        b       t319

f318:
        m_exit  318

t319:
        msr     cpsr_f, FLAG_C or FLAG_V
        mov     r0, 1
        mov     r1, 1
        umull   r2, r3, r0, r1
        bcc     f319
        bvc     f319

        b       t320

f319:
        m_exit  319

t320:
        ; ARM 6: multiply long when rl == rh
        mov     r0, 251
        mul     r0, r0, r0
        mul     r0, r0, r0  ; r0 = 0xEC940E71 (just needs to be big enough to multiply into two registers)
        umull   r2, r3, r0, r0  ; store upper word in r3 and lower in r2 (result = 0xdaa150410b788de1)
        dw      0xE0811090  ; umull   r1, r1, r0, r0  ; perform same multiplication, but with rl == rh
        cmp     r1, r2  ; check that lower word was overwritten
        beq     f320
        cmp     r1, r3  ; check that upper word was written
        bne     f321

        b       t322

f320:
        m_exit  320
f321:
        m_exit  321

t322:
        ; ARM 5: multiply accumulate with PC as accumulator
        mov     r3, 251
        mul     r3, r3, r3
        mul     r3, r3, r3  ; make sure the multiplication is spread over several cycles
        mov     r0, pc
        dw      0xE021F393  ; mla     r1, r3, r3, pc
        add     r0, 4
        mla     r0, r3, r3, r0
        cmp     r0, r1  ; check that PC was not offset
        bne     f322

        b       t323

f322:
        m_exit  322

t323:
        ; ARM 5: multiply accumulate with PC as multiplicand
        mov     r3, 251
        mul     r3, r3, r3
        mul     r3, r3, r3  ; make sure the multiplication is spread over several cycles
        mov     r0, pc
        dw      0xE021339F  ; mla     r1, pc, r3, r3
        add     r0, 4
        mla     r0, r0, r3, r3
        cmp     r0, r1  ; check that PC was not offset
        bne     f323

        b       t324

f323:
        m_exit  323

t324:
        ; ARM 5: multiply accumulate with PC as multiplier
        mov     r3, 251
        mul     r3, r3, r3
        mul     r3, r3, r3  ; make sure the multiplication is spread over several cycles
        mov     r0, pc
        dw      0xE0213F93  ; mla     r1, r3, pc, r3
        add     r0, 4
        mla     r0, r3, r0, r3
        cmp     r0, r1  ; check that PC was not offset
        bne     f324

        b       t325

f324:
        m_exit  324

t325:
        ; ARM 5: multiply with PC as multiplicand
        mov     r3, 251
        mul     r3, r3, r3
        mul     r3, r3, r3  ; make sure the multiplication is spread over several cycles
        mov     r0, pc
        dw      0xE001339F  ; mul     r1, pc, r3
        add     r0, 4
        mul     r0, r0, r3
        cmp     r0, r1  ; check that PC was not offset
        bne     f325

        b       t326

f325:
        m_exit  325

t326:
        ; ARM 5: multiply accumulate with PC as multiplier
        mov     r3, 251
        mul     r3, r3, r3
        mul     r3, r3, r3  ; make sure the multiplication is spread over several cycles
        mov     r0, pc
        dw      0xE0013F93  ; mul     r1, r3, pc
        add     r0, 4
        mul     r0, r3, r0
        cmp     r0, r1  ; check that PC was not offset
        bne     f326

        b       t327

f326:
        m_exit  326

t327:
        ; ARM 6: multiply long with PC as multiplicand
        mov     r3, 251
        mul     r3, r3, r3
        mul     r3, r3, r3  ; make sure the multiplication is spread over several cycles
        mov     r0, pc
        dw      0xE081239F  ; umull   r2, r1, pc, r3
        add     r0, 4
        umull   r3, r1, r0, r3
        cmp     r2, r3  ; check that PC was not offset
        bne     f327

        b       t328

f327:
        m_exit  327

t328:
        ; ARM 6: multiply long with PC as multiplier
        mov     r3, 251
        mul     r3, r3, r3
        mul     r3, r3, r3  ; make sure the multiplication is spread over several cycles
        mov     r0, pc
        dw      0xE0812F93  ; umull   r2, r1, r3, pc
        add     r0, 4
        umull   r3, r1, r3, r0
        cmp     r2, r3  ; check that PC was not offset
        bne     f328

        b       t329

f328:
        m_exit  328

t329:
        ; ARM 6: multiply long with PC as rl
        mov     r2, 63  ; set value to check later for if it was overwritten
        mov     r1, 1
        mov     r0, pc
        add     r0, 16
        dw      0xE082F190  ; umull   pc, r2, r0, r1
        b       t330  ; r15 cannot be written by multiply long
        b       f329
        b       f329
        b       f329
        b       f329
        b       f329

f329:
        m_exit  329

t330:
        ; ARM 6: check that rh was overwritten even if write to rl was blocked
        cmp     r2, 0
        bne     f330
        b       t331

f330:
        m_exit  330

t331:
        ; ARM 6: multiply long with PC as rh
        mov     r2, 63  ; set value to check later for if it was overwritten
        mov     r1, 0x80
        lsl     r1, 24
        mov     r0, pc
        add     r0, 20
        lsl     r0, 1
        dw      0xE08F2190  ; umull   r2, pc, r0, r1
        b       t332  ; r15 cannot be written by multiply long
        b       f331
        b       f331
        b       f331
        b       f331
        b       f331

f331:
        m_exit  331

t332:
        ; ARM 6: check that rl was overwritten even if write to rh was blocked
        cmp     r2, 0
        bne     f332
        b       t333

f332:
        m_exit  332

t333:
        ; ARM 6: multiply with PC as rd
        mov     r1, 1
        mov     r0, pc
        add     r0, 16
        dw      0xE00F0190  ; mul     pc, r0, r1
        b       t334  ; r15 cannot be written by multiply
        b       f333
        b       f333
        b       f333
        b       f333
        b       f333

f333:
        m_exit  333

t334:
        ; ARM 5: Multiply with bit 22 set
        mov     r0, -4
        mov     r1, 8
        dw      0xE0400091 ; mul     r0, r1, r0
        cmp     r0, -32
        bne     f334

        b       t335

f334:
        m_exit  334

t335:
        ; ARM 5: Multiply with and without bit 22 set (appears to have no effect, including on timings)
        mov     r4, 0x80
        lsl     r4, 16
        mov     r2, 0
        mov     r3, MEM_IO
        ; init timer
        str     r2, [r3, REG_TIM0CNT]
        str     r4, [r3, REG_TIM0CNT]
        ; MUL with bit 22 set
        mov     r0, -4
        mov     r1, -8
        dw      0xE0400091 ; mul     r0, r1, r0
        ; check timer
        ldr     r5, [r3, REG_TIM0CNT]
        str     r2, [r3, REG_TIM0CNT]
        mov     r6, r0
        ; init timer
        str     r2, [r3, REG_TIM0CNT]
        str     r4, [r3, REG_TIM0CNT]
        ; MUL with bit 22 set
        mov     r0, -4
        mov     r1, -8
        dw      0xE0400091 ; mul     r0, r1, r0
        ; check timer
        ldr     r7, [r3, REG_TIM0CNT]
        str     r2, [r3, REG_TIM0CNT]
        cmp     r0, r6  ; check values
        bne     f335
t336:
        cmp     r5, r7  ; check timings
        bne     f336

        b       multiply_passed

f335:
        m_exit  335
f336:
        m_exit  336

multiply_passed:

eval:
        m_vsync
        m_test_eval r12

idle:
        b       idle

include '../lib/text.asm'
