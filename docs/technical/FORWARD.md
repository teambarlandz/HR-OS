# Forward: Booting HR-OS on Real Hardware

To turn your tested Holy Rust repository into an OS that actually boots on real computer hardware or hypervisors, you need to bridge the gap between raw source code and bare-metal hardware initialization.

Since HR-OS operates without an underlying operating system, your Rust code cannot depend on standard system libraries (`std`). It must execute directly on hardware starting from power-on.

---

## Step 1: Configure the Rust Compiler for Bare Metal (`no_std`)

Standard Rust binaries expect an OS like Linux or Windows to set up memory and load them. For an OS, you must target a bare-metal environment.

- **Target Triples:** Add a bare-metal target for your CPU architecture:
  - x86_64: `x86_64-unknown-none`
  - ARM64: `aarch64-unknown-none`
  - ARM Cortex-M (Microcontrollers): `thumbv7em-none-eabihf`

- **Cargo Configuration (`.cargo/config.toml`):**

```toml
[build]
target = "x86_64-unknown-none"

[target.x86_64-unknown-none]
rustflags = [
    "-C", "link-arg=-Tlinker.ld", # Custom linker script
    "-C", "code-model=kernel",
    "-C", "relocation-model=static",
]
```

- **Code Declarations:** Ensure your entry point disables `std` and defines a custom panic handler:

```rust
#![no_std]
#![no_main]

use core::panic::PanicInfo;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
```

---

## Step 2: Write a Boot Sequence & Linker Script

When a computer turns on, the BIOS/UEFI initializes hardware and looks for a bootloader to hand off control to your kernel.

### 1. Modern Bootloader Integration

Instead of writing a complex x86_64 bootloader from scratch, use an established bootloader standard like Multiboot2 or the `bootloader` crate in Rust:

- **Option A (bootloader crate):** Easiest for Rust. It compiles your kernel and packages it into a bootable BIOS/UEFI image.
- **Option B (Limine / GRUB):** Write a Multiboot2 header at the start of your binary so standard bootloaders can jump directly to your Rust code.

### 2. The Linker Script (`linker.ld`)

Tell the compiler exactly where to place your HR-OS components (like `EXEC_BUFFER` and the Axis 3 Capability SRAM) in physical memory:

```ld
ENTRY(_start)

SECTIONS
{
    . = 0x100000; /* Load kernel at 1MB physical memory */

    .boot : {
        KEEP(*(.multiboot_header))
    }

    .text : {
        *(.text*)
    }

    /* Place Axis 3 Capability Matrix at fixed physical SRAM address */
    . = 0x800000;
    .cap_sram : {
        *(.cap_sram)
    }

    /* Reserve live EXEC_BUFFER space for Axis 4 JIT */
    . = 0x10000000;
    .exec_buffer : {
        *(.exec_buffer)
    }
}
```

---

## Step 3: Implement Bare-Metal Initialization Entry Point (`_start`)

Before Axis 1–4 can run, the hardware needs basic startup assembly to set up a CPU stack pointer and jump to Rust.

```rust
#[no_mangle]
pub extern "C" fn _start() -> ! {
    // 1. Initialize Bare-Metal Serial/UART for Axis 4 streaming input
    uart::init();

    // 2. Clear & Zero Axis 3 Capability SRAM
    capability::init_sram_matrix();

    // 3. Configure Axis 1 Hardware SysTick / APIC Timer Interrupts
    timer::init_hardware_systick(1000); // 1ms quantum

    // 4. Start Axis 4 ASCII JIT Reader & Kernel Loop
    kernel_main();

    loop {}
}
```

---

## Step 4: Build & Package the Bootable Disk Image

To boot on real PC hardware or virtual machines, package the compiled ELF binary into a bootable ISO or RAW Disk Image (`.img`).

If using `bootloader`:

```bash
# Install the bootimage tool
cargo install bootimage

# Build the bootable disk image
cargo bootimage
```

This generates a file named `target/x86_64-unknown-none/debug/bootimage-holy_rust.bin`.

---

## Step 5: Test, Debug, and Deploy

### 1. Test in QEMU (Virtualization)

Run your bootable image inside QEMU to monitor serial outputs and hardware traps:

```bash
qemu-system-x86_64 -drive format=raw,file=target/x86_64-unknown-none/debug/bootimage-holy_rust.bin -serial stdio
```

### 2. Deploy to Real Hardware

To boot on a physical computer or bare-metal server:

- **Flash to USB Drive:** Use `dd` (Linux/macOS) or Rufus (Windows) to write the `.bin` or `.iso` image to a USB flash drive:

```bash
sudo dd if=bootimage-holy_rust.bin of=/dev/sdX bs=4M status=progress && sync
```

