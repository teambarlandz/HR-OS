# Axis 2: Flat Memory Space Topology & PCIe Interconnect Mechanics

Axis 2 defines how the Holy Rust Unikernel Operating System (HR-OS) interacts with physical silicon memory, high-speed system interconnects, and PCI Express peripheral buses.

In standard operating systems, peripheral communication requires layered kernel abstractions: virtual memory translations via Page Tables, kernel-space driver allocation, I/O Memory Management Unit (IOMMU) remapping, and system call gates. HR-OS eliminates these layers by executing strictly in Ring 0 / EL1, utilizing a Single Address Space Architecture (SASA) where physical RAM, peripheral registers, configuration spaces, and DMA execution rings share a continuous, unmapped 64-bit physical address space.

---

## 1. The Global 64-Bit Physical Memory Map Topology

A 64-bit architecture (x86_64, AArch64, or RISC-V 64) provides an address space spanning up to 2⁶⁴ bytes (16 Exabytes). HR-OS partitions this space deterministically across physical boundaries without virtual memory offsets.

```text
 Physical Address Space (Flat 64-bit SASA)
 0x0000_0000_0000_0000 ┌────────────────────────────────────────────────────────┐
                      │ Vector Table & Core Exception Handlers                 │
 0x0000_0000_0010_0000 ├────────────────────────────────────────────────────────┤
                      │ HR-OS Kernel Runtime Engine & Lexer                    │
 0x0000_0000_0080_0000 ├────────────────────────────────────────────────────────┤
                      │ O(1) Capability Bitfield Registry                      │
 0x0000_0000_0100_0000 ├────────────────────────────────────────────────────────┤
                      │ System Task Control Blocks & Ring Buffers              │
 0x0000_0000_1000_0000 ├────────────────────────────────────────────────────────┤
                      │ EXEC_BUFFER (Native Assembly Opcode Target Buffer)     │
 0x0000_0000_8000_0000 ├────────────────────────────────────────────────────────┤
                      │ Physical System DRAM Pool (Stack & Allocations)        │
 0x0000_0000_E000_0000 ├────────────────────────────────────────────────────────┤
                      │ PCIe Enhanced Configuration Access Mechanism (ECAM)    │
 0x0000_0001_0000_0000 ├────────────────────────────────────────────────────────┤
                      │ Memory-Mapped High-Speed PCIe BAR Spaces (NVMe, GPU)  │
 0xFFFF_FFFF_FFFF_FFFF └────────────────────────────────────────────────────────┘
```

### Mathematical Mapping Function

Let `𝔸 = [0, 2⁶⁴−1]` be the set of physical addresses. Memory access in HR-OS is an identity transformation `M`:

```text
M(a) = a   for all a ∈ 𝔸
```

Unlike virtual memory systems where `Translate(VA) → PA` incurs a multi-cycle Page Table Walk across `PML4/PDPT/PD/PT` tables, access time `T(a)` in HR-OS is bounded only by physical interconnect bus propagation latency.

---

## 2. PCIe Interconnect Electronics & Topological Architecture

PCI Express (PCIe) is a high-speed, point-to-point, packet-based serial bus topology. Instead of shared parallel electrical lines, PCIe uses full-duplex differential lanes (x1, x4, x8, x16).

```text
 ┌─────────────────────────────────────────────────────────────────────────────┐
 │                    Host CPU Core & Root Complex                             │
 └──────────────────────────────────────┬──────────────────────────────────────┘
                                        │ High-Speed Internal System Bus
 ┌──────────────────────────────────────┴──────────────────────────────────────┐
 │               PCIe Host Controller / ECAM Window                            │
 └──────┬───────────────────────────────┬───────────────────────────────┬──────┘
        │ PCIe Lane (x16)               │ PCIe Lane (x4)                │ PCIe Lane (x1)
 ┌──────┴───────────────┐        ┌──────┴───────────────┐        ┌──────┴───────────────┐
 │ Discrete High-Speed  │        │ NVMe High-Speed      │        │ Gigabit Ethernet     │
 │ Graphics Processing  │        │ Solid State Drive    │        │ Network Controller   │
 │ Unit (GPU)           │        │ (Storage BAR)        │        │ (Network Ring BAR)   │
 └──────────────────────┘        └──────────────────────┘        └──────────────────────┘
```

