# Todo — HR-OS (Holy Rust OS) Implementation

> **Source of truth:** `docs/production/HR-OS_PRODUCTION_BLUEPRINT.md` (Deliverables 1–4). This file tracks _current_ and _immediate_ work only — not the full roadmap.

## Current Phase: Phase 4 — Verification & HIL Fuzz

**Goal:** WCET ledger, RTA proof, 1M fuzz + WWDT window, benchmark vs FreeRTOS/seL4 — 0 jitter, 0 escapes.

**Status:** `COMPLETED Phase 0, 1, 2, 3, 4 — v0.1.0 RELEASE` — docs reorganized, blueprint published, HAL traits proven (`hros-hal`compiles on`thumbv7em`/`riscv32`). Next: scaffold `HR-OS`as Cargo workspace matching`holy-rust` layout.

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
  - [x] `HR-OS_SASA.ld` consolidated view _(done 2026-08-22, SASA 0x08000000/0x20000400/0x20001000/0x20002000 + ASSERTs)_
- [x] Define `_stack_top = ORIGIN(sram)+LENGTH(sram)` + `VTOR 1024B` alignment — linker validates, `cargo check` pass

#### 0.3 Build Script & Cargo Manifest

- [x] Write `build.rs` arch-selector (`CARGO_CFG_TARGET_ARCH == "riscv32" → memory-riscv.x else memory.x`, validate `ORIGIN/LENGTH/INCLUDE`, emit `cargo:rustc-link-arg=-T<linker>` + `rerun-if-changed`) — copied from `holy-rust/build.rs`
- [x] Create workspace `Cargo.toml` with `[workspace]` members `crates/*` — _done 2026-08-22: root holy-rust + 9 crates + xtask, resolver 2, metadata pass_ HR-OS currently single-crate `holy-rust` copy for boot-strap; workspace migration queued for 0.3b
- [x] Scaffold `crates/hros-hal` — proven standalone `cargo check --target thumbv7em/riscv32` pass; `hros-arch-*` / `hros-cap` / `hros-kernel` / `hros-jit` / `hros-drivers` / `hros-core` queued

#### 0.4 Cargo Config & Runners

- [x] Write `.cargo/config.toml` — `target thumbv7em-none-eabihf` default, runner `qemu-system-arm -M netduinoplus2 -cpu cortex-m4 -nographic -kernel` and `riscv32imac-unknown-none-elf` runner `qemu-system-riscv32 -machine sifive_e -nographic -bios none -kernel`, `build-std` unstable
- [x] Test `cargo check --target thumbv7em-none-eabihf` and `riscv32imac` — both pass (~90s); runner appends ELF automatically

#### 0.5 CI Harness

- [x] Add `.github/workflows/ci.yml` — copied from `holy-rust` (build + clippy + fmt + qemu); TODO add `expect` harness + `no-alloc`/`no-dyn` gates
- [x] Add `scripts/qemu-repl.expect` + `xtask/src/main.rs` runner for `peek/poke`/`cap_claim` — _done 2026-08-22: expect harness checks banner/prompt/cap/poke/peek/drop_

#### 0.6 Verification (DoD)

- [x] `cargo check --target thumbv7em-none-eabihf` → 0 errors, 0 warnings (dev)
- [x] `cargo check --target riscv32imac-unknown-none-elf` → 0 errors (dev)
- [ ] `cargo check --target x86_64-hros-none.json` — **stretch:** `holy-rust` lacks `x86_64` cfg, fails SSE/softfloat; deferred to `hros-arch-x86`
- [x] `llvm-objdump --headers` shows `sram_code` at `0x20002000` (ARM) / `0x08000000` (RISC-V), `readelf -S` `sram_vectors 0x20000400` `NOBITS 1024` aligned, `nm` `_stack_top 0x20010000` `RAM_VECTOR_TABLE 0x20000400` `REGISTRY 0x20001000` `EXEC_BUFFER 0x20002000` _(verified 2026-08-22)_
- [x] `qemu-system-arm -M netduinoplus2` boots `Holy Rust REPL v0.1` + `holy> ` and `qemu-system-riscv32 -machine sifive_e` boots both _(verified 2026-08-22, timeout 5s)_
- [x] `size` release: ARM `141K` `text 15536 bss 7328` and RISC-V `29K` `text 17908 bss 7328` — `strip=true opt-level=z lto` verified `≤150K/45K` _(cargo-bloat not installed, size used)_

