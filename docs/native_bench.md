# Native vs Threaded JIT Benchmark (RoadMap M4)

> `bench;` REPL command · stream `lit 2, lit 3, add, halt` · 1000 iterations
> Cycle sources: DWT->CYCCNT (ARM) / mcycle CSR (RISC-V) · QEMU 8.2.2

## Results — QEMU sifive_e, riscv32imac, release build

| Metric | Cycles |
|---|---|
| Threaded dispatch (steady state) | **2636 cyc/exec** |
| First native call (scan+emit+fence+jump+exec) | 1,611,308 cyc |
| Native steady state | **696 cyc/exec** |
| Speedup | **x3.78** |

## Notes

1. **First-call cost is TCG-dominated**: 1.6M cycles includes QEMU translating the
   freshly-emitted code block on first execution — an emulator artifact, not kernel
   cost. On silicon the emit+fence+jump path is bounded by AXIS-4's O(n) scan
   (~25 cyc/byte + ~12 instr emission).
2. **ARM numbers unavailable under QEMU**: netduinoplus2 model does not implement
   DWT->CYCCNT (reads return 0). The bench prints zero rows and continues by
   design. Silicon bring-up (STM32F407, probe-rs) will fill this column; DWT is
   core to ARMv7-M and enabled in-bench (TRCENA|CYCCNTENA).
3. **Why only 3.78x and not more**: for a 4-word stream, threaded dispatch pays
   ~4 fn-pointer chases (~600c each incl. volatile fetches through SRAM), while
   native executes ~10 instructions (~700c incl. call/fence amortized). Larger
   streams widen the gap linearly — dispatch overhead scales with token count,
   native with basic blocks.
4. RoadMap M4 claim "threaded ~100us compile / native ~1ms" refers to *compile*
   wall-time on host tooling; on-target both compiles are cycle-counted here
   instead (first-call row).

## Reproduce

```sh
cargo run --target riscv32imac-unknown-none-elf --release
holy> bench
```
