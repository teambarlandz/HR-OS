# Todo — HR-OS (Holy Rust OS) Implementation

> **Source of truth:** `docs/production/HR-OS_PRODUCTION_BLUEPRINT.md` (Deliverables 1–4). This file tracks _current_ and _immediate_ work only — not the full roadmap.

## Current Phase: Phase 0 — Toolchain, Linker, Custom Targets & QEMU HIL Harness

**Goal:** Reproducible cross-build from `x86_64-unknown-linux` host to all HR-OS SASA targets without host linker contamination. This phase blocks all later work.

**Status:** `IN PROGRESS` — docs reorganized, blueprint published, HAL traits proven (`hros-hal` compiles on `thumbv7em`/`riscv32`). Next: scaffold `HR-OS` as Cargo workspace matching `holy-rust` layout.

---

### Immediate Tasks (Next 7 Days)

#### 0.1 Toolchain Pinning

- [ ] Add `rust-toolchain.toml` at project root: `channel="nightly"` + `components=["rust-src","llvm-tools-preview","rustfmt","clippy"]`
- [ ] Verify `rustc 1.97+` + `cargo 1.97+` present; `rustup target add thumbv7em-none-eabihf riscv32imac-unknown-none-elf` succeeds
- [ ] Add custom targets `targets/x86_64-hros-none.json` and `targets/riscv64-hros-none.json` (already prototyped in `holy-rust/targets/`) — validate `disable-redzone:true`, `panic=abort`, `features:-mmx,-sse,+soft-float`

#### 0.2 Linker Family & SASA Contract

- [ ] Create `linker/` directory with 5 files:
  - [ ] `linker/memory.x` — ARM STM32F405: `flash 128K@0x08000000`, `sram 52K@0x20003000` + carved `vectors@0x20000400` / `registry@0x20001000` / `sram_code@0x20002000`
  - [ ] `linker/memory-riscv.x` — SiFive E310: `flash 0x20400000`, `DTIM 8K@0x80000000`, `ITIM 4K@0x08000000`
  - [ ] `linker/memory-layout.x` — shared `SECTIONS`: `.isr_vector` 16-word `LONG(_stack_top) LONG(Reset) LONG(fault_hang)×` + `KEEP(*(.isr_vector))` + `/DISCARD/`
  - [ ] `linker/memory-layout-riscv.x` — RISCV variant (no vector table, `Reset` first)
  - [ ] `linker/linker.ld` alias + `HR-OS_SASA.ld` consolidated view
- [ ] Define `_stack_top = ORIGIN(sram)+LENGTH(sram)`, `ASSERT` alignment (`VTOR 1024B`, `mtvec &!0x3`)

#### 0.3 Build Script & Cargo Manifest

- [ ] Write `build.rs` arch-selector: `CARGO_CFG_TARGET_ARCH == "riscv32" → memory-riscv.x else memory.x`, validate `ORIGIN/LENGTH/INCLUDE`, emit `cargo:rustc-link-arg=-T<linker>` + `rerun-if-changed`
- [ ] Create workspace `Cargo.toml` at root: `[workspace]` with members `crates/*`, `[profile.release] opt-level="z" lto=true codegen-units=1 panic="abort" strip=true`, `build-std=["core","compiler_builtins"]`
- [ ] Scaffold `crates/hros-hal` (already proven standalone) + `hros-arch-arm`, `hros-arch-riscv`, `hros-arch-x86`, `hros-cap`, `hros-kernel`, `hros-jit`, `hros-drivers`, `hros-core`, `xtask`

#### 0.4 Cargo Config & Runners

- [ ] Write `.cargo/config.toml`: `target thumbv7em-none-eabihf` default, runner `qemu-system-arm -M netduinoplus2 -cpu cortex-m4 -nographic -kernel` and `riscv32imac-unknown-none-elf` runner `qemu-system-riscv32 -machine sifive_e -nographic -bios none -kernel`, `build-std` unstable
- [ ] Test `cargo run --target thumbv7em-none-eabihf` appends ELF path automatically (cargo runner semantics)

#### 0.5 CI Harness

- [ ] Add `.github/workflows/ci.yml`: jobs `build (3 targets)`, `clippy -- -D warnings`, `fmt --check`, `qemu` (expect harness `holy> ` within 100ms), `no-alloc` grep, `no-dyn` grep, `undocumented_unsafe_blocks` deny
- [ ] Add `scripts/qemu-repl.expect` (or `xtask/src/main.rs` runner) for `peek/poke` roundtrip + `cap_claim` test

#### 0.6 Verification (DoD)

- [ ] `cargo build --target thumbv7em-none-eabihf --release` → 0 errors, 0 clippy warnings
- [ ] `cargo build --target riscv32imac-unknown-none-elf --release` → 0 errors
- [ ] `cargo build --target targets/x86_64-hros-none.json --release` → 0 errors
- [ ] `llvm-objdump --headers` shows `sram_code RWE`, `nm` shows `RAM_VECTOR_TABLE @0x20000400` aligned, `_stack_top` computed
- [ ] `qemu-system-arm` boots banner `Holy Rust REPL v0.1` + `holy> ` in <100ms; SiFive boots via `0x20400000`
- [ ] `cargo bloat` shows `strip` ARM ≤150K, RISC-V ≤45K

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

- [ ] QEMU `sifive_e` PT_LOAD RX vs RW (native `fence.i` gated) — tracked in `docs/technical/UPGRADE.md` Step 4, mitigation `objcopy --set-section-flags` pending Phase 0.5
- [ ] No `gh` CLI on host — GitHub API via `curl` + SSH `git@github.com:teambarlandz/HR-OS.git` used instead
- [ ] Host `thumbv7em`/`riscv32` targets require `rustup target add` (one-time)

## Links

- Blueprint: `docs/production/HR-OS_PRODUCTION_BLUEPRINT.md`
- Technical Specs: `docs/technical/AXIS-*.md`, `BENCHMARK.md`, `WCEF.md`, `ZERO-COPY.md`, `INVALID-OP-CODES.md`, `E2E-SYSTEM-TRACE.md`
- Code Reference: `holy-rust/` workspace (reference impl) + `crates/hros-hal` (HAL traits proven)
- CI: `.github/workflows/ci.yml` (to be added Phase 0.5)
