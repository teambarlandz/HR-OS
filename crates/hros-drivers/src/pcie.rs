//! PCIe ECAM & Autonomous DMA Ring (Axis 2) — hros-drivers crate
//! Same as src/drivers/pcie.rs but using hros-kernel for no_std.

use core::sync::atomic::{AtomicU32, Ordering};
use hros_kernel::memory::{peek_u32, poke_u32};

pub const ECAM_BASE_ARM: usize = 0x40000000;
pub const ECAM_BASE_RISCV: usize = 0x30000000;

#[inline(always)]
pub fn ecam_addr(base: usize, bus: u8, dev: u8, func: u8, reg: usize) -> usize {
    base + ((bus as usize) << 20) + ((dev as usize) << 15) + ((func as usize) << 12) + reg
}

#[repr(C)]
pub struct PcieHeader {
    pub vendor_device: u32,
    pub command_status: u32,
    pub class_revision: u32,
    pub bist_header_latency_cache: u32,
    pub bar0: u32,
    pub bar1: u32,
    pub bar2: u32,
    pub bar3: u32,
    pub bar4: u32,
    pub bar5: u32,
}

#[inline(always)]
pub fn bar_size(bar_addr: usize) -> usize {
    let orig = peek_u32(bar_addr);
    poke_u32(bar_addr, 0xFFFFFFFF);
    let mask = peek_u32(bar_addr);
    poke_u32(bar_addr, orig);
    (!((mask as usize) & !0xF)) + 1
}

#[derive(Copy, Clone)]
#[repr(C, align(64))]
pub struct DmaDescriptor {
    pub src_addr: u64,
    pub dest_addr: u64,
    pub length: u32,
    pub flags: u32,
}

#[repr(C, align(64))]
pub struct AutonomousDmaRing {
    pub descriptors: [DmaDescriptor; 128],
    pub head: AtomicU32,
    pub tail: AtomicU32,
}

impl AutonomousDmaRing {
    pub const fn new() -> Self {
        Self {
            descriptors: [DmaDescriptor {
                src_addr: 0,
                dest_addr: 0,
                length: 0,
                flags: 0,
            }; 128],
            head: AtomicU32::new(0),
            tail: AtomicU32::new(0),
        }
    }
    #[inline(always)]
    #[allow(clippy::missing_safety_doc, clippy::result_unit_err)]
    pub unsafe fn submit_transfer(&self, src: u64, dest: u64, len: u32) -> Result<(), ()> {
        let cur_tail = self.tail.load(Ordering::Relaxed);
        let next_tail = (cur_tail + 1) % 128;
        if next_tail == self.head.load(Ordering::Acquire) {
            return Err(());
        }
        let desc_ptr =
            unsafe { self.descriptors.as_ptr().add(cur_tail as usize) } as *mut DmaDescriptor;
        unsafe {
            (*desc_ptr) = DmaDescriptor {
                src_addr: src,
                dest_addr: dest,
                length: len,
                flags: 0x01,
            };
            core::sync::atomic::compiler_fence(Ordering::Release);
        }
        self.tail.store(next_tail, Ordering::Release);
        Ok(())
    }
    #[inline(always)]
    pub fn has_completed(&self) -> bool {
        self.head.load(Ordering::Acquire) != self.tail.load(Ordering::Acquire)
    }
    #[inline(always)]
    pub fn capacity(&self) -> usize {
        let head = self.head.load(Ordering::Acquire) as usize;
        let tail = self.tail.load(Ordering::Acquire) as usize;
        (head + 128 - tail - 1) % 128
    }
}

pub static DMA_RING: AutonomousDmaRing = AutonomousDmaRing::new();
