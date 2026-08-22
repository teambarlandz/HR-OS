# Axis 3: Mathematical Proofs & Bitwise Logic for O(1) Capability Safety

Axis 3 establishes how the Holy Rust Unikernel Operating System (HR-OS) guarantees safety without relying on hardware-based Virtual Memory Isolation (MMU Page Tables)[span_3](start_span)[span_3](end_span).

In conventional operating systems, memory protection is enforced by physical MMU hardware page tables (`PML4 → PDPT → PD → PT`) and privilege rings (Ring 3 vs. Ring 0)[span_4](start_span)[span_4](end_span). Every memory access undergoes page-table walks, TLB checks, and privilege validations — costing latency and unpredictable page fault delays[span_5](start_span)[span_5](end_span).

HR-OS operates in a Single Address Space Architecture (SASA) at Ring 0 / EL1[span_6](start_span)[span_6](end_span). Safety is guaranteed purely through **Linear Capability Graph Theory**, **Software Fault Isolation (SFI)**, and **O(1) Bitwise Capability Register Proofs**[span_7](start_span)[span_7](end_span).

---

## 1. Formal Mathematical Foundations of the Capability Model

### A. The System Universe Definition

Let `U` be the set of all physical resources present on the motherboard[span_8](start_span)[span_8](end_span):

