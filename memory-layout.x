/* Holy Rust — shared section layout.
 *
 * INCLUDED by memory.x (ARM) and memory-riscv.x (RISC-V), which define the
 * MEMORY regions: flash, sram, vectors, registry, sram_code.
 */

ENTRY(Reset)

_stack_top = ORIGIN(sram) + LENGTH(sram);

/* Program persistence store (Phase 7): 4K below the stack ceiling.
 * ARM only — riscv32 DTIM is fully carved. */
ASSERT(LENGTH(sram) >= 8192, "sram too small for pstore")
_pstore_top = _stack_top;
_pstore_base = _stack_top - 4K;

/* Phase 8 multi-tasking carve: 4 task stacks x 512B descending from pstore,
 * then heap_pool reservation (Phase 9 capability pools — routed nowhere yet).
 * All above .bss; stack-slack ASSERT guards the floor. */
_task_stacks_top = _pstore_base;
_task_stacks_base = _task_stacks_top - 2K;   /* 4 x 512B */
_heap_pool_base = _task_stacks_base - 8K;
ASSERT(LENGTH(sram) >= 16384, "sram too small for task stacks + heap pool")

/* Stack-slack contract (>=4K on ARM's 52K SRAM). */
ASSERT((_heap_pool_base - __ebss) >= 4096, "stack slack below 4K on arm")

TASK_STACK_SIZE = 512;
NUM_TASK_SLOTS = 4;

_task_stack_0 = _task_stacks_top - 0 * 512;
_task_stack_1 = _task_stacks_top - 1 * 512;
_task_stack_2 = _task_stacks_top - 2 * 512;
_task_stack_3 = _task_stacks_top - 3 * 512;

/* JIT slots: 4 x 1K inside the existing EXEC_BUFFER region (defined in memory.x as 4K). */
JIT_SLOT_SIZE = 1024;
_jit_slot_0 = ORIGIN(sram_code);
_jit_slot_1 = ORIGIN(sram_code) + 1 * JIT_SLOT_SIZE;
_jit_slot_2 = ORIGIN(sram_code) + 2 * JIT_SLOT_SIZE;
_jit_slot_3 = ORIGIN(sram_code) + 3 * JIT_SLOT_SIZE;

SECTIONS
{
    /* Hardware vector table. First two words are emitted here directly by
     * the linker: initial stack pointer and Reset entry. NOTE: ELF function
     * symbols for Thumb code already carry the Thumb bit in their value,
     * so plain LONG(Reset) yields an odd (interworking-correct) address;
     * adding +1 here would clear the bit and boot the core in ARM state.
     *
     * The remaining core exceptions (slots 2..15) route to fault_hang so a
     * wild access degrades to a visible UART stop instead of lockup; zero
     * marks architecturally-reserved slots. KEEP(*(.isr_vector)) still
     * follows, letting Rust statics append device IRQ entries. */
    .isr_vector : ALIGN(4)
    {
        __vector_start = .;
        LONG(_stack_top)
        LONG(Reset)
        LONG(fault_hang)        /*  1 NMI */
        LONG(fault_hang)        /*  2 HardFault */
        LONG(fault_hang)        /*  3 MemManage */
        LONG(fault_hang)        /*  4 BusFault */
        LONG(fault_hang)        /*  5 UsageFault */
        LONG(0)                 /*  6 reserved */
        LONG(0)                 /*  7 reserved */
        LONG(0)                 /*  8 reserved */
        LONG(0)                 /*  9 reserved */
        LONG(fault_hang)        /* 10 SVCall */
        LONG(fault_hang)        /* 11 DebugMon */
        LONG(0)                 /* 12 reserved */
        LONG(fault_hang)        /* 13 PendSV */
        LONG(fault_hang)        /* 14 SysTick */
        KEEP(*(.isr_vector))
        __vector_end = .;
    } > flash

    .text : ALIGN(4)
    {
        *(.text .text.*)
        *(.rodata .rodata.*)
        *(.srodata .srodata.*)
        . = ALIGN(4);
        __etext = .;
    } > flash

    /* Relocatable SRAM vector table (VTOR target on ARM). */
    .sram_vectors (NOLOAD) : ALIGN(4)
    {
        KEEP(*(.sram_vectors))
    } > vectors

    /* O(1) capability bitfield registry. */
    .capability_registry (NOLOAD) : ALIGN(4)
    {
        KEEP(*(.capability_registry))
    } > registry

    /* JIT execution buffer (writable + executable). */
    .sram_code (NOLOAD) : ALIGN(4)
    {
        KEEP(*(.sram_code))
    } > sram_code

    .data : ALIGN(4)
    {
        __sdata = .;
        *(.data .data.*)
        *(.sdata .sdata.*)
        . = ALIGN(4);
        __edata = .;
    } > sram AT > flash
    __sidata = LOADADDR(.data);

    .bss (NOLOAD) : ALIGN(4)
    {
        __sbss = .;
        *(.bss .bss.*)
        *(.sbss .sbss.*)
        *(COMMON)
        . = ALIGN(4);
        __ebss = .;
    } > sram

    /* RISC-V small-data relaxation anchor (initialized in startup). */
    __global_pointer$ = __sdata + 0x800;

    /DISCARD/ :
    {
        *(.eh_frame*)
        *(.comment*)
    }
}