- **Boot Hardware:** Insert the USB into your target machine, enter BIOS/UEFI settings, disable Secure Boot, set the boot device to USB, and boot directly into HR-OS.

---

## Configuring Cargo for Bare Metal

Setting up a custom target specification and `Cargo.toml` allows Cargo to compile your Rust code directly to bare-metal machine instructions without assuming an underlying operating system like Linux or Windows.

### Step 1: Create a Custom Target JSON File (`x86_64-hros-none.json`)

Standard targets like `x86_64-unknown-none` work, but defining a custom target specification lets you explicitly configure memory layouts, disable hardware floating-point registers if desired, and prevent red-zone stack corruption in bare-metal interrupt handlers.

Create a file named `x86_64-hros-none.json` in your repository root:

```json
{
  "llvm-target": "x86_64-unknown-none",
  "data-layout": "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128",
  "arch": "x86_64",
  "target-endian": "little",
  "target-pointer-width": "64",
  "target-c-int-width": "32",
  "os": "none",
  "executables": true,
  "linker-flavor": "ld.lld",
  "linker": "rust-lld",
  "panic-strategy": "abort",
  "disable-redzone": true,
  "features": "-mmx,-sse,+soft-float"
}
```

- `"linker": "rust-lld"`: Uses LLVM's built-in LLD linker so you do not depend on a host system GCC/Clang linker.
- `"disable-redzone": true`: Critical for bare-metal kernels. Prevents the compiler from optimizing stack allocations in a way that interrupt handlers could corrupt.
- `"panic-strategy": "abort"`: Removes stack unwinding metadata (`libunwind`), keeping the kernel binary compact and deterministic.

### Step 2: Configure Cargo for Bare-Metal Compilation (`.cargo/config.toml`)

Create or update `.cargo/config.toml` to tell Cargo to automatically build the core library for your custom target specification and pass your custom linker script to LLD:

```toml
[build]
# Set default compilation target to your custom JSON spec file
target = "x86_64-hros-none.json"

[target.x86_64-hros-none]
# Custom LLVM flags for bare-metal kernel generation
rustflags = [
    "-C", "link-arg=-Tlinker.ld",        # Direct LLD to use your bare-metal layout script
    "-C", "code-model=kernel",           # Restrict code addresses to top 2GB virtual/physical space
    "-C", "relocation-model=static",     # Disable dynamic position-independent code (PIC)
    "-C", "force-frame-pointers=yes"     # Preserve frame pointers for deterministic stack unwinding
]

[unstable]
# Tell Rust to automatically compile core/compiler_builtins for your custom target
build-std = ["core", "compiler_builtins"]
build-std-features = ["compiler-builtins-mem"]
```

### Step 3: Configure `Cargo.toml`

Your root `Cargo.toml` must declare the kernel binary and configure optimization profiles for hard real-time execution:

```toml
[package]
name = "holy_rust_os"
version = "0.1.0"
edition = "2021"
authors = ["HR-OS Core Team"]
description = "Holy Rust Operating System: Bare-Metal O(1) Unikernel"

# Disable automatic discovery of main.rs if using custom entry points
[[bin]]
name = "hros_kernel"
path = "src/main.rs"

[dependencies]
# Optional compiler intrinsic helpers
rlibc = "1.0"

# Strict release optimization profile for WCET determinism
[profile.dev]
panic = "abort"

[profile.release]
panic = "abort"
opt-level = 3        # Maximum compiler performance
lto = true           # Link-Time Optimization to resolve cross-crate inline calls
codegen-units = 1    # Force single code-generation unit for maximal LTO optimization
```

### Step 4: Minimal Kernel Entry Point (`src/main.rs`)

Replace standard `main()` with a `no_std` entry point:

```rust
#![no_std]
#![no_main]

use core::panic::PanicInfo;

/// Bare-metal kernel entry point called by bootloader assembly
#[no_mangle]
pub extern "C" fn _start() -> ! {
    // 1. Initialize Axis 3 SRAM capability vectors
    // 2. Start Axis 4 JIT listener
    // 3. Enable Axis 1 SysTick timer

    loop {
        // Halt CPU until next interrupt (Axis 1 preemption)
        core::hint::spin_loop();
    }
}

/// Diverging panic handler required when `std` is disabled
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
```

### Step 5: Test Building the Bare-Metal Target

Ensure you have a nightly Rust toolchain installed (required for the `build-std` feature):

```bash
rustup override set nightly
rustup component add rust-src
cargo build --release
```

Cargo will build `core` from source for `x86_64-hros-none` and produce a bare-metal ELF binary in `target/x86_64-hros-none/release/hros_kernel`.
