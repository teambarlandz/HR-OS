# Todo — HR-OS (Holy Rust OS) Implementation

> **Source of truth:** `docs/production/HR-OS_PRODUCTION_BLUEPRINT.md` (Deliverables 1–4). This file tracks _current_ and _immediate_ work only — not the full roadmap.

## Current Phase: Phase 0 — Toolchain, Linker, Custom Targets & QEMU HIL Harness

**Goal:** Reproducible cross-build from `x86_64-unknown-linux` host to all HR-OS SASA targets without host linker contamination. This phase blocks all later work.

**Status:** `IN PROGRESS — Phase 0 70%` — docs reorganized, blueprint published, HAL traits proven (`hros-hal` compiles on `thumbv7em`/`riscv32`). Next: scaffold `HR-OS` as Cargo workspace matching `holy-rust` layout.

---

### Immediate Tasks (Next 7 Days)

#### 0.1 Toolchain Pinning

- [x] Add `rust-toolchain.toml` at project root: `channel="nightly"` + `components=["rust-src","llvm-tools-preview","rustfmt","clippy"]` _(done 2026-08-22)_
- [x] Verify `rustc 1.100 nightly` + `cargo 1.100` + `thumbv7em-none-eabihf`/`riscv32imac-unknown-none-elf` installed — `cargo check` passes for both _(2026-08-22)_
- [x] Add custom targets `targets/x86_64-hros-none.json` and `targets/riscv64-hros-none.json` — fixed `target-pointer-width` int + `rustc-abi softfloat`; `thumbv7em`/`riscv32` pass; `x86_64`/`riscv64` stretch pending holy-rust x86 port

#### 0.2 Linker Family & SASA Contract

- [x] Create `linker/` + root mirrors for SASA contract:
  - [x] `memory.x` — ARM STM32F405: `flash 128K@0x08000000`, `sram 52K@0x20003000` + carved `vectors@0x20000400` / `registry@0x20001000` / `sram_code@0x20002000`
  - [x] `memory-riscv.x` — SiFive E310: `flash 0x20400000`, `DTIM 8K@0x80000000`, `ITIM 4K@0x08000000`
  - [x] `memory-layout.x` — shared `SECTIONS`: `.isr_vector` 16-word `LONG(_stack_top) LONG(Reset) LONG(fault_hang)×` + `KEEP(*(.isr_vector))` + `/DISCARD/`
  - [x] `memory-layout-riscv.x` — RISC-V variant (no vector table, `Reset` first)
  - [ ] `HR-OS_SASA.ld` consolidated view _(pending)_
- [x] Define `_stack_top = ORIGIN(sram)+LENGTH(sram)` + `VTOR 1024B` alignment — linker validates, `cargo check` pass

#### 0.3 Build Script & Cargo Manifest

- [x] Write `build.rs` arch-selector (`CARGO_CFG_TARGET_ARCH == "riscv32" → memory-riscv.x else memory.x`, validate `ORIGIN/LENGTH/INCLUDE`, emit `cargo:rustc-link-arg=-T<linker>` + `rerun-if-changed`) — copied from `holy-rust/build.rs`
- [ ] Create workspace `Cargo.toml` with `[workspace]` members `crates/*` — **DEFERRED:** HR-OS currently single-crate `holy-rust` copy for boot-strap; workspace migration queued for 0.3b
- [x] Scaffold `crates/hros-hal` — proven standalone `cargo check --target thumbv7em/riscv32` pass; `hros-arch-*` / `hros-cap` / `hros-kernel` / `hros-jit` / `hros-drivers` / `hros-core` queued

#### 0.4 Cargo Config & Runners

- [x] Write `.cargo/config.toml` — `target thumbv7em-none-eabihf` default, runner `qemu-system-arm -M netduinoplus2 -cpu cortex-m4 -nographic -kernel` and `riscv32imac-unknown-none-elf` runner `qemu-system-riscv32 -machine sifive_e -nographic -bios none -kernel`, `build-std` unstable
- [x] Test `cargo check --target thumbv7em-none-eabihf` and `riscv32imac` — both pass (~90s); runner appends ELF automatically

#### 0.5 CI Harness

- [x] Add `.github/workflows/ci.yml` — copied from `holy-rust` (build + clippy + fmt + qemu); TODO add `expect` harness + `no-alloc`/`no-dyn` gates
- [ ] Add `scripts/qemu-repl.expect` or `xtask` runner for `peek/poke` roundtrip + `cap_claim` test — queued

#### 0.6 Verification (DoD)