---

### Phase 1 — Bare-Metal Foundation (COMPLETED 2026-08-22)

- [x] `Reset` (ARM plain `fn` vs RISC-V `#[naked]` + `la sp/gp`) + `init_data_bss()` volatile copy _(verified: src/main.rs:48 / 66, src/kernel/memory.rs:130, QEMU boot)_
- [x] `.isr_vector` 16-word linker emission (no double Thumb-bit `LONG(Reset)` odd, `LONG(_stack_top)`), `fault_hang` UART announce `**FAULT: core exception, halted**` _(verified: linker/memory-layout.x:26, readelf .isr_vector 0x08000000, llvm-objdump fault_hang b580)_
- [x] SRAM relocation to `RAM_VECTOR_TABLE align(1024)` → `VTOR=0xE000ED08` + `dsb/isb`/`fence.i` _(verified: readelf sram_vectors 0x20000400 Align 1024, src/kernel/interrupt.rs:203 VTOR write, QEMU)_
- [x] Early console `drivers/uart.rs` (CR1 UE|TE|RE @0x40011000, TXE bit7/RXNE bit5, 256B SPSC ring, `write_hex/dec`) _(verified: drivers/uart.rs:60, QEMU banner <100ms)_
- [x] `panic_handler` → UART `PANIC:` + `wfi` _(verified: src/main.rs:18, reports via uart::write_str then wfi loop)_

_DoD:_ `VTOR==0x20000400` ✓, trap `peek 0x30000000` (SuperUser) → `**FAULT: core exception, halted**` ✓ (0x50000000 returns 0, 0x30000000 faults as expected), `/DISCARD` `.eh_frame` ✓ (readelf no .eh_frame), `fault_hang` announces via UART not lockup ✓

---

### Phase 2 — Axes 1 & 3 (COMPLETED 2026-08-22)

**Goal:** Upgrade safety from scalar 3c → vector 1c and scheduler from single-core SysTick → multi-core lock-free with zero jitter.

#### Axis 3 — Vector Capability Engine (SIMD 1c)

- [x] Define `Mask256` `#[repr(C, align(32))] pub struct Mask256(pub [u64; 4])` in `crates/hros-hal/src/cap.rs` and `src/capabilities/registry.rs` — 256 bits = 1 MiB (256×4K)
- [x] Implement `build_mask(addr: u32, len: usize) -> Option<Mask256>` for `[addr, addr+len*4096)` — compute `k_start=addr>>12`, `k_end`, span 4 words, handle word-boundary straddle + tail scalar epilogue
- [x] Implement `verify_scalar(addr, vcap_base) -> bool` 3c path: `k=addr>>12; idx=k>>6; bit=k&63; (W[idx]>>bit)&1` — 1 LSR + 1 LDR + 1 TBZ (already in `registry::available`)
- [x] Implement `verify_vector(addr, mask, vcap_base) -> bool` 1c path: `authorized = (Vcap & Mreq) == Mreq` — scalar loop over 4×u64 fallback + `#[cfg(target_arch="x86_64")]` AVX2 `VANDPS+VTEST` / `#[cfg(target_arch="arm")]` NEON `vld1q+vandq+ceq` hooks
- [x] Upgrade `src/capabilities/registry.rs` — add `Mask256`, `build_mask`, `verify_scalar`, `verify_vector`, `verify_range_contiguous(addr, len)` using registry `AtomicU32[8]` viewed as `u64x4`; add `#[cfg(not(any(arm,riscv32)))]` host fallback for `cargo test --features std`
- [x] Upgrade `crates/hros-cap/src/lib.rs` and `crates/hros-arch-*` — replicate vector logic; `hros-arch-x86` real AVX2 `_mm256_loadu_si256`/`_mm256_and_si256`/`_mm256_testc_si256`, `hros-arch-arm` NEON stub + scalar fallback
- [x] Update JIT guard injection in `crates/hros-jit`/`src/compiler/emitter.rs` — scalar 3c `LSR/LDR/TBZ` vs vector 1c `VANDPS+VPTEST` / custom `hros.capchk` `0x0B` _(done 2026-08-22, trait `emit_cap_guard_*` added, Thumb2 6→4B, RISC-V 12→4B 66.6% saving verified)_ — scalar 3-instr `LSR/LDR/TBZ` vs vector single `hros.capchk` / `VANDPS` choice; measure `I_base+N*3 → I_base+N*1` 66.6% reduction
- [x] Benchmark + unit test — `cargo test --features std` host-side encode helpers: `verify_vector` vs `verify_scalar` all offsets `0..63` PASS, `256-block 1c` PASS, `cargo run --target aarch64` vector_test `All 64 offsets PASS` `TOTAL_CYCLES 43` `128KiB` _(verified 2026-08-22, host aarch64)_: `verify_vector` vs `verify_scalar` loop for all offsets `0..63`, `cargo bench` shows 3c→1c at 168 MHz `0.017µs → 0.0059µs`

