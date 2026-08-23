/* Holy Rust — RISC-V RV32IMAC memory map (SiFive E310 / QEMU sifive_e).
 *
 * NOTE: QEMU's sifive_e boot ROM unconditionally jumps to the flash
 * controller base 0x2040_0000 (the 0x2000_0000 window is the XIP alias),
 * so code is linked there. The DTIM at 0x8000_0000 is 8K in this machine
 * and is carved into non-overlapping regions:
 *
 *   0x8000_0000  sram       .data / .bss / stack (4K; stack descends
 *                           from _stack_top = 0x8000_1000)
 *   0x8000_1000  sram_code  JIT execution buffer (768B, DTIM)
 *   0x8000_1300  scratch    dedicated poke/peek-safe test scratch (256B)
 *   0x8000_1400  vectors    trap/handler slots
 *   0x8000_1800  registry   O(1) capability bitfield
 */

MEMORY
{
    flash (rx)     : ORIGIN = 0x20400000, LENGTH = 512K
    sram (rwx)     : ORIGIN = 0x80000000, LENGTH = 4K
    sram_code (rwx): ORIGIN = 0x80001000, LENGTH = 768
    scratch   (rw) : ORIGIN = 0x80001300, LENGTH = 256
    vectors (rw)   : ORIGIN = 0x80001400, LENGTH = 1K
    registry (rw)  : ORIGIN = 0x80001800, LENGTH = 256
}

/* NOTE: QEMU sifive_e faults stores/fetches at the real FE310 ITIM window
 * 0x0800_0000 (ITIM requires PRCI clock enable on silicon; QEMU leaves it
 * unmapped). EXEC_BUFFER therefore lives in DTIM alongside .bss; execute
 * permission is granted by the post-link RWE segment patch
 * (scripts/patch-riscv-x.py). EXEC_BUFFER is sized 1K — REPL-generated
 * streams are far smaller, and DTIM totals only 8K shared with data/bss. */

INCLUDE memory-layout-riscv.x
