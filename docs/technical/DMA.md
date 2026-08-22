# DMA & Interrupt Capability Safety

In traditional operating systems, Direct Memory Access (DMA) transactions and hardware interrupt handlers present severe security vulnerabilities: DMA engines bypass the CPU's MMU entirely to read/write physical memory directly, while Interrupt Service Routines (ISRs) execute asynchronously with elevated kernel privileges.

HR-OS enforces O(1) capability safety across DMA and interrupts at the silicon interface layer without relying on a hardware IOMMU or adding dynamic runtime overhead to interrupt dispatching.

---

## 1. Asynchronous DMA Capability Validation

Because a hardware DMA controller (e.g., PCIe BAR, Ethernet, USB, or Scatter-Gather DMA) writes directly to physical DRAM addresses without passing through CPU registers, HR-OS enforces safety at **DMA configuration time** rather than during the transfer itself.

```text
  [ User Task ] ──► Tries to initiate DMA (Source Address A, Length L)
                          │
                          ▼
        [ Axis 4 Inline JIT DMA Trap Guard ]
                          │
  1. Boundary Check: A_end = A + L
  2. Range Validation: Extract Block Indices [k_start .. k_end]
  3. Axis 3 O(1) Mask Verification against Task Capability Vector
                          │
             ┌────────────┴────────────┐
       All Bits == 1              Any Bit == 0
             │                         │
             ▼                         ▼
   [ Write DMA Controller ]     [ Hardware Trap ]
   [ Channel Physical Regs]     [ Terminate Task]
```

### The DMA Validation Math

Before the JIT engine emits the hardware registers initialization (`DMA_SADDR`, `DMA_DADDR`, `DMA_LEN`, `DMA_CR`), it injects a Bounded Array Capability Sweep:

- **Range Extraction:** Given source/destination physical address `A` and transfer length `L`, the block range `[k_start, k_end]` is calculated:

```text
k_start = A >> 12
k_end   = (A + L − 1) >> 12
```

- **O(1) Word Masking (For Contiguous Blocks):** If `k_start` and `k_end` fall within the same 64-bit word of the Capability Matrix (`I_start == I_end`), verification reduces to a single bitwise AND operation:

```text
mask = ((1 << (k_end − k_start + 1)) − 1) << (k_start & 63)
authorized = (W[I_start] & mask) == mask
```

- **Zero-In-Flight Hazard Guarantee:** Once verified, physical addresses are committed to the DMA controller's registers. Because tasks cannot modify physical memory mappings dynamically in SASA (Axis 2), the DMA controller operates in a mathematically locked physical window. The task cannot modify its capability rights while a DMA channel is active.

---

## 2. Peripheral Interrupt Capability Safety

HR-OS eliminates privilege escalation in Interrupt Service Routines (ISRs) by eliminating traditional kernel-mode ISR routines. Interrupts are treated as asynchronous token pushes into task-isolated ring buffers.

```text
  [ Hardware Peripheral (e.g., UART/Ethernet) ]
                        │
                        ▼ Assert IRQ Line
          [ CPU Hardware NVIC / GIC ]
                        │
                        ▼ Vector Table Entry
        [ HR-OS Atomic Interrupt Router ]
                        │
     1. Read IRQ Channel ID (N)
     2. Lookup Static Capability Map: IRQ_Map[N] -> Target Task ID
     3. Verify IRQ Capability Bit in SRAM Matrix
                        │
                        ▼
    [ Push Payload to Task Ring Buffer in SRAM ]
                        │
                        ▼
   [ Trigger Axis 1 Preemption / Signal Ready ]
```

### A. The Static Vector Capability Binding

Each peripheral interrupt vector `N` (e.g., IRQ 37 for UART1) is assigned an implicit Hardware Capability Bit in the System Capability Universe `U`.

A task `Ti` can only register an event handler for IRQ `N` if its Axis 3 Capability Vector explicitly owns bit `N_IRQ`:

```text
C_Ti[N_IRQ] == 1  ⇔  Task Ti owns IRQ N
```

### B. Safe Asynchronous Dispatch Pipeline

When an IRQ fires:

- **Hardware Ingestion:** The CPU vectors to the single HR-OS Kernel Router (`.IRQ_ROUTER`). No user code ever executes in raw interrupt context.
- **O(1) Target Look Up:** The router reads the IRQ number `N` directly from the CPU Interrupt Controller status register (`ICSR` / `IAR`):

```text
N = ICSR & 0x1FF
```

- **Lockless Ring Buffer Injection:** The router writes the event payload (e.g., received byte from UART FIFO) directly into `Target_Task.Event_Buffer` located in shared SRAM.
- **Immediate Tail-Chaining (Axis 1):** The router marks `Target_Task` as `READY`. If the task's priority exceeds the currently running context, Axis 1 performs an immediate O(1) hardware context switch upon exiting the IRQ.

---

## 3. Comparative Safety Guarantee

| Hardware Boundary           | Conventional OS (Linux / microkernel)            | HR-OS Unikernel O(1) Architecture                     |
| --------------------------- | ------------------------------------------------ | ----------------------------------------------------- |
| DMA Protection              | Hardware IOMMU page table translation            | JIT-Injected Range Verification at Config Time        |
| IOMMU Latency Overhead      | High (IOMMU page faults, IoTLB misses)           | Zero (Native physical bus speeds)                     |
| Interrupt Execution Context | Elevated Kernel Privilege (Ring 0 / EL1 Handler) | Generic Kernel Router + Unprivileged Ring Buffer Push |
| ISR Vulnerability Scope     | Bug in ISR compromises full OS kernel            | Bug in Handler isolated strictly to assigned Task     |
| Verification Overhead       | Dynamic per-byte/page mapping checks             | Single bitwise mask evaluation before bus commit      |
