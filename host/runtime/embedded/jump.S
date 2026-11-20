.syntax unified
.thumb

.section .text.kb_setjmp,"ax",%progbits
.global kb_setjmp
.thumb_func
kb_setjmp:
    stmia r0!, {r4-r11}
    mov r2, sp
    str r2, [r0], #4
    str lr, [r0], #4
    vstmia r0!, {d8-d15}
    movs r0, #0
    bx lr

.section .text.kb_longjmp,"ax",%progbits
.global kb_longjmp
.thumb_func
kb_longjmp:
    ldmia r0!, {r4-r11}
    ldr r2, [r0], #4
    mov sp, r2
    ldr lr, [r0], #4
    vldmia r0!, {d8-d15}
    mov r0, r1
    bx lr
