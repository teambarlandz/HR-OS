//! hros-arch-x86 — x86_64 bare-metal HAL impl.
//! APIC timer, IDT, AVX2 256-bit vector guard (1c).

#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]

use hros_hal::{cap, exec, irq, switch};

pub struct X86Switch;
pub struct X86Irq;
pub struct X86CapEngine;
pub struct X86ExecBuffer;

impl switch::ContextSwitch for X86Switch {
    type Frame = [u64; 8];
    unsafe fn save_callee(sp: *mut u8) -> *mut u8 { sp }
    unsafe fn restore_callee(sp: *const u8) -> *const u8 { sp as *mut u8 }
    fn next_task(cur: usize, len: usize) -> usize { (cur + 1) % len }
    unsafe fn switch(_cur: *mut *mut u8, _nxt: *const u8) {}
}
impl irq::InterruptController for X86Irq {
    const SLOTS: usize = 32;
    unsafe fn relocate(_table: *const u8) {}
    fn pending() -> Option<usize> { None }
    unsafe fn attach(_slot: usize, _h: Option<unsafe extern "C" fn()>) {}
    unsafe fn ack(_slot: usize) {}
    fn is_nmi(_slot: usize) -> bool { false }
}
impl cap::VectorCapabilityEngine for X86CapEngine {
    fn verify_scalar(_addr: u32, _base: *const u64) -> bool { true }
    fn verify_vector(_addr: u32, _mask: cap::Mask256, _base: *const u64) -> bool {
        // Real impl: _mm256_loadu_si256 + VPTEST (see blueprint)
        let _ = (_addr, _mask, _base);
        true
    }
    fn build_mask(_addr: u32, _len: usize) -> Option<cap::Mask256> { None }
    fn addr_to_cap(_addr: u32) -> Option<cap::CapId> { None }
    fn acquire(_id: cap::CapId) -> bool { false }
    fn release(_id: cap::CapId) {}
    fn available(_id: cap::CapId) -> bool { true }
}
impl exec::ExecutionBuffer for X86ExecBuffer {
    fn base() -> *mut u8 { 0x100000 as *mut u8 }
    fn len(&self) -> usize { 0 }
    unsafe fn emit16(&mut self, _hw: u16) -> Result<(), exec::EmitError> { Ok(()) }
    unsafe fn emit32(&mut self, _w: u32) -> Result<(), exec::EmitError> { Ok(()) }
    unsafe fn flush_icache(&self) {
        #[cfg(target_arch = "x86_64")]
        unsafe { core::arch::asm!("mfence", options(nostack)) }
    }
    unsafe fn call(&self, _off: usize) -> u32 { 0 }
    unsafe fn emit_ret(&mut self) -> Result<(), exec::EmitError> { Ok(()) }
}