```text
U = M ∪ P ∪ E

Where:
 * M = {m₀, m₁, …, m_K−1} is the set of K physical memory blocks (RAM regions).
 * P = {p₀, p₁, …, p_L−1} is the set of L Memory-Mapped I/O peripheral registers (PCIe BARs, UART, PMIC, Timers).
 * E = {e₀, e₁, …, e_Q−1} is the set of Q executable routines in EXEC_BUFFER.
The total number of protected system hardware capabilities is N = |U| = K + L + Q.
B. The Capability Identity Function
Every resource r ∈ U maps deterministically to a unique integer capability identifier CapId ∈ [0, N−1] via a spatial hash function H: 𝔸 → ℤ_N, where 𝔸 is the 64-bit physical address space:
CapId = H(address) = address >> M   (where M = 12 for 4KB granularity)

Because hardware regions (DRAM blocks, MMIO pages) are non-overlapping physical address ranges, H is injective.
C. Capability Access Token Vector (C)
Let a Task Ti possess a Capability State represented as a dense bitfield vector C_Ti of length N bits:
C_Ti ∈ {0,1}ᴺ   where  C_Ti[k] = 1 ⇔ Task Ti is authorized for resource k

Where each bit position corresponds to a distinct CapId.
2. Mathematical Proof of O(1) Time Complexity & Dual-Path Verification Safety
Theorem — Deterministic O(1) / Bounded O(4) Safety Verification
> Safety authorization for any arbitrary physical memory address a ∈ 𝔸 in HR-OS executes in deterministic O(1) worst-case time complexity (or strictly bounded O(4) on non-SIMD soft-float bare-metal targets), requiring at most 3 to 12 CPU instructions, independent of system scale, memory size, or total number of active tasks.
> 
Proof:
 * Address Partitioning: The 64-bit physical address space 𝔸 is partitioned into uniform capability granularity blocks of size S = 2ᴹ bytes (e.g., M = 12 ⇒ S = 4096 Bytes).
 * Index Extraction: For any physical target address a, the capability vector index k is extracted using a binary bit-shift:
k = a >> M

This operation executes in 1 clock cycle via a single hardware logical right shift (LSR) instruction.
 * Bitfield Word & Mask Computation: The bitfield vector C is packed as an array of 64-bit integers W = [W₀, W₁, …, W_{⌈N/64⌉ − 1}].
   * The Word Index I = k >> 6 (Word location in memory)
   * The Bit Offset b = k & 63 (Bit position inside the 64-bit word)
 * Boolean Decision Predicate Evaluation:
Path 1: Primary SIMD Hardware Vector Verification (AVX2 / ARM NEON)
When target architectures support 256-bit SIMD registers, capability bitmasks are evaluated simultaneously across a 256-bit window (V_{\text{cap}} \ \& \ M_{\text{req}} == M_{\text{req}}):
P_vec(a, C) = _mm256_testc_si256( _mm256_and_si256(V_cap, M_req), M_req )

 * Instruction Count: 1 Load + 1 VANDPS + 1 VPTEST = 3 instructions / 1 clock cycle (\mathcal{O}(1)).
Path 2: Bounded Scalar Fallback Verification (Soft-Float / No-SIMD Targets)
For bare-metal non-vector targets (such as x86_64-hros-none with disabled vector registers or soft-float targets), evaluation executes across a fixed array of four 64-bit scalar words (W_0, W_1, W_2, W_3):
P_scalar(a, C) = (v[0] & m[0] == m[0]) ∧ (v[1] & m[1] == m[1]) ∧ (v[2] & m[2] == m[2]) ∧ (v[3] & m[3] == m[3])

 * Instruction Count: 4 Bitwise ANDs + 4 Equality Comparisons across a statically bounded 4-word loop.
Since the execution trace consists of a fixed, bounded sequence of instructions without dynamic table traversals or unbounded loops:
∴  T(P) = O(1) [SIMD Hardware]   or   T(P) = O(4) [Scalar Bounded Fallback]

3. Physical SRAM Memory Layout of the Capability Matrix
The Capability Registry resides in an isolated, high-speed SRAM region (0x0000_0000_0080_0000). It is organized as a two-dimensional bit-matrix where rows represent Tasks and columns represent Capability Bits.
 0x0000_0000_0080_0000 (Capability SRAM Base Address)
┌────────────────────────────────────────────────────────────────────────────────────────┐
│ TASK 0 CAPABILITY VECTOR (Words 0 .. K)                                                │
│ [ 0x8000_0000_0000_0001 ] [ 0x0000_0000_0000_00FF ] ... [ 0x0000_0000_0000_0000 ]        │
├────────────────────────────────────────────────────────────────────────────────────────┤
│ TASK 1 CAPABILITY VECTOR (Words 0 .. K)                                                │
│ [ 0x0000_0000_0000_0003 ] [ 0x0000_0000_0000_0000 ] ... [ 0xFF00_0000_0000_0000 ]        │
├────────────────────────────────────────────────────────────────────────────────────────┤
│ TASK N CAPABILITY VECTOR (Words 0 .. K)                                                │
└────────────────────────────────────────────────────────────────────────────────────────┘

Memory Compactness Calculation
For a system supporting up to 16,384 distinct 4 KB memory/MMIO regions (64 MB address space granularity per capability bit) and 64 active tasks:
Memory = 64 tasks × (16,384 bits / 8) = 64 × 2048 bytes = 128 KB

This entire safety structure fits directly within the CPU's high-speed L1/L2 cache or internal SRAM, guaranteeing zero DRAM bus access penalties during safety validation.
4. Hardware Bitwise Guard Logic & Software Fault Isolation (SFI)
When Axis 4's JIT engine compiles incoming text streams into raw Thumb-2, ARM64, or x86_64 opcodes, it injects an inline O(1) capability guard right before emitting any volatile peek (Load) or poke (Store) instruction.
The Guard Assembly Injection Pattern (ARM64 / AArch64)
  Raw Unchecked Target Instruction:
  STR X1, [X0]    ; Store value X1 into physical address X0

  HR-OS JIT Injected Safety Guard Block:

  ; --- STEP 1: Compute Capability Bit Index ---
  LSR X2, X0, #12         ; X2 = Address >> 12 (Extract 4KB Block ID)
  LSR X3, X2, #6          ; X3 = X2 / 64 (Compute 64-bit Word Offset)
  AND X4, X2, #63         ; X4 = X2 % 64 (Compute Bit Shift Offset)

  ; --- STEP 2: Fetch Task Capability Word from SRAM ---
  ; X21 holds the base physical pointer to Current_Task.Capability_Vector
  LDR X5, [X21, X3, LSL #3] ; X5 = Capability_Vector[Word_Offset]

  ; --- STEP 3: Bitwise Mask Verification ---
  LSR X5, X5, X4          ; Shift target bit to Bit 0 position
  TBZ X5, #0, .FAULT_TRAP ; Test Bit 0: If Zero (Unauthorized), branch instantly to HARD_FAULT

  ; --- STEP 4: Executed Only If Verified Authorized ---
  STR X1, [X0]            ; Physical Memory Write Executed Safely

Execution Flow Matrix
  [ Incoming Write Request (Addr) ]
                 │
                 ▼
  [ Compute Word Index & Bit Shift ]
                 │
                 ▼
  [ Read 64-bit Bitmask Word from SRAM ]
                 │
                 ▼
      [ Test Bit 0 (Is Granted?) ]
         /                  \
   Bit == 1 (Valid)    Bit == 0 (Forbidden)
       /                      \
  [ Execute STR/LDR ]    [ Branch to .FAULT_TRAP ]
                                  │
                                  ▼
                         [ Freeze Task Context ]
                         [ Revoke Execution ]

5. Linear Capability State Transitions (Algebraic Operations)
To prevent privilege escalation without complex security policies, HR-OS models capabilities using Linear Capability Logic. A capability token cannot be duplicated arbitrarily — it can only be Granted, Split, or Revoked via explicit atomic operations.
                [ System Super-Kernel (Task 0) ]
                               │
                       Grant Capability
                               │
                               ▼
    ┌─────────────────────────────────────────────────────┐
    │  Task A Capability Vector                           │
    │  [ Bit 14 = 1 ] (Owns MMIO Region 0x4000_E000)      │
    └──────────────────────────┬──────────────────────────┘
                               │
                       Transfer / Delegate
                               │
                               ▼
    ┌─────────────────────────────────────────────────────┐
    │  Task B Capability Vector                           │
    │  [ Bit 14 = 1 ] (Gained Access)                     │
    └─────────────────────────────────────────────────────┘

A. Atomic Capability Grant (⊕)
When the kernel assigns a new hardware block to a task:
C_Ti' = C_Ti ⊕ M_grant   =  C_Ti OR M_grant

Where M_grant is a bitmask vector containing 1s only at the granted capability indices.
B. Atomic Capability Revocation (⊖)
When a resource is reclaimed or a process terminates:
C_Ti' = C_Ti ⊖ M_revoke  =  C_Ti AND (NOT M_revoke)

This operation sets target capability bits back to 0 in a single clock cycle using an atomic bitwise BIC (Bit Clear) instruction.
C. Capability Transitive Delegation Proof
Let T₀ be the root kernel task possessing full authority C_T₀ = 1ᴺ.
Any delegated capability state C_Tk for an unprivileged task satisfies the subset constraint:
C_Tk ⊆ C_T₀

This guarantees mathematically that no task can ever acquire authority over a hardware block that was not explicitly derived from the root kernel's initial allocation matrix.
6. Architectural Overhead Comparison
| Safety Metric | Standard OS (Hardware MMU Page Tables) | HR-OS O(1) Bitfield Capability Matrix |
|---|---|---|
| Safety Mechanism | Dynamic Page Translation (VA → PA) | Software Guard Injections + Bitfield Mask |
| Verification Delay | 10 to 100 Cycles (Page Table Walk / TLB Miss) | Deterministic 3–5 Cycles (1c SIMD / Bounded 4-Word) |
| Memory Footprint | Gigabytes (PML4 / PDPT / PD / PT tables) | 128 KB SRAM total for full matrix |
| Page Fault Overhead | 1,000–10,000 Cycles (Interrupt + OS Handler) | Zero (Failures trap instantly at compile/guard time) |
| Isolation Boundary | Hardware Rings (Ring 3 vs Ring 0) | Linear Software Token Bitmask (Pure Ring 0) |

