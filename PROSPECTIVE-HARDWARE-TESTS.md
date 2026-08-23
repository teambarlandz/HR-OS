# PROSPECTIVE-HARDWARE-TESTS.md

> **Purpose:** every test that requires real silicon, written down _before_ the hardware
> arrives. Each entry carries a difficulty tier, exact procedure, and pass/fail gates.
> Nothing speculative — each closes a claim already made in BENCHMARK.md / WCEF.md /
> report.md / testv2.md that could only be simulated on host or QEMU.
>
> **Targets:** STM32F407VG / F429ZI (Cortex-M4F/M7, 168 MHz), SiFive E310 (RV32IMAC),
> optional x86_64 Atom x6000E. Tooling: probe-rs + ST-Link v3 (4-pin SWD).
> **Rule:** a test is PASS only when its numeric gate prints from on-target code.
> Host/QEMU evidence does not transfer.

---

## Hardware inventory (what to put on the bench)

| #   | Item                                                 | Role                                        | Notes                                                                                |
| --- | ---------------------------------------------------- | ------------------------------------------- | ------------------------------------------------------------------------------------ |
| H1  | STM32F407VG-DISC1                                    | Primary ARM target: S1, S3-S6, C1-C7, X1-X8 | Cortex-M4F @168 MHz, DWT present; user LED on PA05 (E2E poke test), user button PA00 |
| H2  | STM32F429ZI-DISC2 (optional)                         | M7 variant check: C1, C2                    | 216 MHz capable; confirms cycle bounds scale with clock                              |
| H3  | SiFive HiFive1 Rev B / FE310 board                   | RISC-V target: S2, S4, C2, C5               | RV32IMAC; ITIM needs PRCI enable on real silicon (QEMU-unmapped caveat)              |
| H4  | x86_64 Atom x6000E board (optional, late)            | C8 ECAM, baremetal-x86 port                 | Ring 0, LAPIC; QEMU q35 stand-in until purchased                                     |
| H5  | ST-Link v3 (or probe-rs-compatible debug probe)      | Flash + SWD + RTT for all ARM/RISC-V boards | 4-pin SWD: SWDIO, SWCLK, NRST, GND                                                   |
| H6  | USB-UART adapter (3.3 V)                             | Console capture when RTT is insufficient    | 115200-style baud, matches QEMU serial behavior                                      |
| H7  | Signal/function generator (for C3)                   | EXTI trigger source at known frequency      | any GPIO-capable square wave                                                         |
| H8  | Bench PSU with glitch-injection capability (X5 only) | Voltage-glitch fault injection              | deferred until X-tier                                                                |

### Per-board setup checklist

- STM32F4x Discovery: power via USB (ST-Link); SWD through onboard ST-Link or external H5;
  clock = HSI-PLL to 168 MHz before SysTick config (init_data_bss/boot path already does UART,
  VTOR relocation; confirm PLL lock in boot log)
- SiFive E310: JTAG via Olimex/arm-usb-tiny or FTDI; DTIM 8K carve as linked; PRCI: enable
  ITIM clock only if EXEC_BUFFER moves back to 0x0800_0000 (currently DTIM-resident)
- All boards: Secure Boot/NRST strap cleared so `probe-rs reset --halt` works for X-tier tests

### Test-to-hardware mapping

| Tests                   | Board(s)          |
| ----------------------- | ----------------- |
| S1, S3-S6, C1-C7, X1-X8 | H1 (+H2 optional) |
| S2, S4, C5              | H3                |
| C8                      | H4 only           |

---

## Difficulty legend

| Tier        | Meaning                                                                     |
| ----------- | --------------------------------------------------------------------------- |
| Simple      | single register read/flag check; minutes; no silicon risk                   |
| Complex     | multi-step procedure or timing measurement; needs harness scripting         |
| Complicated | cross-subsystem invariants, adversarial sequences, physical fault injection |

---

## TIER 1 - SIMPLE

### S1. DWT cycle-counter sanity (ARM)

- Procedure: enable TRCENA/CYCCNTENA; read DWT->CYCCNT @0xE0001004, NOP sled, read again
- Pass: monotonic non-zero delta; delta/N within 5 pct of 1 cyc/instr
- Closes: ARM bench zero-column (native_bench.md note 2)

### S2. mcycle sanity (RISC-V)

- Procedure: csrr mcycle twice around a NOP sled
- Pass: monotonic non-zero delta

### S3. UART console parity with QEMU

- Procedure: flash release ELF; capture banner + prompt via probe-rs
- Pass: byte-identical `Holy Rust REPL v0.1` + prompt vs QEMU log

### S4. Scratch persistence across REPL lines (riscv32)

- Procedure: poke 0x80001340 0xCAFE; three unrelated commands; peek
- Pass: = 0x0000CAFE after churn

### S5. ELF segment sanity on flashed image

- Procedure: readelf -l on flashed ELF; compare PHDR vaddr/memsz to QEMU build
- Pass: no memsz > SRAM size; EXEC_BUFFER present at expected vaddr; no >1 MB segments

### S6. Capability fail-closed on cold boot

- Procedure: fresh power-on; poke GPIO without cap_claim
- Pass: E001 CAPABILITY_VIOLATION before any claim (no stale SRAM state grants access)

---

## TIER 2 - COMPLEX

