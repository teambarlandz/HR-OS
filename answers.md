Project Directives: Transitioning to Phase 2 & Resolving Open Hardening Gaps
Context & Directive Overview:
We are moving the project into Phase 2. To unblock development without physical hardware testing setups, we are adopting a formal stubbing strategy for hardware-dependent targets (Tasks 1 & 3) while applying a clean, linker-level fix for the RISC-V PT_LOAD section flag issue (Task 2).
Execute the following three-part action plan across the workspace:
1. Resolution for Item 2: RISC-V PT_LOAD Flag Fix (RW \to RWE)
Objective: Fix execution permissions for native synthesized instructions on RISC-V targets (sifive_e / riscv32) without relying on post-build host tool invocations (e.g., llvm-objcopy inside build.rs).
Action Items:
 * Update the RISC-V linker script (link.x) to explicitly define Program Headers (PHDRS) and assign read-write-execute (RWE) flags to the .sram_code / EXEC_BUFFER section at link time:
   PHDRS
{
  text PT_LOAD FLAGS(5);       /* RX: Read=4 + Exec=1 */
  ram_code PT_LOAD FLAGS(7);   /* RWE: Read=4 + Write=2 + Exec=1 */
  data PT_LOAD FLAGS(6);       /* RW: Read=4 + Write=2 */
}

SECTIONS
{
  .sram_code : ALIGN(4)
  {
    *(.sram_code .sram_code.*);
  } > RAM AT > FLASH :ram_code
}

 * Verify that QEMU sifive_e executes native compiled code out of EXEC_BUFFER without dropping into threaded fallback mode.
 * Lift the native.rs gate for riscv32 and re-run local integration tests to confirm native execution speeds.
2. Stubbing Strategy for Items 1 & 3: Hardware Abstraction & Architecture Isolation
Objective: Cleanly stub out bare-metal hardware-dependent targets (x86_64 LAPIC/IDT/COM1 and ARM DWT bare-metal cycle counters) behind trait-based HAL interfaces and Cargo feature flags so the workspace remains 100% green without physical hardware.
Action Items:
 * Configure Feature Gates in Cargo.toml:
   Implement conditional compilation flags to separate hosted/simulated execution from bare-metal targets:
   [features]
default = ["simulated-hw"]
simulated-hw = []  # Software cycle mocks / QEMU-friendly stubs
baremetal-arm = [] # Direct ARM Cortex-M DWT registers & SysTick
baremetal-x86 = [] # x86_64 LAPIC, IDT, and 16550 UART assembly

 * Stub hros-arch-x86 (Task 3 & 4):
   * Keep the hros-arch-x86 crate cleanly stubbed behind standard trait abstractions.
   * Provide safe software mocks for the IDT, APIC timer, and COM1 UART when simulated-hw is active.
   * Restrict live hardware assembly calls to the --feature baremetal-x86 flag.
 * Isolate Bare-Metal Hardware Requirements (Task 1):
   * Gate the 0\text{-delta} execution jitter check (verify_jitter_bounds() == 0) strictly behind #[cfg(all(target_os = "none", feature = "baremetal-arm"))].
   * Fall back to percentile/statistical tail-gating (p99/p99.9) under hosted/QEMU simulation environments to prevent OS preemption artifacts from breaking test runs.
3. Reference Blueprint: Hardware Requirements Specification
For architectural documentation and future physical manufacturing/testing, record the following target silicon specifications in docs/hardware_spec.md:
 * Target Microcontrollers & Processors:
   * ARM Cortex-M4/M7: STM32F407VG / STM32F429ZI (32-bit ARMv7-M, 168–216 MHz, onboard DWT cycle counter, SysTick timer).
   * x86_64: Industrial Embedded x86_64 core (e.g., Intel Atom x6000E or QEMU target running strictly at Ring 0 / EL1 with LAPIC enabled).
 * Memory Real Estate (SASA Identity-Mapped Space):
   * SRAM: Minimum 192 KB total. 128 KB dedicated to the O(1) Capability Matrix SRAM (0x0000_0000_0080_0000); remaining memory mapped to EXEC_BUFFER (0x0000_0000_1000_0000) and task stack frames.
   * Flash: Minimum 512 KB onboard NOR Flash.
 * Hardware Interlocks & Debug:
   * Dual-bound Windowed Watchdog Timer (WWDT) connected to CPU Non-Maskable Interrupt (NMI).
   * 4-pin SWD Interface (SWDIO, SWCLK, NRST, GND) via probe-rs / ST-Link v3.
Execution Step: Implement the linker script changes for RISC-V (Section 1) and feature-gate the stubbed architectures (Section 2), then run cargo test and cargo check --all-targets to verify all gates pass.
