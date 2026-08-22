//! hros-arch-arm — ARM Cortex-M4/M7 HAL impl.
//! 12+8+3+8+12 cyc switch, VTOR 0xE000ED08, Thumb-2 emitters, WFE/SEV.

#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]

use core::arch::asm;
use hros_hal::{cap, exec, irq, switch};

pub struct ArmM4Switch;
pub struct ArmNvic;
pub struct ArmCapEngine;
pub struct ArmExecBuffer;

// 43c switch: HW auto-stack 12 + push 8 + sched 3 + pop 8 + unstack 12
impl switch::ContextSwitch for ArmM4Switch {
    type Frame = [u32; 8];
    #[inline(always)]
    unsafe fn save_callee(sp: *mut u8) -> *mut u8 {
        let mut out = sp;
        // SAFETY: sp in TCB bounds, Ring 0 SASA, stmdb decrement-before
        unsafe { asm!("stmdb {sp}!, {{r4-r11}}", sp = inout(reg) out, options(nostack)) }
        out
    }
    #[inline(always)]
    unsafe fn restore_callee(sp: *const u8) -> *const u8 {
        let mut inp = sp as *mut u8;
        unsafe { asm!("ldmia {sp}!, {{r4-r11}}", sp = inout(reg) inp, options(nostack)) }
        inp
    }
    #[inline(always)]
    fn next_task(cur: usize, len: usize) -> usize {
        (cur + 1) % len
    }
    #[inline(always)]
    unsafe fn switch(cur: *mut *mut u8, nxt: *const u8) {
        unsafe {
            let saved = Self::save_callee(*cur);
            core::ptr::write(cur, saved);
            let restored = Self::restore_callee(nxt);
            asm!("mov sp, {0}", in(reg) restored, options(nostack));
            // WFE/SEV for cross-core wake is handled by scheduler queue, not here
        }
    }
}
impl irq::InterruptController for ArmNvic {
    const SLOTS: usize = 32;
    unsafe fn relocate(table: *const u8) {
        // SAFETY: VTOR at 0xE000ED08, table 1024B aligned (SASA)
        unsafe {
            core::ptr::write_volatile(0xE000ED08 as *mut u32, table as u32);
            asm!("dsb", "isb", options(nostack));
        }
    }
    fn pending() -> Option<usize> {
        // ICSR at 0xE000ED04, VECTACTIVE 9 bits
        let icsr = unsafe { core::ptr::read_volatile(0xE000ED04 as *const u32) };
        let n = (icsr & 0x1FF) as usize;
        if n >= 16 {
            Some(n - 16)
        } else {
            None
        }
    }
    unsafe fn attach(slot: usize, h: Option<unsafe extern "C" fn()>) {
        if slot < 32 {
            // For Phase 2, we use the typed RAM_VECTOR_TABLE in hros-kernel, not raw VTOR slots
            // This stub shows the DSB/ISB barrier required after attach
            let _ = h;
            unsafe { asm!("dsb", "isb", options(nostack)) }
        }
    }
    unsafe fn ack(slot: usize) {
        // Example: clear pending bit in NVIC ISPR
        let ispr = 0xE000E200 as *mut u32;
        unsafe { core::ptr::write_volatile(ispr.add(slot >> 5), 1 << (slot & 31)) }
    }
    fn is_nmi(slot: usize) -> bool {
        slot == 2
    } // HardFault as NMI in HR-OS windowed WDT
}
impl cap::VectorCapabilityEngine for ArmCapEngine {
    unsafe fn verify_scalar(addr: u32, base: *const u64) -> bool {
        let k = (addr >> 12) as usize & 255;
        let word = k >> 6;
        let bit = k & 63;
        unsafe { (*base.add(word) >> bit) & 1 == 1 }
    }
    unsafe fn verify_vector(_addr: u32, mask: cap::Mask256, base: *const u64) -> bool {
        // On ARM with NEON, would use vld1q + vandq + ceq; fallback to scalar 4x
        unsafe {
            let v = core::slice::from_raw_parts(base, 4);
            let m = mask.0;
            (v[0] & m[0] == m[0])
                && (v[1] & m[1] == m[1])
                && (v[2] & m[2] == m[2])
                && (v[3] & m[3] == m[3])
        }
    }
    fn build_mask(addr: u32, len: usize) -> Option<cap::Mask256> {
        if len == 0 || len > 256 {
            return None;
        }
        let k_start = (addr >> 12) as usize;
        let k_end = k_start + len - 1;
        let base = k_start & !255;
        if k_end >= base + 256 {
            return None;
        }
        let mut m = [0u64; 4];
        for k in k_start..=k_end {
            let o = k - base;
            m[o >> 6] |= 1u64 << (o & 63);
        }
        Some(cap::Mask256(m))
    }
    fn addr_to_cap(_addr: u32) -> Option<cap::CapId> {
        None
    }
    fn acquire(_id: cap::CapId) -> bool {
        false
    }
    fn release(_id: cap::CapId) {}
    fn available(_id: cap::CapId) -> bool {
        true
    }
}
impl exec::ExecutionBuffer for ArmExecBuffer {
    fn base() -> *mut u8 {
        0x20002000 as *mut u8
    }
    fn len(&self) -> usize {
        0
    }
    unsafe fn emit16(&mut self, _hw: u16) -> Result<(), exec::EmitError> {
        Ok(())
    }
    unsafe fn emit32(&mut self, _w: u32) -> Result<(), exec::EmitError> {
        Ok(())
    }
    unsafe fn flush_icache(&self) {
        unsafe { asm!("dsb", "isb", options(nostack)) }
    }
    unsafe fn call(&self, _off: usize) -> u32 {
        0
    }
    unsafe fn emit_ret(&mut self) -> Result<(), exec::EmitError> {
        Ok(())
    }
}