### C1. Context-switch determinism: sigma == 0 (WCEF/BENCHMARK headline)

- Procedure: two-task ping-pong, 10k forced switches; per-switch DWT delta into SRAM histogram; UART dump
- Pass: mean <= 43 cyc (+2 pipeline); max-min == 0; p99 == mode
- Closes: testv2 host-simulated determinism caveat; BENCHMARK row "43 cycles"

### C2. Native-vs-threaded on-target (fills native_bench.md ARM column)

- Procedure: existing bench; command on target; record DWT/mcycle rows
- Pass: native < threaded steady-state (>= 2x); first-call within AXIS-4 bound (<= 25 cyc/byte x len x 4); ARM rows non-zero

### C3. Interrupt latency IRQ-to-ISR = 12 cycles

- Procedure: EXTI line triggers ISR; ISR entry writes GPIO BSRR; measure with timer-capture latching CYCCNT
- Pass: <= 12 cyc +/-1; jitter 0 over 1,000 triggers
- Closes: BENCHMARK row "12 cycles"

### C4. IPC capability shift = 8 cycles (ZERO-COPY.md)

- Procedure: timed release(A)/acquire(B) loop x100k, cycle-counted per op
- Pass: p99 <= 8 cyc + barrier allowance; 0 failed acquires post-release

### C5. WWDT dual-bound window on real peripheral

- Procedure: configure t_lower/t_upper; 1,000 feeds per class (early/in/late)
- Pass: 0 false NMI in-window; 100 percent NMI out-of-window

### C6. SysTick quantum accuracy (AXIS-1)

- Procedure: N = f x dt reload; capture 1M tick intervals with DWT; histogram
- Pass: interval spread <= 16 ticks (crystal ppm + rounding); mean within 0.1 pct

### C7. DMA ring wrap-around at hardware speed

- Procedure: fill 127 descriptors, drain 64, refill 64 x10k rounds; verify data integrity by CRC per descriptor
- Pass: 0 lost transfers, 0 corrupt payloads, capacity invariant every round

### C8. ECAM enumeration on real PCIe (x86_64 target only)

- Procedure: sweep all B/D/F; compare vendor/device IDs against lspci baseline
- Pass: identical endpoint set; O(1) address formula spot-check on 256 random endpoints

---

## TIER 3 - COMPLICATED

### X1. Invalid-opcode trap recovery < 15 cycles (INVALID-OP-CODES.md)

- Procedure: JIT-emit an illegal encoding into EXEC_BUFFER; execute; .FAULT_TRAP must fire; count trap-entry to scheduler-resume cycles via CYCCNT latched in handler
- Pass: task killed, capability vector zeroed, next task dispatched; total <= 15 cyc; system survives 10k consecutive illegal-execution attempts
- Risk: deliberate Ring-0 fault storms; watchdog must be masked during measurement

### X2. WWDT NMI during live DMA storm

- Procedure: saturate DMA ring; force t_upper expiry mid-storm
- Pass: NMI preempts DMA-completion IRQs (NMI non-maskable proof); ring recovers without descriptor corruption

### X3. Multi-core lock-free queue under adversarial load (UPGRADE.md Step 2)

- Procedure: 2+ cores hammer push/pop with randomized bursts x10M ops; loom-equivalent on-silicon invariant checks
- Pass: no lost/duplicated tasks; p95 dispatch <= 12 cyc; MESI bouncing bounded (perf counters)

### X4. Speculative-execution guard (watchlist #8)

- Procedure: Spectre-v1 style cache flush+reload around TBZ guard with mistrained predictor
- Pass: 0 secret-dependent cache hits across 1M attempts (CSDB barrier effective)

### X5. Physical fault injection: voltage glitch on EXEC_BUFFER fetch

- Procedure: controlled glitch during native JIT execution; expect .FAULT_TRAP or WWDT reset, never silent corruption
- Pass: 100 percent of glitches end in visible fault/reset; audit log intact

### X6. RTA schedulability under full task set (WCEF.md section 4)

- Procedure: load n=8 periodic tasks from testv2 T6; run 24h soak
- Pass: zero deadline misses; R_i <= D_i holds for entire soak; zero drift

### X7. 24h determinism soak

- Procedure: continuous REPL poke/peek/bench loop; hourly cycle-histogram dumps
- Pass: sigma stays 0; no memory growth (static-only proof); no capability-state drift

### X8. Power-cycle state audit

- Procedure: hard power-cut mid-JIT-compile x1,000 randomized timings
- Pass: boot always clean; registry/vectors initialized correctly by init_data_bss path; no partial-stream execution ever observed

---

## Execution order when hardware lands

1. S1-S6 same day (board sanity, minutes each)
2. C1, C2 immediately after - they close the two loudest open caveats
3. C3-C8 in any order
4. X-tier only after C-tier is green: adversarial tests assume working baselines

## Traceability

Every gate above maps to a spec file: BENCHMARK.md (43c/12c/8c rows), WCEF.md (sigma 0,
RTA), ZERO-COPY.md (IPC), INVALID-OP-CODES.md (.FAULT_TRAP, WWDT), UPGRADE.md (multi-core),
testv2.md (T1-T9 host equivalents). When a test passes, update the corresponding spec row
from 'simulated' to 'silicon-proven' and note the board serial.