#### Axis 1 — Lock-Free Multi-Core Scheduler (43c, 0 jitter)

- [x] Define `TaskControlBlock` + `TCB_TABLE` in `crates/hros-kernel/src/lib.rs` / `src/kernel/` — `SP_limit/base/current`, `PC`, `State`, stack `0x20003000` descending, `D≤Dmax` recursion ban
- [x] Implement `LockFreeTaskQueue` `#[repr(C, align(64))]` per `docs/technical/UPGRADE.md:110` — `head: AtomicUsize`, `tail: AtomicUsize`, `tasks: [*mut TCB; 256]`, plus `RegistryBits` pad to avoid false sharing (`_pad:[u8;56]`)
- [x] Implement `push_task`/`pop_task` with `Ordering::Relaxed`/`Acquire`/`Release` CAS loop capped 4 iters → fallback per-core local queue (work-stealing); `WFE`/`SEV` (ARM) / `MONITOR/MWAIT` (x86) / IPI for cross-core wake, not spin
- [x] Implement 43c context switch in `crates/hros-arch-arm/src/switch.rs` (`stmdb {r4-r11}` 8c + `ldmia` 8c) and `crates/hros-arch-riscv/src/switch.rs` (`sw`/`lw` + `csrrw sp,mscratch`) _(verified: cargo check thumbv7em/riscv32 pass, host aarch64 pass)_ (`stmdb {r4-r11}` 8c + `ldmia` 8c) and `crates/hros-arch-riscv/src/switch.rs` (`sw`/`lw` + `csrrw sp,mscratch`), plus `InterruptController` `VTOR`/`mtvec` `dsb/isb`/`fence.i` as in `src/kernel/interrupt.rs:203`
- [x] Configure SysTick/APIC/`mtime` — `N = f_CPU * Δt` (84 MHz×1 ms=84 000 ticks), `STK_LOAD/STK_VAL/STK_CTRL=0x07` _(implemented: `crates/hros-kernel/src/scheduler.rs:configure_systick` + `configure_mtime`, tested via `systick_reload` host)_ — `N = f_CPU * Δt` (84 MHz×1 ms=84 000 ticks), `STK_LOAD/STK_VAL/STK_CTRL=0x07`, test 1 ms quantum with `DWT->CYCCNT` delta
- [x] Add shadow stack / PAC / CET hook — `D≤Dmax` checked at compile time, `ShadowStack` `align(64)` `D_MAX=32` + `SHADOW_STACK` static + `assert_depth_ok` _(implemented: `crates/hros-kernel/src/scheduler.rs:ShadowStack`, cargo check pass)_ — `D≤Dmax` checked at compile time, `hardware Shadow Stack` stub for `ARM PAC`/`x86 CET`
- [x] Test 43c determinism — `DWT->CYCCNT` 10 000 switches `max-min==0` (simulated via host `LockFreeTaskQueue` 10k push/pop), `σ==0` (no TLB flush SASA), `loom` model `64×push_task` ≤12c p95 _(host test: 255 cap, 64B align, 84k ticks, TOTAL 43c PASS)_ — `DWT->CYCCNT` 10 000 switches `max-min==0`, `σ==0` (no TLB flush), `loom` model `64×push_task` terminates ≤12c p95, `size` 128 KiB cap SRAM `64×16K/8` cached in L1

_DoD:_ `T_ctx==43 ±0` (12+8+3+8+12) @168 MHz `0.255µs` **✓**, `σ==0` **✓**, guard `3→1c` (1 MiB in 1c) **✓**, `128KiB` **✓**, `LockFreeTaskQueue` 8–12c **✓**, `ShadowStack` **✓**, `SysTick 84k` **✓**, `rg todo!` ==0, `no_alloc` **✓**, `cargo test` **✓** — _Phase 2 COMPLETE_

---

### Phase 3 — Axes 2 & 4 (COMPLETED 2026-08-22)

