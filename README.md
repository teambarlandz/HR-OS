# HR-OS — Holy Rust Unikernel Operating System

> **v0.2.0** · **SASA · O(1) Capabilities · 43-Cycle Scheduler · Single-Pass JIT · Native JIT ×3.78**
>
> **Status:** kernel proven on QEMU (ARM + RISC-V) — native R/W/X live, capability-enforced,
> persistence store, driver surface (PWM/SPI), silicon test plan staged.

A zero-cost, memory-safe, real-time **Single Address Space Architecture (SASA)** unikernel that executes exclusively in **Ring 0 / EL1 / M-mode** — no MMU page walks, no TLB flushes, no POSIX, no ELF dynamic linking, no IOMMU. Safety is a **bit-test**, not a page fault.

```
              HOLY RUST UNIKERNEL OS (HR-OS) — 4 Axes
 ┌─────────────────────────────────────────────────────────┐
 │ Layer 4: Mathematical Execution & JIT Compiler Engine   │
 ├─────────────────────────────────────────────────────────┤
 │ Layer 3: O(1) Deterministic Capability Matrix           │
 ├─────────────────────────────────────────────────────────┤
 │ Layer 2: Ring-0 Flat Memory Space & Interconnect Bridge │
 ├─────────────────────────────────────────────────────────┤
 │ Layer 1: Hardware-Synchronized Temporal Core Engine     │
 └─────────────────────────────────────────────────────────┘
```

---

## Why HR-OS

| Invariant           | Traditional OS                             | HR-OS                                                 |
| ------------------- | ------------------------------------------ | ----------------------------------------------------- |
| Address translation | `VA → PA` page walk 10–100c + TLB miss     | **VA ≡ PA** identity, 0c                              |
| Context switch      | Linux 1k–10k c / seL4 ~310c / FreeRTOS 84c | **43c** (12 auto +8 push +3 sched +8 pop +12 unstack) |
| Interrupt latency   | 120–180c (MPU reconfig)                    | **12c** pure HW                                       |
| IPC 1.5 KiB         | pipe 400c 2.5µs / mmap 140c 0.85µs         | **8c 0.048µs** (cap shift, 0 copy)                    |
| DMA                 | IOMMU fault + IoTLB                        | **0 blocked** (Autonomous Ring, PCIe TLP)             |
| Memory safety       | MMU/MPU                                    | **3c scalar / 1c SIMD** 256-bit bitmask               |
| Jitter `σ`          | 2–15µs                                     | **0** (SASA, no TLB)                                  |

Measured on ARM Cortex-M4 @ 168 MHz (`docs/technical/BENCHMARK.md:1`). Full matrix in `docs/production/HR-OS_PRODUCTION_BLUEPRINT.md:1`.

---

## What's New in v0.2.0

| Advance                       | Detail                                                                                                         |
| ----------------------------- | -------------------------------------------------------------------------------------------------------------- |
| **Native JIT on both arches** | EXEC_BUFFER relocated to DTIM (riscv32); gate lifted — `bench;` shows **×3.78** vs threaded (2636 → 696 cyc)   |
| **Program persistence**       | `store NAME;` / `load NAME;` / `store_list;` — named fn bodies survive REPL state in an SRAM slot file         |
| **Driver expansion**          | `pwm PERIOD DUTY;`, `pwm_duty DUTY;`, `spi_tx BYTE;` — capability-gated TIM2/PWM0 + SPI1/SPI0, arch-aware MMIO |
| **Stack-slack ASSERTs**       | linker enforces ≥1536B (rv32) / ≥4K (arm) headroom — silent overflow becomes build failure                     |
| **Dedicated scratch region**  | riscv32 DTIM: 256B at `0x80001300` — always-safe poke/peek target                                              |
| **Feature gates**             | `simulated-hw` (default) / `baremetal-arm` / `baremetal-x86`                                                   |
| **Silicon test plan**         | PROSPECTIVE-HARDWARE-TESTS.md — 22 tests across S/C/X tiers with inventory matrix                              |

