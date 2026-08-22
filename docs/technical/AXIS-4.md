# Axis 4: Single-Pass JIT Machine Code Synthesis

Axis 4 defines how the Holy Rust Unikernel Operating System (HR-OS) eliminates the traditional toolchain lifecycle — static compilation, linkers, disk binary files (`.elf`/`.exe`), and binary loader subsystems.

Instead of reading binary executables off persistent block storage, the HR-OS kernel acts as a living, single-pass instruction emitter. As ASCII text streams arrive byte-by-byte over a hardware interface (UART serial, USB keyboard buffer, or network packet ring), HR-OS's inline compiler translates symbolic tokens directly into executable machine code opcodes (O(1) lookahead) and writes them into physical RAM (`EXEC_BUFFER`).

---

## 1. Mathematical Formalism of Single-Pass Compulsory Translation

Let `Σ` be the ASCII input alphabet. An incoming program is an ordered finite sequence of characters:

```text
P = (a₀, a₁, …, a_n−1)   where  a_i ∈ Σ
```

### A. Non-Backtracking Grammar Constraint

HR-OS enforces a strict **LL(1) Non-Backtracking Grammar**. The parsing function `P` reads character `a_i` and requires at most 1 token of lookahead to make a deterministic state transition without maintaining a parse tree in memory:

```text
δ: Q × Σ → Q × O*       (single-step, O(1) lookahead)
```

Where:

- `Q` is the finite set of compiler internal state modes.
- `O*` is a sequence of native 16-bit or 32-bit machine code opcodes emitted directly to memory.

### B. Linear Memory Emitter Function

Let `α ∈ 𝔸` be the physical address pointer inside `EXEC_BUFFER` (`0x0000_0000_1000_0000`). As valid statements are parsed, opcodes are written sequentially:

```text
*α       = opcode₀
*(α + 2) = opcode₁   (Thumb-2 16-bit)
*(α + 4) = opcode₂   ...
 α ← α + sizeof(opcode)
```

This guarantees that compilation time scales strictly linearly with input size: `T(n) = O(n)`.

---

## 2. Pipeline Architecture: Character Stream to RAM Execution

```text
               ASCII CHARACTER STREAM (UART / Keyboard Buffer)
                                     │
                                     ▼
                    ┌─────────────────────────────────┐
                    │  Stage 1: Streaming Lexer Engine│
                    └────────────────┬────────────────┘
                                     │ Tokens
                                     ▼
                    ┌─────────────────────────────────┐
                    │  Stage 2: Deterministic Parser  │
                    └────────────────┬────────────────┘
                                     │ Semantic Actions
                                     ▼
                    ┌─────────────────────────────────┐
                    │  Stage 3: Machine Code Emitter │
                    └────────────────┬────────────────┘
                                     │ Native Opcodes
                                     ▼
                  EXEC_BUFFER IN DRAM (0x0000_0000_1000_0000)
                                     │
                                     ▼
                         CPU PROGRAM COUNTER (PC/RIP)
```

### Stage 1: Streaming Lexer Engine

The lexer consumes single ASCII bytes from the hardware FIFO register (`peek(UART_DR)`). It accumulates characters into a tiny 32-byte scratchpad buffer until a delimiter (space, newline, parenthesis, brace) is detected.

### Stage 2: Deterministic State Machine Parser

Tokens are mapped to operational primitives using an inline hash/switch table:

| Symbolic Token | Parsed Operational Category | Hardware Behavior                                     |
| -------------- | --------------------------- | ----------------------------------------------------- |
| `poke`         | Volatile Memory Write       | Emits Base Address + Value registers, then `STR`      |
| `peek`         | Volatile Memory Read        | Emits Base Address register, then `LDR`               |
| `loop`         | Bounded Iteration           | Emits Counter register decrement & conditional branch |
| `delay`        | Bus Cycle Timing            | Emits `NOP` / delay loop instructions                 |

### Stage 3: Direct Machine Code Synthesis

Rather than building an Abstract Syntax Tree (AST), the parser triggers machine code emitter routines immediately upon identifying a command node.

---

## 3. Concrete Translation Walkthrough (ARM Thumb-2 & ARM64)

To understand the exact binary conversion, consider a high-level HR-OS command to clear a hardware register:

```text
poke 0x40001000 1
```

### Step-by-Step Opcode Generation (ARM Thumb-2 16/32-bit Instruction Set)

#### 1. Load Address (0x40001000) into Register R0

- **Instruction:** `MOV32 R0, 0x40001000` (Encoded as two 16-bit Thumb-2 words: MOVW and MOVT).
- **Binary Synthesis:**
  - Lower 16 bits (`0x1000`): Opcode `0xF241 0x0000`
  - Upper 16 bits (`0x4000`): Opcode `0xF2C4 0x0000`
