# HR-OS Hardware Requirements Specification

> Reference blueprint for physical silicon bring-up (answers.md Section 3).
> Companion to `docs/production/HR-OS_PRODUCTION_BLUEPRINT.md` and `linker/*.x`.

---

## 1. Target Microcontrollers & Processors

| Target      | Silicon                                                                          | ISA / Core              | Clock       | Notes                                                                                                                       |
| ----------- | -------------------------------------------------------------------------------- | ----------------------- | ----------- | --------------------------------------------------------------------------------------------------------------------------- |
| Primary ARM | **STM32F407VG** (Discovery) / **STM32F429ZI** (Discovery)                        | ARMv7-M Cortex-M4F / M7 | 168–216 MHz | Onboard DWT cycle counter (`DWT->CYCCNT` @ `0xE0001004`) and SysTick; QEMU stand-in: `netduinoplus2` (STM32F405)            |
| x86_64      | Industrial embedded core, e.g. **Intel Atom x6000E** — or QEMU `q35`/`pc` target | x86_64, strictly Ring 0 | Any         | LAPIC enabled; no MMU page tables enabled; IDT-only trap routing. AVX2 recommended for the 1-cycle 256-bit capability guard |

### Tooling

- Flash/debug: **probe-rs** via ST-Link v3 (4-pin SWD: SWDIO, SWCLK, NRST, GND).
- Cycle truth: DWT->CYCCNT on ARM; `mcycle` CSR or CLINT mtime on RISC-V.

## 2. Memory Real Estate (SASA Identity-Mapped)

VA ≡ PA everywhere; no MMU translation, no TLB.

| Region                 | ARM carve (STM32F4)                                                                                                                          | RISC-V carve (SiFive E310 / QEMU sifive_e)                                                                                                              |
| ---------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Capability Matrix SRAM | 128 KB @ `0x2000_0000_0080_0000` blueprint address; current build: `.capability_registry` @ `0x2000_1000` (256 resources × 4 KB granularity) | `.capability_registry` @ `0x8000_1800` (256 B, DTIM)                                                                                                    |
| EXEC_BUFFER            | `.sram_code` @ `0x2000_2000`, 4 K RWX                                                                                                        | `.sram_code` @ `0x8000_1000`, 1 K RWX in DTIM (post-link RWE patch); ITIM window `0x0800_0000` unusable under QEMU (unmapped without PRCI clock enable) |
| Task stacks + data     | SRAM pool above `0x2000_3000`, stack descends from `_stack_top`                                                                              | DTIM `sram` @ `0x8000_0000` (4 K), stack descends from `0x8000_1000`                                                                                    |

**Minimums for production silicon:**

- **SRAM ≥ 192 KB total**: 128 KB dedicated to the O(1) Capability Matrix; remainder to EXEC_BUFFER and task stack frames.
- **Flash ≥ 512 KB NOR** onboard (XIP), vector table at flash base.

> QEMU constraints discovered during verification: (a) the SiFive E310 ITIM
> (`0x0800_0000`) requires PRCI clock enable on real silicon and is unmapped/faulting
> under QEMU 8.2 — EXEC_BUFFER was re-carved into DTIM; (b) QEMU softmmu does NOT
> enforce segment X flags on mapped RAM — native JIT runs from a plain-RW PT_LOAD,
> so `scripts/patch-riscv-x.py` is required only for real PMP-enforcing silicon.

### Linker decision record (why rust-lld + python patch, not GNU ld)

Five lld PHDRS variants and a GNU ld swap were tested exhaustively:

| Attempt                                       | Result                                                                                                                                                                                                           |
| --------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| rust-lld + multi-phdr PHDRS, `.data AT>flash` | `section '.data' will not fit in region 'flash': overflowed by ~384 MB` (lld LMA arithmetic bug)                                                                                                                 |
| rust-lld + `-z noseparate-code`               | identical failure                                                                                                                                                                                                |
| rust-lld + PHDRS, no `AT>`                    | file-offset collision: `.text` overlaps `.shstrtab` / `.riscv.attributes`                                                                                                                                        |
| rust-lld + single RWX phdr                    | links, but corrupt: `p_filesz=0x5fc00000` (~1.6 GB garbage), QEMU hangs                                                                                                                                          |
| **GNU ld 2.42**                               | links and runs — **but** mixed-LMA phdr produced `p_memsz=0x5fbfd688` (~1.6 GB bogus BSS segment); splitting into `data_init`/`data_bss` phdrs fixed memsz but adds a second toolchain dependency for one target |

**Decision:** stay on rust-lld (zero extra host deps) + `scripts/patch-riscv-x.py`
(4-byte in-place flag flip, stdlib-only, idempotent, refuses unexpected states).
The script runs once per silicon flash — never in the QEMU path — and the kernel
image itself is unchanged (~15 K text). Revisit if a strict loader ever rejects
the unpatched ELF on real hardware.

## 3. Hardware Interlocks & Debug

| Interlock    | Requirement                                                                                                                                                  |
| ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Watchdog     | Dual-bound **Windowed Watchdog Timer (WWDT)** wired to CPU **NMI**: feed rejected before `t_lower`, NMI after `t_upper`; hysteresis ±2σ per `WCEF.md`        |
| Trap routing | ARM: VTOR → SRAM table (`repr(align(1024))`); RISC-V: `mtvec` direct mode, 4-byte aligned                                                                    |
| Debug port   | 4-pin SWD (SWDIO, SWCLK, NRST, GND) via probe-rs / ST-Link v3; fault visibility over UART (`fault_hang` announces `**FAULT: core exception, halted**`)       |
| Console      | UART 115200-style: STM32 USART1 `0x4001_1000` (CR1 UE\|TE\|RE); SiFive UART0 `0x1001_3000` (txdata/rxdata); x86_64 stub: COM1 `0x3F8` behind `baremetal-x86` |

## 4. Feature-Gated Hardware Abstraction (build-time contract)

| Cargo feature (hros-kernel) | Meaning                                      | Jitter gate                |
| --------------------------- | -------------------------------------------- | -------------------------- |
| `simulated-hw` (default)    | Hosted tests + QEMU; software cycle mocks    | statistical tail `<100 µs` |
| `baremetal-arm`             | Direct DWT->CYCCNT / SysTick register access | strict `delta == 0`        |
| `baremetal-x86`             | LAPIC timer, IDT, 16550 COM1 assembly        | (reserved)                 |

Strict zero-delta proof requires: real silicon + `--features baremetal-arm` +
`verify_jitter_bounds(0)` from an on-target DWT harness.