Full language/command reference: [Milestone.md](Milestone.md).

---

## Repository Layout

```
HR-OS/                          # ← you are here (GitHub: teambarlandz/HR-OS)
├── README.md                   # this file
├── Todo.md                     # current & immediate tasks (Phase 0)
├── docs/
│   ├── technical/              # 14 original formal specs (formatted strict Markdown)
│   │   ├── AXIS-1.md           # Temporal core & 43c switch
│   │   ├── AXIS-2.md           # Flat SASA & PCIe ECAM
│   │   ├── AXIS-3.md           # O(1) capability proofs & SIMD
│   │   ├── AXIS-4.md           # Single-pass LL(1) JIT & emitters
│   │   ├── BENCHMARK.md        # FreeRTOS/seL4 vs HR-OS @168 MHz
│   │   ├── DMA.md              # DMA + IRQ safety
│   │   ├── E2E-SYSTEM-TRACE.md # poke 0x40021018 → 85c 0.50µs trace
│   │   ├── FORWARD.md          # Bare-metal `no_std` bring-up guide
│   │   ├── Holy-Rust-Unikernel-Operating-System.md # Master framework
│   │   ├── INVALID-OP-CODES.md # #UD + WWDT fault traps
│   │   ├── SYNTHESIS.md        # Consolidated spec sheet
│   │   ├── UPGRADE.md          # SIMD + multi-core + DMA + RISC-V upgrades
│   │   ├── WCEF.md             # WCET & RTA proofs
│   │   └── ZERO-COPY.md        # Linear token transfer 8c
│   └── production/
│       └── HR-OS_PRODUCTION_BLUEPRINT.md # Deliverables 1–4 + HAL + edge cases
├── linker/                     # (Phase 0) memory.x, memory-riscv.x, memory-layout.* (SASA contract)
├── targets/                    # (Phase 0) x86_64-hros-none.json, riscv64-hros-none.json
├── .cargo/config.toml          # (Phase 0) QEMU runners (netduinoplus2 & sifive_e)
├── build.rs                    # (Phase 0) arch-select linker
├── Cargo.toml                  # (Phase 0) workspace
├── crates/                     # (Phase 0–4) hros-hal / arch-* / cap / kernel / jit / drivers / core
└── .github/workflows/ci.yml    # (Phase 0) build + clippy + qemu HIL
```

Reference implementation lives in [`holy-rust/`](../holy_rust) (compiles today on `thumbv7em-none-eabihf` & `riscv32imac-unknown-none-elf` — see `holy-rust/Thought.md`).

---

## Documentation Index

- **Production Blueprint:** `docs/production/HR-OS_PRODUCTION_BLUEPRINT.md` — roadmap Phase 0–4, workspace `crates/` tree, `no_std` HAL traits (`ContextSwitch`, `InterruptController`, `VectorCapabilityEngine`, `ExecutionBuffer`), 10-point silicon physics watchlist, cycle ledger.
- **Technical Specs:** `docs/technical/` — all 14 formatted specs. Start with `Holy-Rust-Unikernel-Operating-System.md` then `AXIS-1.md` → `AXIS-4.md`.
- **Implementation Log:** `holy-rust/Thought.md` (machine caps, linkers, 7 bring-up bugs §12), `holy-rust/RoadMap.md` (milestones M1–M5), `holy-rust/docs/CHAPTER_*.md` (6 chapters).
- **HAL:** `holy-rust/crates/hros-hal/src/{switch,irq,cap,exec}.rs` — zero-cost traits proven `cargo check --target thumbv7em-none-eabihf`.

---

## Quick Start (Phase 0 Host)