### Addressing PCIe Space: The Bus/Device/Function Identifier

Every device on a PCIe topology is uniquely addressed by a 16-bit physical coordinate:

- **Bus Number (B):** 8 bits ⇒ `[0, 255]`
- **Device Number (D):** 5 bits ⇒ `[0, 31]`
- **Function Number (F):** 3 bits ⇒ `[0, 7]`

The total address space accommodates up to `256 × 32 × 8 = 65,536` unique functional endpoints.

---

## 3. The Enhanced Configuration Access Mechanism (ECAM)

Legacy x86 hardware accessed PCI devices through dual 32-bit I/O ports (`0xCF8` Configuration Address and `0xCFC` Configuration Data). Modern PCIe exposes the Enhanced Configuration Access Mechanism (ECAM), which maps the entire 256 MB configuration space directly into memory.

### ECAM Address Calculation Formula

Given a base memory address `ECAM_Base` provided by ACPI tables (typically `0xE000_0000` or retrieved via the MCFG ACPI table), the physical MMIO address for any target device register is computed in O(1) constant time:

```text
Target_Addr = ECAM_Base + (B << 20) + (D << 15) + (F << 12) + R
```

Where:

- `B ∈ [0, 255]` (Bus offset, shifted by 20 bits ⇒ 1 MB per Bus)
- `D ∈ [0, 31]` (Device offset, shifted by 15 bits ⇒ 32 KB per Device)
- `F ∈ [0, 7]` (Function offset, shifted by 12 bits ⇒ 4 KB per Function)
- `R ∈ [0, 4095]` (Register offset inside the function's 4 KB configuration block)

```text
 31                20 19        15 14    12 11                      0
┌────────────────────┬────────────┬────────┬─────────────────────────┐
│     Bus (8-bit)    │ Device(5b) │ Func(3b)│ Register Offset (12-bit)│
└────────────────────┴────────────┴────────┴─────────────────────────┘
```

---

## 4. Hardware Discovery Algorithm & BAR Parsing

HR-OS enumerates devices across the PCIe bus topology without using external drivers or OS bus abstractions. It scans the ECAM physical memory space directly.

### A. PCIe Configuration Header Structure (Type 00h - Endpoints)

Every functional device implements a mandatory 64-byte configuration header at `R = 0`:

| Offset | Contents                                         |
| ------ | ------------------------------------------------ |
| `0x00` | `[ Vendor ID (16-bit) ] [ Device ID (16-bit) ]`  |
| `0x04` | `[ Command Register ] [ Status Register ]`       |
| `0x08` | `[ Revision ID ] [ Class / Subclass / Prog IF ]` |
| `0x10` | `[ Base Address Register 0 (BAR0) ]`             |
| `0x14` | `[ Base Address Register 1 (BAR1) ]`             |
| `0x18` | `[ Base Address Register 2 (BAR2) ]`             |
| `0x1C` | `[ Base Address Register 3 (BAR3) ]`             |
| `0x20` | `[ Base Address Register 4 (BAR4) ]`             |
| `0x24` | `[ Base Address Register 5 (BAR5) ]`             |

### B. Mathematical Bus Enumeration Loop

The HR-OS discovery engine executes an O(N) sweep over valid physical memory offsets:

```text
1. Set Bus B = 0
2. Loop B from 0 to 255:
     Loop D from 0 to 31:
       Loop F from 0 to 7:
         Compute Target_Addr = ECAM_Base + (B << 20) + (D << 15) + (F << 12)
         Read Vendor_ID = peek(Target_Addr) & 0xFFFF

         IF Vendor_ID == 0xFFFF:
           Continue (No physical device connected)

         Read Device_ID = (peek(Target_Addr) >> 16) & 0xFFFF
         Read Class_Code = peek(Target_Addr + 0x08) >> 8

         Register Endpoint Entry in HR-OS Hardware Tree

         IF F == 0 AND (peek(Target_Addr + 0x0E) & 0x80) == 0:
           Break Function Loop (Single-function device)
```

### C. Base Address Register (BAR) Memory Sizing

Peripherals use Base Address Registers (BARs) to request dedicated blocks of main memory for high-speed hardware MMIO (e.g., framebuffer buffers or NVMe command rings).

To determine the memory size requested by a peripheral hardware BAR without using host OS helper APIs:

```text
1. Read Original BAR Value:              V_orig = peek(BAR_Addr)
2. Write All Ones (0xFFFFFFFF) to BAR:   poke(BAR_Addr, 0xFFFFFFFF)
3. Read Masked Value Back:               V_mask = peek(BAR_Addr)
4. Restore Original BAR Value:           poke(BAR_Addr, V_orig)
5. Calculate Size in Bytes:              Size = ~(V_mask & ~0xF) + 1
```

**Concrete Example:**

- Read back `V_mask = 0xFFF00000`
- Clear control bits ⇒ `0xFFF00000`
- Invert bits ⇒ `0x000FFFFF`
- Add 1 ⇒ `0x00100000 = 1,048,576 Bytes` ⇒ **1 MB Memory-Mapped Region**

---

## 5. Direct Memory Access (DMA) Mechanics

To achieve high throughput for disk storage, graphics rendering, and networking, the CPU must not copy data manually pixel-by-pixel or byte-by-byte. HR-OS uses Direct Memory Access (DMA) engines built into PCIe peripherals.

```text
 ┌─────────────────────────────────────────────────────────────────────────────┐
 │                              Main System DRAM                               │
 │  ┌───────────────────────────────────────────────────────────────────────┐  │
 │  │ Circular Ring Buffer (Command Descriptors / Frame Data)               │  │
 │  └───────────────────────────────────────────────────────────────────────┘  │
 └──────────────────────────────────────▲──────────────────────────────────────┘
                                        │ High-Speed Direct PCIe Bus Access
                                        │ (Bypasses CPU Core entirely)
 ┌──────────────────────────────────────┴──────────────────────────────────────┐
 │                      PCIe Peripheral DMA Controller                         │
 │  1. Read Physical Address from Doorbell Register                            │
 │  2. Fetch Data directly from System DRAM over PCIe Lanes                    │
 │  3. Execute Hardware Operation (Write to Flash / Display Output)            │
 └──────────────────────────────────────▲──────────────────────────────────────┘
                                        │
                                        │ Doorbell Write Signal (poke)
 ┌──────────────────────────────────────┴──────────────────────────────────────┐
 │                       HR-OS Kernel Execution Core                           │
 └─────────────────────────────────────────────────────────────────────────────┘
```

### DMA Execution Pipeline in HR-OS

- **Allocate Contiguous Physical Memory:** HR-OS picks a continuous physical address block in DRAM (e.g., `0x0000_0000_2000_0000`).
- **Construct Command Descriptors:** Write raw request packets (e.g., NVMe Read Commands) directly into that DRAM address using `poke`.
- **Pass Physical Pointer to Peripheral:** Write the physical DRAM starting address to the peripheral's BAR memory-mapped register:

```asm
poke(BAR_Doorbell, DRAM_Physical_Address)
```

- **Autonomous Transport:** The PCIe hardware DMA engine reads memory independently over the system bus, processes the ring payload, and fires an interrupt back to Axis 1's interrupt vector table when complete.

---

## 6. Comparison: Peripheral Memory Access Overhead

| Metric / Phase       | Linux / Windows Traditional Driver Model         | HR-OS SASA Unikernel Model                  |
| -------------------- | ------------------------------------------------ | ------------------------------------------- |
| Address Mode         | Virtual Address Space (VA → PA Translation)      | Physical Address Space (Identity Mapped)    |
| Driver Isolation     | User Mode (Ring 3) / Kernel Space (Ring 0) Gates | Single Ring 0 Execution Mode                |
| Configuration Access | Layered Kernel Frameworks (`pci_read_config`)    | Direct ECAM O(1) Physical `peek` / `poke`   |
| MMIO Mapping Latency | High (`ioremap()` page-table modification)       | Zero (Pre-mapped physical addresses)        |
| DMA Setup            | IOMMU Virtual Page Remapping & Buffer Locks      | Direct Physical Address Memory Pointer Pass |
