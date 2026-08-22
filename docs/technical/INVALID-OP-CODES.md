# Invalid Opcode & Infinite Loop Handling

When unprivileged or JIT-synthesized code in `EXEC_BUFFER` attempts to execute corrupt machine code, an invalid opcode sequence, or falls into an infinite execution loop, HR-OS handles the failure entirely in hardware without virtual memory paging, guest operating system crashing, or kernel panic.

Protection and instant task termination are achieved via a dual-layer hardware trap architecture: **CPU Architectural Exception Handlers** (for invalid opcodes) and a **Dedicated Hardware Windowed Watchdog Timer (WDT)** (for execution loops).

---

## 1. Handling Invalid Opcodes: Hardware Vector Exception Traps

If Axis 4 emits malformed opcodes or execution jumps to an unaligned/unpopulated address in `EXEC_BUFFER`, the CPU hardware pipeline instantly aborts the instruction decode phase before execution completes.

```text
       EXEC_BUFFER Execution
                 │
  [ CPU Hardware Pipeline Decode ]
                 │
     Invalid / Undefined Opcode?
                 │
                 ├──► YES ──► [ CPU Hardware Hard-Fault Exception ]
                 │                    │
                 │            Vector Address Shift
                 │                    │
                 │                    ▼
                 │       [ .FAULT_TRAP Vector Handler ]
                 │                    │
                 │           1. Read Fault Register (CFSR/ESR)
                 │           2. Read Saved PC from Stack Frame
                 │           3. Is PC inside EXEC_BUFFER?
                 │                   /          \
                 │                YES            NO (Kernel Bug)
                 │                 /              \
                 │      [ Terminate Task ]    [ Panic System ]
                 │      [ Reclaim Memory ]
                 │      [ Clear Capability ]
```

### Subsystem Sequence for Invalid Opcodes

- **Hardware Exception Generation:** On ARM Cortex/AArch64 architectures, executing an illegal bit pattern raises a `UsageFault` or `UNDEFINED_INSTRUCTION` exception instantly (1 cycle). On x86_64, it triggers an Invalid Opcode `#UD` fault.
- **Deterministic Vector Jump:** The CPU immediately halts the execution pipeline, auto-stacks the current state (`xPSR`, `PC`, `LR`), and redirects the Program Counter (`PC`) to the fixed address of `.FAULT_TRAP` in the vector table.
- **Execution Context Inspection:** The `.FAULT_TRAP` handler inspects the System Control Block (e.g., Configurable Fault Status Register `CFSR` on ARM, or Exception Syndrome Register `ESR_EL1` on ARM64).

- **Isolated Task Eviction:**
  - If `Faulted_PC ∈ EXEC_BUFFER_RANGE`, the fault is isolated strictly to the user task.
  - HR-OS immediately zeroes out the task's O(1) Capability Vector in SRAM.
  - The task's stack and execution buffers are marked free in the allocation bitmap.
  - Control returns to the Axis 1 Kernel Scheduler to dispatch the next ready task.

---

## 2. Handling Infinite Loops: Hardware Windowed Watchdog (WDT)

Because HR-OS operates without MMU hardware slicing, an unprivileged program with a continuous loop `{ ... }` or infinite branch (`B .`) cannot be interrupted by software alone. HR-OS guarantees deterministic execution bounds using a hardware Windowed Watchdog Timer (WWDT) synchronized with Axis 1's temporal scheduler.

```text
  AXIS 1 Quantum Reset (e.g., 1ms Target)

  Clock Cycles ──► 0                     t_lower           t_upper
                   │                        │                 │
  Watchdog Window  │   FORBIDDEN REFRESH    │   VALID REFRESH │ FAULT (EXPIRED)
                   ├────────────────────────┴─────────────────┤
                                                              ▲
                                                              │
                                                   Loop Code Exceeds Bound!
                                                              │
                                                   [ Hardware NMI Interrupt ]
```

### The Dual-Bound Watchdog Mechanics

The hardware Watchdog Timer is configured with a strict lower bound (`t_lower`) and upper bound (`t_upper`):

- **Upper Bound Constraint (`t_upper`):** Maximum execution time allocated for an `EXEC_BUFFER` slice (e.g., `1.0 ms`). If code runs past `t_upper` without yielding or being preempted, the WDT hardware fires a Non-Maskable Interrupt (NMI).
- **Lower Bound Constraint (`t_lower`):** Code cannot reset ("feed") the watchdog before `t_lower` has elapsed. This prevents rogue or corrupted code in `EXEC_BUFFER` from continuously resetting the watchdog in a tight loop to hog the CPU.

---

## 3. The Recovery Flow: `.FAULT_TRAP` Execution Sequence

When either an Invalid Opcode exception or a Watchdog NMI fires, HR-OS runs the following bare-metal recovery routine in under 15 CPU cycles:

```asm
.global .FAULT_TRAP
.type .FAULT_TRAP, %function

.FAULT_TRAP:
    ; Step 1: Disable Task Interrupts, lock core
    CPSID   I

    ; Step 2: Read Task ID from Kernel SRAM Control Register
    LDR     R0, =CURRENT_TASK_ID
    LDR     R1, [R0]                  ; R1 = Current Task Index

    ; Step 3: Zero out Task's Axis 3 Capability Vector in SRAM
    LDR     R2, =CAPABILITY_SRAM_BASE
    ADD     R2, R2, R1, LSL #11       ; R2 = Base + (Task_ID * 2048 Bytes)
    MOV     R3, #0
    STR     R3, [R2]                  ; Atomic revocation of capability word 0

    ; Step 4: Reset EXEC_BUFFER Stack Pointer for faulted task
    LDR     R4, =TASK_STATE_TABLE
    STR     R3, [R4, R1, LSL #2]      ; Mark Task State = TASK_STATE_DEAD

    ; Step 5: Refresh Hardware Watchdog Timer to prevent board reset
    LDR     R5, =WWDT_REFRESH_REG
    LDR     R6, =WWDT_UNLOCK_KEY
    STR     R6, [R5]                  ; Feed WDT hardware

    ; Step 6: Re-enable Interrupts and Force Axis 1 Scheduler Jump
    CPSIE   I
    B       AXIS1_SCHEDULE_NEXT       ; Instantly switch to next healthy task
```

---

## 4. Hardware Watchdog vs. Traditional OS Trap Handling

| Recovery Dimension        | Traditional OS (POSIX / Virtual Memory)    | HR-OS Unikernel Watchdog Trap                |
| ------------------------- | ------------------------------------------ | -------------------------------------------- |
| Fault Detection Mechanism | MMU Page Fault / SIGILL / SIGSEGV          | Direct CPU Hardware Vector + Windowed WDT    |
| Fault Detection Delay     | Hundreds of CPU cycles (Page Table Walks)  | 1 Clock Cycle (Hardware Vector Signal)       |
| Infinite Loop Handling    | Software Preemption via OS Timer Interrupt | Hardware Windowed Watchdog NMI Trap          |
| Memory Isolation Cleanup  | Reclaim Virtual Page Tables & TLB Flushes  | Bitwise Zeroing of Task SRAM Capability Word |
| Recovery Latency          | > 100 µs (Context unwind & Signal)         | < 0.1 µs (Direct Vector Assembly Jump)       |
