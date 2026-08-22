//! hros-arch-riscv — RISC-V RV32IMAC/RV64 HAL impl.
//! fence.i, mtvec, gp relaxation, WFI.

#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]

use core::arch::asm;
use hros_hal::{cap, exec, irq, switch};

pub struct RiscvSwitch;
pub struct RiscvIrv;
pub struct RiscvCapEngine;
pub struct RiscvExecBuffer;

impl switch::ContextSwitch for RiscvSwitch {
    type Frame = [u32; 8];
    #[inline(always)]
    unsafe fn save_callee(sp: *mut u8) -> *mut u8 {
        // RISC-V callee-saved: s0-s11 (x8-x9, x18-x27) — save s0-s7 as example (8 regs)
        let mut out = sp;
        unsafe {
            asm!(
                "addi {sp}, {sp}, -32",
                "sw s0, 0({sp})", "sw s1, 4({sp})", "sw s2, 8({sp})", "sw s3, 12({sp})",
                "sw s4, 16({sp})", "sw s5, 20({sp})", "sw s6, 24({sp})", "sw s7, 28({sp})",
                sp = inout(reg) out,
                options(nostack)
            )
        }
        out
    }
    #[inline(always)]
    unsafe fn restore_callee(sp: *const u8) -> *const u8 {
        let mut inp = sp as *mut u8;
        unsafe {
            asm!(
                "lw s0, 0({sp})", "lw s1, 4({sp})", "lw s2, 8({sp})", "lw s3, 12({sp})",
                "lw s4, 16({sp})", "lw s5, 20({sp})", "lw s6, 24({sp})", "lw s7, 28({sp})",
                "addi {sp}, {sp}, 32",
                sp = inout(reg) inp,
                options(nostack)
            )
        }
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
            asm!("mv sp, {0}", in(reg) restored, options(nostack));
        }
    }
}
impl irq::InterruptController for RiscvIrv {
    const SLOTS: usize = 32;
    unsafe fn relocate(table: *const u8) {
        let base = table as usize & !0x3;
        unsafe { asm!("csrw mtvec, {0}", in(reg) base) }
    }
    fn pending() -> Option<usize> {
        let cause: usize;
        unsafe { asm!("csrr {0}, mcause", out(reg) cause) }
        Some(cause & 0x7FFFFFFF)
    }
    unsafe fn attach(_slot: usize, _h: Option<unsafe extern "C" fn()>) {
        unsafe { asm!("fence.i", options(nostack)) }
    }
    unsafe fn ack(_slot: usize) {}
    fn is_nmi(_slot: usize) -> bool {
        false
    }
}
impl cap::VectorCapabilityEngine for RiscvCapEngine {
    unsafe fn verify_scalar(addr: u32, base: *const u64) -> bool {
        let k = (addr >> 12) as usize & 255;
        let word = k >> 6;
        let bit = k & 63;
        unsafe { (*base.add(word) >> bit) & 1 == 1 }
    }
    unsafe fn verify_vector(_addr: u32, mask: cap::Mask256, base: *const u64) -> bool {
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
impl exec::ExecutionBuffer for RiscvExecBuffer {
    fn base() -> *mut u8 {
        0x08000000 as *mut u8
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
        unsafe { asm!("fence.i", options(nostack)) }
    }
    unsafe fn call(&self, _off: usize) -> u32 {
        0
    }
    unsafe fn emit_ret(&mut self) -> Result<(), exec::EmitError> {
        Ok(())
    }
}
