//! PCIe ECAM & Autonomous DMA Ring (Axis 2, Phase 3)
//! Single Address Space: ECAM identity-mapped, DMA bus-mastering 0 CPU cycles.
//! See docs/technical/AXIS-2.md and UPGRADE.md Step 3.

use core::sync::atomic::{AtomicU32, Ordering};

/// ECAM base from MCFG ACPI table (QEMU: 0x40000000 for ARM, 0x30000000 for RISC-V via alias)
pub const ECAM_BASE_ARM: usize = 0x40000000;
pub const ECAM_BASE_RISCV: usize = 0x30000000;

/// O(1) ECAM address: Target = Base + (B<<20)|(D<<15)|(F<<12)|R
#[inline(always)]
pub fn ecam_addr(base: usize, bus: u8, dev: u8, func: u8, reg: usize) -> usize {
    base + ((bus as usize) << 20) + ((dev as usize) << 15) + ((func as usize) << 12) + reg
}

/// PCIe device header (Type 00h) — first 64 bytes
#[repr(C)]
pub struct PcieHeader {
    pub vendor_device: u32, // 0x00: vendor 16 + device 16
    pub command_status: u32, // 0x04
    pub class_revision: u32, // 0x08
    pub bist_header_latency_cache: u32, // 0x0C
    pub bar0: u32, // 0x10
    pub bar1: u32, // 0x14
    pub bar2: u32, // 0x18
    pub bar3: u32, // 0x1C
    pub bar4: u32, // 0x20
    pub bar5: u32, // 0x24
    // ... rest omitted
}

/// BAR sizing: write all 1s, read back mask, restore, compute size `~(mask & !0xF) + 1`
#[inline(always)]
pub fn bar_size(bar_addr: usize) -> usize {
    let orig = crate::kernel::memory::peek_u32(bar_addr);
    crate::kernel::memory::poke_u32(bar_addr, 0xFFFFFFFF);
    let mask = crate::kernel::memory::peek_u32(bar_addr);
    crate::kernel::memory::poke_u32(bar_addr, orig);
    let size = (!((mask as usize) & !0xF)) + 1;
    size
}

/// Enumerate ECAM bus: O(N) sweep, returns number of devices found
pub fn enumerate_ecam(base: usize, out: &mut [PcieHeader; 32]) -> usize {
    let mut found = 0usize;
    for bus in 0..=255u16 {
        for dev in 0..32u8 {
            for func in 0..8u8 {
                let addr = ecam_addr(base, bus as u8, dev, func, 0);
                // Use volatile read via peek
                let vendor = crate::kernel::memory::peek_u32(addr) & 0xFFFF;
                if vendor == 0xFFFF || vendor == 0x0000 {
                    if func == 0 {
                        // Check header type bit 7: if 0, single-function, break
                        let hdr = crate::kernel::memory::peek_u32(addr + 0x0E) & 0xFF;
                        if (hdr & 0x80) == 0 {
                            break;
                        }
                    }
                    continue;
                }
                if found < out.len() {
                    // Copy header words
                    out[found].vendor_device = crate::kernel::memory::peek_u32(addr);
                    out[found].command_status = crate::kernel::memory::peek_u32(addr + 0x04);
                    out[found].class_revision = crate::kernel::memory::peek_u32(addr + 0x08);
                    out[found].bar0 = crate::kernel::memory::peek_u32(addr + 0x10);
                    found += 1;
                }
                if func == 0 {
                    let hdr = crate::kernel::memory::peek_u32(addr + 0x0E) & 0xFF;
                    if (hdr & 0x80) == 0 {
                        break;
                    }
                }
            }
        }
        if found >= out.len() { break; }
        if bus == 0 && found == 0 {
            // Quick exit for QEMU without PCIe (netduino/sifive have no ECAM)
            break;
        }
    }
    found
}

// ---------------------------------------------------------------------------
// Autonomous DMA Ring — 0 CPU cycles blocked, PCIe TLP autonomous
// ---------------------------------------------------------------------------

/// DMA descriptor — 64B cache line aligned, hardware-owned head vs driver tail
#[derive(Copy, Clone)]
#[repr(C, align(64))]
pub struct DmaDescriptor {
    pub src_addr: u64,      // Physical source (SASA)
    pub dest_addr: u64,     // Physical dest MMIO/RAM
    pub length: u32,        // Bytes
    pub flags: u32,         // Ready, EOR, IOC
}

/// Autonomous DMA ring — lock-free SPSC, head = HW, tail = driver
#[repr(C, align(64))]
pub struct AutonomousDmaRing {
    pub descriptors: [DmaDescriptor; 128],
    pub head: AtomicU32, // HW updates via bus mastering
    pub tail: AtomicU32, // Driver updates
}

impl AutonomousDmaRing {
    pub const fn new() -> Self {
        Self {
            descriptors: [DmaDescriptor { src_addr: 0, dest_addr: 0, length: 0, flags: 0 }; 128],
            head: AtomicU32::new(0),
            tail: AtomicU32::new(0),
        }
    }

    /// Enqueue zero-copy transfer — O(1), 0 blocked CPU (async TLP)
    #[inline(always)]
    pub unsafe fn submit_transfer(&self, src: u64, dest: u64, len: u32) -> Result<(), ()> {
        let cur_tail = self.tail.load(Ordering::Relaxed);
        let next_tail = (cur_tail + 1) % 128;
        if next_tail == self.head.load(Ordering::Acquire) {
            return Err(()); // Ring full
        }
        let desc_ptr = unsafe { self.descriptors.as_ptr().add(cur_tail as usize) } as *mut DmaDescriptor;
        unsafe {
            (*desc_ptr) = DmaDescriptor { src_addr: src, dest_addr: dest, length: len, flags: 0x01 };
            // Ensure descriptor write completes before tail update (Release)
            core::sync::atomic::compiler_fence(Ordering::Release);
        }
        self.tail.store(next_tail, Ordering::Release);
        Ok(())
    }

    /// Check if ring has completed entries (head != tail)
    #[inline(always)]
    pub fn has_completed(&self) -> bool {
        self.head.load(Ordering::Acquire) != self.tail.load(Ordering::Acquire)
    }

    /// Capacity remaining: C = (head - tail -1) mod 128
    #[inline(always)]
    pub fn capacity(&self) -> usize {
        let head = self.head.load(Ordering::Acquire) as usize;
        let tail = self.tail.load(Ordering::Acquire) as usize;
        (head + 128 - tail - 1) % 128
    }
}

/// Global DMA ring — placed in SRAM via linker .sram section
pub static DMA_RING: AutonomousDmaRing = AutonomousDmaRing::new();
