#include "kb_guest.h"
#include <setjmp.h>

#define KB_TABLE_SIZE 128

static uint8_t memory_bytes[WASM_DEFAULT_PAGE_SIZE] __attribute__((aligned(8)));
static wasm_rt_funcref_t table_entries[KB_TABLE_SIZE];
static w2c_kb__guest instance;
static jmp_buf recovery;
uint32_t wasm_rt_call_stack_depth;

bool kb_stack_available(void)
{
#if defined(__arm__)
    extern const uint32_t _stack_end;
    uintptr_t stack;
    __asm__ volatile ("mov %0, sp" : "=r"(stack));
    return stack > (uintptr_t)&_stack_end + 3072;
#else
    return true;
#endif
}

bool wasm_rt_is_initialized(void)
{
    return true;
}

void wasm_rt_trap(wasm_rt_trap_t code)
{
    wasm_rt_call_stack_depth = 0;
    longjmp(recovery, (int)code);
}

void abort(void)
{
    wasm_rt_trap(WASM_RT_TRAP_UNREACHABLE);
}

void wasm_rt_allocate_memory(wasm_rt_memory_t *memory, uint64_t initial,
                             uint64_t maximum, bool is64, uint32_t page_size)
{
    if (initial != 1 || maximum == 0 || is64 || page_size != sizeof(memory_bytes)) {
        wasm_rt_trap(WASM_RT_TRAP_OOB);
    }
    memset(memory_bytes, 0, sizeof(memory_bytes));
    *memory = (wasm_rt_memory_t){
        .data = memory_bytes,
        .data_end = memory_bytes + sizeof(memory_bytes),
        .page_size = sizeof(memory_bytes),
        .pages = 1,
        .max_pages = 1,
        .size = sizeof(memory_bytes),
        .is64 = false,
    };
}

uint64_t wasm_rt_grow_memory(wasm_rt_memory_t *memory, uint64_t pages)
{
    if (pages != 0) {
        wasm_rt_trap(WASM_RT_TRAP_OOB);
    }
    return memory->pages;
}

void wasm_rt_free_memory(wasm_rt_memory_t *memory)
{
    *memory = (wasm_rt_memory_t){0};
}

void wasm_rt_allocate_funcref_table(wasm_rt_funcref_table_t *table,
                                   uint32_t elements, uint32_t maximum)
{
    if (elements > KB_TABLE_SIZE || maximum < elements) {
        wasm_rt_trap(WASM_RT_TRAP_OOB);
    }
    memset(table_entries, 0, sizeof(table_entries));
    *table = (wasm_rt_funcref_table_t){
        .data = table_entries,
        .max_size = elements,
        .size = elements,
    };
}

void wasm_rt_free_funcref_table(wasm_rt_funcref_table_t *table)
{
    *table = (wasm_rt_funcref_table_t){0};
}

uint32_t kb_embedded_begin(uint32_t fuel)
{
    int trap = setjmp(recovery);
    if (trap != 0) {
        return (uint32_t)trap;
    }
    wasm2c_kb__guest_instantiate(&instance);
    *w2c_kb__guest_kb_meter(&instance) = fuel;
    return 0;
}

void kb_embedded_fuel(uint32_t fuel)
{
    *w2c_kb__guest_kb_meter(&instance) = fuel;
}

uint8_t *kb_embedded_memory(void)
{
    return memory_bytes;
}

enum {
    Realloc, BrailleMap, BrailleStep, DisplayDraw, DisplayNavigate, KeyboardProcess,
    KeyboardTransform, PostBrailleMap, PostBrailleStep, PostDisplayDraw,
    PostDisplayNavigate, PostKeyboardProcess, PostKeyboardTransform,
};

uint32_t kb_embedded_call(uint32_t operation, const uint32_t *arguments, uint32_t *output)
{
    int trap = setjmp(recovery);
    if (trap != 0) {
        return (uint32_t)trap;
    }

    switch (operation) {
    case Realloc:
        *output = w2c_kb__guest_cabi_realloc(&instance, arguments[0], arguments[1], arguments[2], arguments[3]);
        break;
    case BrailleMap:
        *output = w2c_kb__guest_kb0x3Afirmware0x2Fbraille0x4000x2E10x2E00x23map0x2Dcell(&instance, arguments[0]);
        break;
    case BrailleStep:
        *output = w2c_kb__guest_kb0x3Afirmware0x2Fbraille0x4000x2E10x2E00x23step(&instance, arguments[0], arguments[1], arguments[2]);
        break;
    case DisplayDraw:
        *output = w2c_kb__guest_kb0x3Afirmware0x2Fdisplay0x4000x2E10x2E00x23render(&instance, arguments[0], arguments[1], arguments[2], arguments[3], arguments[4], arguments[5]);
        break;
    case DisplayNavigate:
        *output = w2c_kb__guest_kb0x3Afirmware0x2Fdisplay0x4000x2E10x2E00x23navigate(&instance, arguments[0], arguments[1], arguments[2], arguments[3]);
        break;
#ifdef KB_EMBEDDED_KEYBOARD
    case KeyboardProcess:
        *output = w2c_kb__guest_kb0x3Afirmware0x2Fkeyboard0x4000x2E10x2E00x23process(&instance, arguments[0], arguments[1], arguments[2], arguments[3], arguments[4]);
        break;
    case KeyboardTransform:
        *output = w2c_kb__guest_kb0x3Afirmware0x2Fkeyboard0x4000x2E10x2E00x23transform0x2Doutput(&instance, arguments[0], arguments[1], arguments[2], arguments[3]);
        break;
    case PostKeyboardProcess:
        w2c_kb__guest_cabi_post_kb0x3Afirmware0x2Fkeyboard0x4000x2E10x2E00x23process(&instance, arguments[0]);
        break;
    case PostKeyboardTransform:
        w2c_kb__guest_cabi_post_kb0x3Afirmware0x2Fkeyboard0x4000x2E10x2E00x23transform0x2Doutput(&instance, arguments[0]);
        break;
#endif
    case PostBrailleMap:
        w2c_kb__guest_cabi_post_kb0x3Afirmware0x2Fbraille0x4000x2E10x2E00x23map0x2Dcell(&instance, arguments[0]);
        break;
    case PostBrailleStep:
        w2c_kb__guest_cabi_post_kb0x3Afirmware0x2Fbraille0x4000x2E10x2E00x23step(&instance, arguments[0]);
        break;
    case PostDisplayDraw:
        w2c_kb__guest_cabi_post_kb0x3Afirmware0x2Fdisplay0x4000x2E10x2E00x23render(&instance, arguments[0]);
        break;
    case PostDisplayNavigate:
        w2c_kb__guest_cabi_post_kb0x3Afirmware0x2Fdisplay0x4000x2E10x2E00x23navigate(&instance, arguments[0]);
        break;
    default:
        return WASM_RT_TRAP_UNREACHABLE;
    }
    return 0;
}
