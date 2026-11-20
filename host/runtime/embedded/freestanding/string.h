#ifndef KB_STRING_H
#define KB_STRING_H

#include <stddef.h>

void *memmove(void *dest, const void *src, size_t count);
void *memcpy(void *dest, const void *src, size_t count);
void *memset(void *dest, int value, size_t count);
int memcmp(const void *lhs, const void *rhs, size_t count);

#endif
