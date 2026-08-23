/* Holy Rust — shared section layout, RISC-V variant.
 *
 * INCLUDED by memory-riscv.x, which defines the MEMORY regions: flash,
 * sram, vectors, registry, sram_code.
 *
 * No .isr_vector section: RISC-V has no hardware vector table at the flash
 * base, and QEMU's sifive_e boot ROM jumps directly to ORIGIN(flash), so
 * the Reset entry code must be the first thing there.
 */

ENTRY(Reset)

_stack_top = ORIGIN(sram) + LENGTH(sram);

/* Stack-slack contract: .bss end to stack top must leave >=1.5K for REPL,
 * JIT compiler frames and the panic printer. Ring 0 has no page-fault net:
 * overflow = hard fault, so enforce headroom at link time. */
ASSERT((_stack_top - __ebss) >= 1536, "stack slack below 1.5K on riscv32 DTIM")

SECTIONS
{
    .text : ALIGN(4)
    {
        *(.text .text.*)
        *(.rodata .rodata.*)
        *(.srodata .srodata.*)
        . = ALIGN(4);
        __etext = .;
    } > flash

    /* Relocatable trap/handler slot array (mtvec target once vectored
     * dispatch is configured). */
    .sram_vectors (NOLOAD) : ALIGN(4)
    {
        KEEP(*(.sram_vectors))
    } > vectors

    /* O(1) capability bitfield registry. */
    .capability_registry (NOLOAD) : ALIGN(4)
    {
        KEEP(*(.capability_registry))
    } > registry

    /* JIT execution buffer (writable + executable, ITIM).
     * NOLOAD keeps the section out of the ELF file; build.rs runs
     * objcopy to grant the PT_LOAD segment covering this address range
     * execute permission (LLD infers RW from the input sections). */
    .sram_code (NOLOAD) : ALIGN(4)
    {
        KEEP(*(.sram_code))
    } > sram_code

    /* Dedicated always-safe scratch (tests, diagnostics). Not zeroed by
     * init_data_bss; contents undefined until first write. */
    .scratch (NOLOAD) : ALIGN(4)
    {
        KEEP(*(.scratch))
    } > scratch

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

    /* Small-data relaxation anchor (initialized in startup). */
    __global_pointer$ = __sdata + 0x800;

    /DISCARD/ :
    {
        *(.eh_frame*)
        *(.comment*)
    }
}
