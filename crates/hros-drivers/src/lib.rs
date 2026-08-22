//! hros-drivers — HAL-adjacent drivers.
//! See src/drivers/* (reference impl).

#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]

pub mod uart {
    pub fn init() {}
    pub fn put_byte(_b: u8) {}
    pub fn poll_get_byte() -> Option<u8> { None }
    pub fn write_str(_s: &[u8]) {}
    pub fn write_hex_u32(_v: u32) {}
    pub fn write_dec_u32(_v: u32) {}
}
pub mod repl { pub fn run() -> ! { loop { unsafe { core::arch::asm!("wfi") } } } }
pub mod pcie {
    // ECAM enumerator stub — Phase 3A
    pub fn ecam_addr(base: usize, b: u8, d: u8, f: u8, r: usize) -> usize { base + ((b as usize)<<20) + ((d as usize)<<15) + ((f as usize)<<12) + r }
}
pub mod timer {
    pub fn init_systick(_ticks: u32) {}
}