- **Emitted Hex Bytes:** `0xF2410000` followed by `0xF2C40000`

#### 2. Load Immediate Value (0x00000001) into Register R1

- **Instruction:** `MOVS R1, #1`
- **Binary Synthesis:**
  - Opcode Pattern: `0x2100 | Immediate_Value`
  - Emitted Hex Bytes: `0x2101`

#### 3. Inject Axis 3 Capability Check

- Emitter inserts the 3-instruction O(1) safety guard verifying `R0` against the task's capability bitmask in SRAM.

#### 4. Execute Volatile Store (poke)

- **Instruction:** `STR R1, [R0]`
- **Binary Synthesis:**
  - Opcode Pattern: `0x6000 | (R0 << 3) | R1`
  - Emitted Hex Bytes: `0x6001`

#### 5. Target EXEC_BUFFER Memory Layout After Synthesis

| Address       | Opcode Hex     | Disassembly / Instruction               |
| ------------- | -------------- | --------------------------------------- |
| `0x1000_0000` | `0xF2410000`   | `MOVW R0, #0x1000`                      |
| `0x1000_0004` | `0xF2C40000`   | `MOVT R0, #0x4000` ; R0 = 0x40001000    |
| `0x1000_0008` | `0x2101`       | `MOVS R1, #1` ; R1 = 0x00000001         |
| `0x1000_000A` | `[GUARDBLOCK]` | Axis 3 O(1) Bitwise Safety Verification |
| `0x1000_0016` | `0x6001`       | `STR R1, [R0]` ; Volatile Write (poke)  |
| `0x1000_0018` | `0x4770`       | `BX LR` ; Return to Kernel Loop         |

---

## 4. Compiling Iterative Control Structures: Single-Pass Backpatching

A challenge in single-pass compilation is handling forward conditional jumps — such as loop blocks — where the jump target address is unknown until the end of the block is parsed.

HR-OS solves this using Linear Offset Backpatching using a small LIFO stack:

```text
Stream Parse: loop 1000 { poke 0xE000E010 1 }
```

1. Lexer encounters `'loop 1000 {'`
2. Emitter outputs: `MOVS R2, #1000` (Load loop counter)
3. Emitter records Current Address `α_start = 0x1000_0010`
4. Emitter outputs body opcodes: `STR R1, [R0]` (Body execution)
5. Lexer encounters closing brace `'}'`
6. Emitter outputs: `SUBS R2, R2, #1` (Decrement loop counter)
7. Emitter calculates relative jump offset:

```text
Offset = α_start − α_current
```

8. Emitter outputs: `BNE (0x2600 | Offset)` (Branch back to `α_start` if non-zero)

Because control blocks specify jump distances relative to current program offsets, the jump instruction can be emitted immediately at block closure without rescanning input text or maintaining secondary AST trees.

---

## 5. Execution Transfer: Jumping to Generated Opcodes

Once the parser encounters a line terminator (e.g., `\n` or semicolon), compilation halts and execution begins instantly.

### ARM/Thumb-2 Register State Transition

HR-OS loads the start address of the compiled function into a register and branches:

```asm
; R0 = Base Target Address in EXEC_BUFFER (0x1000_0000)
; Set Bit 0 to 1 to signal Thumb Execution Mode to the ARM Core

ORR R0, R0, #1      ; R0 = 0x1000_0001
BX  R0              ; Branch & Exchange: PC jumps directly to new opcodes
```

The CPU hardware transitions instruction fetching from the kernel body to `EXEC_BUFFER`. Execution runs at the full native clock rate of the processor (2–4 GHz on modern hardware).

---

## 6. Traditional Toolchain Compilation vs. HR-OS Single-Pass JIT

| Compulsory Metric           | Traditional Compiler Toolchain (gcc/rustc)            | HR-OS Inline Single-Pass Emitter           |
| --------------------------- | ----------------------------------------------------- | ------------------------------------------ |
| Intermediate Representation | Multi-layer (AST → High-Level IR → LLVM IR → Codegen) | None (Direct ASCII → Machine Opcodes)      |
| Backtracking & Pass Count   | Multi-pass (10+ optimization passes)                  | Single Pass (O(n) Linear Scan)             |
| Output Target               | Binary Disk File (.elf, .exe, .apk)                   | Direct Executable RAM (EXEC_BUFFER)        |
| Execution Delay             | Seconds to Minutes (Build, link, flash, load)         | Microseconds (Instant stream-to-execution) |
| Memory Footprint            | Gigabytes of RAM required for toolchain               | < 32 KB RAM Compiler Kernel Footprint      |
