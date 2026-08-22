/* HR-OS — SASA Consolidated Physical Memory Map (SYNTHESIS.md, AXIS-2.md)
 *
 * Single Address Space Architecture: VA ≡ PA. No MMU, no page tables.
 * This file is a consolidated *documentation* view of the physical map.
 * The build uses memory.x / memory-riscv.x + memory-layout*.x (SASA carved).
 *
 * Address space is 64-bit, but QEMU STM32F405/SiFive models use the
 * low 32-bit window shown. All regions are identity-mapped.
 *
 *   0x0000_0000 ───────────────────────────────────────────────
 *   0x0800_0000 flash (RX) 128K — ARM vector + .text/.rodata (also aliased 0x0000_0000)
 *   0x2000_0400 vectors (RW) 3K   — RAM_VECTOR_TABLE align(1024) → VTOR 0xE000ED08
 *   0x2000_1000 registry (RW) 256B — REGISTRY_BITS AtomicU32[8] (256 caps)
 *   0x2000_2000 sram_code (RWX) 4K — EXEC_BUFFER (ITIM 0x08000000 on SiFive)
 *   0x2000_3000 sram (RW) 52K  — .data/.bss + stack descends from _stack_top
 *   0x2001_0000 sram top (52K + 0x3000)
 *   0x4000_0000 MMIO peripherals — GPIOA 0x40020000, GPIOB 0x40020400, UART0 0x40011000, etc.
 *   0xE000_0000 System Control (SCS) — VTOR 0xE000ED08, DWT 0xE0001000 (deny-listed)
 *   0xFFFF_FFFF end
 *   SiFive variant:
 *   0x2040_0000 flash (RX) 16M — code linked at QEMU ROM jump
 *   0x0800_0000 ITIM (RWX) 4K   — EXEC_BUFFER on SiFive
 *   0x8000_0000 DTIM (RW) 8K    — sram + vectors + registry
 *
 * SASA INVARIANTS (linker-enforced):
 *   _stack_top = ORIGIN(sram) + LENGTH(sram)
 *   ASSERT( (ORIGIN(vectors) & 0x3FF) == 0, "VTOR must be 1024B aligned" )
 *   ASSERT( (ORIGIN(registry) & 0x3) == 0, "registry 4B aligned" )
 *   ASSERT( (ORIGIN(sram_code) & 0x3) == 0, "EXEC_BUFFER 4B aligned" )
 */

MEMORY
{
    flash      (rx)  : ORIGIN = 0x08000000, LENGTH = 128K
    vectors    (rw)  : ORIGIN = 0x20000400, LENGTH = 3K
    registry   (rw)  : ORIGIN = 0x20001000, LENGTH = 256
    sram_code  (rwx) : ORIGIN = 0x20002000, LENGTH = 4K
    sram       (rwx) : ORIGIN = 0x20003000, LENGTH = 52K
    mmio       (rw)  : ORIGIN = 0x40000000, LENGTH = 512M
    scs        (rw)  : ORIGIN = 0xE0000000, LENGTH = 1M
}

REGION_ALIAS("FLASH", flash)
REGION_ALIAS("RAM_VECTORS", vectors)
REGION_ALIAS("CAP_REGISTRY", registry)
REGION_ALIAS("EXEC_BUFFER", sram_code)
REGION_ALIAS("RAM", sram)

/* SiFive aliases (when building with memory-riscv.x, these override):
 *   flash → 0x20400000, sram_code → 0x08000000, sram → 0x80000000
 *   vectors → 0x80000400, registry → 0x80001000
 */

_stack_top = ORIGIN(sram) + LENGTH(sram);
ASSERT((_stack_top & 0x7) == 0, "stack top must be 8B aligned");
ASSERT((ORIGIN(vectors) & 0x3FF) == 0, "VTOR 1024B alignment");
ASSERT((ORIGIN(registry) & 0x3) == 0, "registry alignment");
ASSERT((ORIGIN(sram_code) & 0x3) == 0, "EXEC_BUFFER alignment");

/* Include real layout — this file is documentation; actual SECTIONS come from memory-layout*.x */
INCLUDE memory-layout.x