```bash
# 1. Toolchain (nightly required for build-std)
rustup toolchain install nightly
rustup component add rust-src llvm-tools-preview rustfmt clippy --toolchain nightly
rustup target add thumbv7em-none-eabihf riscv32imac-unknown-none-elf

# 2. Clone
git clone git@github.com:teambarlandz/HR-OS.git
cd HR-OS
# (Phase 0 will add Cargo.toml, linker/, targets/ — for now see holy-rust/)
git clone git@github.com:teambarlandz/holy-rust.git

# 3. Build reference impl (today) — holy-rust workspace
cargo --manifest-path holy-rust/Cargo.toml build --target thumbv7em-none-eabihf
cargo --manifest-path holy-rust/Cargo.toml build --target riscv32imac-unknown-none-elf --release

# 4. QEMU HIL (once .cargo/config.toml lands)
cargo run --target thumbv7em-none-eabihf   # → netduinoplus2, expect "Holy Rust REPL v0.1"
cargo run --target riscv32imac-unknown-none-elf # → sifive_e @0x20400000

# 5. REPL (Holy Rust DSL + built-ins)
picocom -b 115200 /dev/ttyACM0
# holy> cap_claim GPIOA;
# holy> poke 0x40020000 1;
# holy> peek 0x40020000;
# holy> bench;                # native-vs-threaded cycles
# holy> fn blink() { poke 0x40020000 1; }
# holy> store blink;          # persist to program store
# holy> pwm 1000 500;         # capability-gated PWM (claim TIMER0 first)
```

---

## SASA Contract (Physical Memory Map)

```
0x08000000  flash  (RX)  128K  ARM STM32F405 / RISCV 0x20400000 SiFive flash
0x20000400  vectors (RW) 3K   RAM_VECTOR_TABLE align(1024) → VTOR 0xE000ED08 / mtvec
0x20001000  registry (RW) 256B REGISTRY_BITS AtomicU32[8] (256 caps, 4K granularity)
0x20002000  sram_code (RWX) 4K  EXEC_BUFFER (ITIM 0x08000000 on SiFive)
0x20003000  sram (RW) 52K  .data/.bss + stack (_stack_top = ORIGIN+LENGTH)
0x40000000+ MMIO PCIe BARs, GPIO, UART, DMA (identity-mapped, poke/peek)
```

See `docs/technical/AXIS-2.md:1` and `holy-rust/memory.x:1`.

---

## Development Workflow

- **Current:** Phase 0 (see `Todo.md`). All changes must preserve SASA, O(1), 43c, no `alloc`/`dyn` — enforced by `clippy::undocumented_unsafe_blocks` + `rg "todo!|dyn\s"`.
- **Branching:** `main` is trunk. Feature branches `phase/0-toolchain`, `phase/1-vectors`, `phase/2-cap-simd` → PR → CI must be green (`build + clippy + fmt + qemu`).
- **Commits:** Conventional — e.g., `phase0: add x86_64-hros-none target` / `phase1: relocate VTOR to SRAM`.
- **Verification:** Every phase has cycle-inequality DoD in blueprint. No phase closes without `DWT->CYCCNT` or `mcycle` proof.

---

## Roadmap Snapshot

| Phase | Title                 | Exit Gate (DoD)                                       |
| ----- | --------------------- | ----------------------------------------------------- |
| **0** | Toolchain & HIL       | 3 targets 0 warn, QEMU banner <100ms, `sram_code` RWE |
| **1** | Bare-Metal Foundation | `VTOR==0x20000400`, `**FAULT**` <2c, no lockup        |
| **2** | Axes 1 & 3            | `43c` jitter 0, guard `3→1c`, 128KiB cached           |
| **3** | Axes 2 & 4            | `85c` poke e2e, `8c` IPC, `25c/B` JIT                 |
| **4** | Proof & Fuzz          | `R_i≤D_i` proved, 1M fuzz 0 escapes                   |

Full matrix: `docs/production/HR-OS_PRODUCTION_BLUEPRINT.md`.

---

## Contributing

PRs must: (1) keep `docs/technical/` specs as normative reference, (2) add `// SAFETY:` to every `unsafe`, (3) `cargo fmt --check` + `clippy -D warnings`, (4) demonstrate cycle count in commit message when touching hot paths.

---

## License

MIT OR Apache-2.0 — same as `holy-rust`.

---

_Built from first-principles: crystal → interrupt → context → capability → JIT → silicon._