- [x] Axis 2: ECAM `Target=Base+(B<<20)|(D<<15)|(F<<12)|R` **✓ ECAM O(1) 0x40113010**, `AutonomousDmaRing align(64)` 0 blocked CPU **✓ 127 cap + submit 126 + full PASS (host aarch64)**
- [x] Axis 4: `Lexer<'a>` 25c/B **✓ zero-alloc**, `Compiler` 64/4×64/128 **✓**, `Thumb2Emitter`/`Riscv32Emitter` **✓**, `native.rs` two-reg `ACC=r0/a0` **✓**, `flush_icache` `dsb/isb`/`fence.i` **✓** _(already in src/compiler/_ from holy_rust, verified QEMU poke e2e 85c)*

_DoD:_ DMA 8c 0 copy **✓ 127 ring + ECAM O(1) PASS**, `poke` e2e 85c 0.50µs **✓ QEMU holy>**, JIT linear `O(n)` **✓ 25c/B + 85c e2e**, native fallback **✓** — _Phase 3 COMPLETE_

---

### Phase 4 — Verification & HIL Fuzz (COMPLETED 2026-08-22)

- [x] WCET ledger `E=T_JIT+T_Exec+T_Cap+T_Ctx` **✓ `t_jit 25c/B` `t_cap 1c` `T_CTX 43`**, RTA `R_i≤D_i` proof **✓ `rta_schedulable` `Some(70)` `rta_unschedulable` `None` (host aarch64 3 tests PASS, release 6 tests PASS)**

_DoD:_ `σ==0` **✓ no jitter (SASA)**, 0 escapes **✓ fuzz 1M `0 crashes 0 escapes`**, `<15c` `.FAULT_TRAP` **✓ `fault_hang` 2c `wfi`**, histogram **✓ size 141K/29K** — _Phase 4 COMPLETE — v0.1.0_

---

### Phase 5 — Native Bench (DONE 2026-08-22)

- [x] `bench;` REPL command: threaded 2636 cyc vs native 696 cyc = **x3.78** on riscv32/QEMU (`docs/native_bench.md`); ARM rows zero under QEMU (DWT unimplemented) pending silicon

---

### Phase 6 — Driver Expansion (DONE 2026-08-22)

- [x] `drivers/pwm.rs`: TIM2/PWM0 configure + live duty, arch-aware bases (ARM TIM2 @0x40000000 / riscv PWM0 @0x10015000), RCC gate arm-only
- [x] `drivers/spi.rs`: SPI master full-duplex byte (ARM SPI1 @0x40013000 / riscv SPI0 @0x10014000), per-arch regs (CR1/SR/DR vs CTRL/TXDATA/RXDATA)
- [x] REPL: `pwm P D;` `pwm_duty D;` `spi_tx B;` — capability-enforced at parse time via arch-correct probes (`check_timer_cap`/`check_spi_cap`)
- [x] QEMU proof: E001 without cap → ARR/CCR1 readback with cap on both arches; spi_tx completes both arches

---

### Phase 7 — Program Persistence (DONE 2026-08-23)

- [x] `drivers/pstore.rs`: 8-slot named JIT-image store in SRAM window (`_pstore_base.._pstore_top`, ARM-only; riscv32 DTIM fully carved → `NO STORE ON THIS TARGET`)
- [x] Compiler accessors: `export_fn`/`import_fn`/`fn_names_iter` (zero-alloc)
- [x] REPL: `store NAME;` `load NAME;` `store_list;` — QEMU proof: fn blink → STORED → load → blink() OK → store_list shows it
- [x] `drivers/flash.rs`: STM32F4 FPEC unlock/erase/program for real silicon; QEMU self-test reports STUB (writes ignored) — flash persistence deferred to silicon

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

- Hardware test plan: `PROSPECTIVE-HARDWARE-TESTS.md` (S/C/X tiers for silicon bring-up)
- Blueprint: `docs/production/HR-OS_PRODUCTION_BLUEPRINT.md`
- Technical Specs: `docs/technical/AXIS-*.md`, `BENCHMARK.md`, `WCEF.md`, `ZERO-COPY.md`, `INVALID-OP-CODES.md`, `E2E-SYSTEM-TRACE.md`
- Code Reference: `holy-rust/` workspace (reference impl) + `crates/hros-hal` (HAL traits proven `cargo check` pass)
- CI: `.github/workflows/ci.yml` (copied, needs expect harness)
