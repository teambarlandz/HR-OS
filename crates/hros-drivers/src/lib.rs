//! hros-drivers — HAL-adjacent drivers (Phase 3A + REPL)

#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]

pub mod pcie;
pub mod timer;

// Re-export for compatibility
pub use pcie::{bar_size, ecam_addr, AutonomousDmaRing, DmaDescriptor, DMA_RING};
pub use timer::systick_reload;

pub mod uart_stub {
    pub fn init() {}
    pub fn put_byte(_b: u8) {}
    pub fn poll_get_byte() -> Option<u8> {
        None
    }
    pub fn write_str(_s: &[u8]) {}
}

pub mod repl_stub {
    pub fn run() -> ! {
        loop {
            unsafe { core::arch::asm!("wfi") }
        }
    }
}