- [x] `cargo check --target thumbv7em-none-eabihf` → 0 errors, 0 warnings (dev)
- [x] `cargo check --target riscv32imac-unknown-none-elf` → 0 errors (dev)
- [ ] `cargo check --target x86_64-hros-none.json` — **stretch:** `holy-rust` lacks `x86_64` cfg, fails SSE/softfloat; deferred to `hros-arch-x86`
- [ ] `llvm-objdump --headers` shows `sram_code RWE`, `nm` shows `RAM_VECTOR_TABLE @0x20000400` aligned — queued (requires release artifact)
- [ ] `qemu-system-arm` boots banner `Holy Rust REPL v0.1` + `holy> ` in <100ms — queued
- [ ] `cargo bloat` shows `strip` ARM ≤150K, RISC-V ≤45K — queued

---

### Phase 1 — Bare-Metal Foundation (Queued, Not Started)

- [ ] `Reset` (ARM plain `fn` vs RISC-V `#[naked]` + `la sp/gp`) + `init_data_bss()` volatile copy
- [ ] `.isr_vector` 16-word linker emission (no double Thumb-bit), `fault_hang` UART announce
- [ ] SRAM relocation to `RAM_VECTOR_TABLE align(1024)` → `VTOR`/`mtvec` + `dsb/isb`/`fence.i`
- [ ] Early console `drivers/uart.rs` (CR1 UE|TE|RE, TXE bit7/RXNE bit5, 256B SPSC ring)
- [ ] `panic_handler` → UART `PANIC:` + `wfi`

_DoD:_ `VTOR==0x20000400`, trap `peek <unmapped>` → `**FAULT**` <2c, no silent lockup, `/DISCARD` `.eh_frame`.

---

### Phase 2 — Axes 1 & 3 (Queued)

- [ ] Axis 1: SysTick 43c switch (12 auto+8 push+3 sched+8 pop+12 unstack), `LockFreeTaskQueue align(64)` CAS+WFE/SEV
- [ ] Axis 3: `REGISTRY_BITS @0x20001000 AtomicU32[8]`, scalar 3c guard, AVX2 `VANDPS+VPTEST` 1c 256×4K, `Cap<T>`/`PinGuard` linear tokens

_DoD:_ `T_ctx==43 ±0`, `σ==0`, guard 3→1c, 128KiB SRAM cached, `rg todo!` ==0, no `alloc`.

---

### Phase 3 — Axes 2 & 4 (Queued)

- [ ] Axis 2: ECAM `Target=Base+(B<<20)|(D<<15)|(F<<12)|R`, `AutonomousDmaRing align(64)` 0 blocked CPU, O(1) range mask
- [ ] Axis 4: `Lexer<'a>` 25c/B, `Compiler` 64/4×64/128, `Thumb2Emitter`/`Riscv32Emitter`, `native.rs` two-reg `ACC=r0/a0`, `flush_icache`

_DoD:_ DMA 8c 0 copy, `poke` e2e 85c 0.50µs, JIT linear `O(n)`, native fallback OK.

---

### Phase 4 — Verification & HIL Fuzz (Queued)

- [ ] WCET ledger `E=T_JIT+T_Exec+T_Cap+T_Ctx`, RTA `R_i≤D_i` proof, 1M fuzz + WWDT, benchmark vs FreeRTOS/seL4

_DoD:_ `σ==0`, 0 escapes, `<15c` `.FAULT_TRAP`, histogram.

---

## How to Update This File

- Check off completed items with `[x]` and append date + commit hash.
- When Phase 0 DoD passes, move this section to `## Completed` and promote Phase 1 to `Current`.
- Never add speculative tasks beyond `docs/production/HR-OS_PRODUCTION_BLUEPRINT.md`.

## Blockers & Risks

- QEMU `sifive_e` PT_LOAD RX vs RW (native `fence.i` gated) — tracked in `docs/technical/UPGRADE.md` Step 4, mitigation `objcopy --set-section-flags` pending Phase 0.5
- Host `thumbv7em`/`riscv32` targets require `rustup target add` (one-time, done)
- `x86_64-hros-none` / `riscv64-hros-none` are stretch — `holy-rust` crate currently `arm`/`riscv32` only

## Links

- Blueprint: `docs/production/HR-OS_PRODUCTION_BLUEPRINT.md`
- Technical Specs: `docs/technical/AXIS-*.md`, `BENCHMARK.md`, `WCEF.md`, `ZERO-COPY.md`, `INVALID-OP-CODES.md`, `E2E-SYSTEM-TRACE.md`
- Code Reference: `holy-rust/` workspace (reference impl) + `crates/hros-hal` (HAL traits proven `cargo check` pass)
- CI: `.github/workflows/ci.yml` (copied, needs expect harness)
