#ifndef KB_SETJMP_H
#define KB_SETJMP_H

#include <stdint.h>

typedef uint32_t jmp_buf[26] __attribute__((aligned(8)));

int kb_setjmp(jmp_buf) __attribute__((returns_twice));
void kb_longjmp(jmp_buf, int) __attribute__((noreturn));

#define setjmp kb_setjmp
#define longjmp kb_longjmp

#endif
