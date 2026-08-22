# Worst-Case Execution & Formal Schedulability (WCEF)

The Holy Rust Unikernel Operating System (HR-OS) achieves Deterministic Hard Real-Time Worst-Case Execution Time (WCET) bounds by eliminating the nondeterministic hardware and software primitives present in traditional operating systems.

HR-OS removes dynamic hardware paging (no TLB misses or page faults), dynamic memory allocators (no heap fragmentation or garbage collection), and multi-pass compilation (no unbounded optimizer loops). Every task's execution profile is strictly predictable at compile time.

---

## 1. The WCET Formulations Engine

For any active task `Ti` executing inside `EXEC_BUFFER`, the total execution time `E(Ti)` is bounded by four deterministic components:

```text
E(Ti) = T_JIT(S) + T_Exec(P) + T_Cap + T_Ctx
```

Where:

- `T_JIT(S)` is the Axis 4 single-pass JIT compilation overhead for an ASCII input stream of `S` bytes.
- `T_Exec(P)` is the native instruction execution time of the compiled program loop path `P`.
- `T_Cap` is the total accumulated time spent executing inline Axis 3 capability guards.
- `T_Ctx` is the Axis 1 hardware-synchronized context switching penalty.

---

## 2. Derivation of Individual WCET Component Bounds

### A. JIT Translation Bound (T_JIT)

Because Axis 4 enforces a strict LL(1) non-backtracking grammar with O(1) lookahead, processing each input byte `a ∈ S` requires a constant maximum number of CPU cycles `C_lexer`:

```text
T_JIT(S) = S × C_lexer
```

Where `C_lexer ≤ 25 cycles` per byte. For a maximum allowed command stream of 256 bytes on a 168 MHz core:

```text
T_JIT(256) ≤ 256 × 25 = 6,400 cycles  ≈ 38.0 µs
```

### B. Inline Capability Verification Bound (T_Cap)

Every volatile `peek`/`poke` or array index access has a JIT-injected O(1) Axis 3 bitwise safety guard block. As proven in Axis 3, every guard executes in exactly 3 instructions (`C_guard = 3 cycles` assuming 1 cycle/instruction execution on an in-order pipeline):

```text
T_Cap = N_accesses × C_guard     where  C_guard = 3 cycles
```

Because memory access counts `N_accesses` are statically bounded by loop iteration counts during Axis 4 synthesis, `T_Cap` is completely deterministic.

### C. Context Switch Bound (T_Ctx)

Because HR-OS operates in a Single Address Space Architecture (SASA) at Ring 0, context switches do not flush MMU TLBs or invalidate page tables.

On ARM Cortex-M4/M7 / AArch64 hardware:

- `C_auto_stack = 12 cycles` (Hardware register stacking)
- `C_reg_save + C_reg_restore = 16 cycles` (Push/Pop `R4–R11`)
- `C_sched = 15 cycles` (Axis 1 priority table lookup)

```text
T_Ctx = 43 cycles  ≈ 0.255 µs @ 168 MHz
```

---

## 3. Loop Bounding & Structural Static Analysis

To prevent unbounded execution via infinite loops or deep recursion, Axis 4 imposes strict language-level guarantees before code generation:

- **Unbounded Loops are Forbidden:** The grammar rejects arbitrary `while(true)` constructs unless explicitly tagged with an absolute cycle counter:

```rust
loop 1000 { poke 0x40021018 1 }   // Bounded: 1000 iterations, statically verified
```

- **Recursion is BANNED:** Function calls cannot form cycles in the call graph. The call depth `D` is statically checked (`D ≤ D_max`), guaranteeing stack overflow is mathematically impossible within allocated task SRAM.
- **Total Native Execution Bound (T_Exec):**

Using the Control Flow Graph (CFG) generated during the single-pass scan, the maximum execution path cost is:

```text
T_Exec(P) = Σ (basic_block_cost × iteration_bound)
```

---

## 4. Schedulability Proof via Response Time Analysis (RTA)

For a set of `n` periodic tasks `T = {T₁, T₂, …, Tₙ}` ordered by Axis 1 priorities (where `T₁` is highest priority), the worst-case response time `Rᵢ` of task `Tᵢ` is calculated using the iterative Response Time Analysis equation:

```text
Rᵢ⁽ᵏ⁺¹⁾ = W_Ti + Σ_{j < i}  ceil(Rᵢ⁽ᵏ⁾ / Pⱼ) × W_Tj
```

Where:

- `W_Ti` is the isolated worst-case execution time of task `Tᵢ`.
- `Pⱼ` is the period of higher-priority task `Tⱼ`.
- `ceil(Rᵢ⁽ᵏ⁾ / Pⱼ) × W_Tj` is the preemptive interference caused by higher-priority task `Tⱼ`.

### Hard Real-Time Determinism Theorem

> A task set `T` running on HR-OS is mathematically guaranteed to meet all real-time deadlines without deadline overrun if and only if:
>
> ```text
> ∀ i :  Rᵢ ≤ Dᵢ
> ```
>
> Where `Dᵢ` is the hard deadline of task `Tᵢ`. Because `W_Ti` contains zero page-fault jitter, zero TLB flush jitter, and bounded JIT translation bounds, `Rᵢ` converges deterministically in finite steps.

---

## 5. Predictability Comparison: Traditional OS vs. HR-OS

| WCET Determinism Parameter | Traditional Linux / RTOS               | HR-OS O(1) Unikernel                        |
| -------------------------- | -------------------------------------- | ------------------------------------------- |
| Page Fault Delay           | 1,000–10,000 cycles (Nondeterministic) | 0 cycles (No Virtual Memory/MMU)            |
| TLB Miss Penalty           | 10–100 cycles (Hardware Walk)          | 0 cycles (Single Address Space)             |
| Dynamic Memory Allocation  | Unbounded (malloc heap searching)      | 0 cycles (Static SRAM Block Allocation)     |
| Context Switch Jitter      | 2.0–15.0 µs                            | Deterministic 0.255 µs (43 cycles)          |
| Parsing & Execution Jitter | Variable JIT / Interpreter overhead    | Bounded O(n) Single-Pass Assembly Synthesis |
