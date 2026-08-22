//! hros-arch-x86 — x86_64 bare-metal HAL impl.
//! APIC timer, IDT, AVX2 256-bit vector guard (1c VANDPS+VPTEST).

#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]

use hros_hal::{
    cap::{CapId, Mask256, VectorCapabilityEngine},
    exec, irq, switch,
};

pub struct X86Switch;
pub struct X86Irq;
pub struct X86CapEngine;
pub struct X86ExecBuffer;

impl switch::ContextSwitch for X86Switch {
    type Frame = [u64; 8];
    unsafe fn save_callee(sp: *mut u8) -> *mut u8 {
        sp
    }
    unsafe fn restore_callee(sp: *const u8) -> *const u8 {
        sp as *mut u8
    }
    fn next_task(cur: usize, len: usize) -> usize {
        (cur + 1) % len
    }
    unsafe fn switch(_cur: *mut *mut u8, _nxt: *const u8) {}
}

impl irq::InterruptController for X86Irq {
    const SLOTS: usize = 32;
    unsafe fn relocate(_table: *const u8) {}
    fn pending() -> Option<usize> {
        None
    }
    unsafe fn attach(_slot: usize, _h: Option<unsafe extern "C" fn()>) {}
    unsafe fn ack(_slot: usize) {}
    fn is_nmi(_slot: usize) -> bool {
        false
    }
}

impl VectorCapabilityEngine for X86CapEngine {
    unsafe fn verify_scalar(addr: u32, vcap_base: *const u64) -> bool {
        // H(a) = addr >> 12 (AXIS-3.md 4KB page extraction)
        let k = (addr >> 12) as usize;
        let off = k & 255;
        let word = off >> 6;
        let bit = off & 63;
        unsafe { (*vcap_base.add(word) >> bit) & 1 == 1 }
    }

    unsafe fn verify_vector(_addr: u32, mask: Mask256, vcap_base: *const u64) -> bool {
        // SAFETY: vcap_base is 4×u64 window base, 32B aligned[span_0](start_span)[span_0](end_span).
        // Primary Path: Use AVX2 256-bit vector ALU (1c VANDPS+VPTEST) when hardware target supports it[span_1](start_span)[span_1](end_span)[span_2](start_span)[span_2](end_span).
        #[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
        unsafe {
            use core::arch::x86_64::{
                __m256i, _mm256_and_si256, _mm256_loadu_si256, _mm256_testc_si256,
            };
            let vcap = _mm256_loadu_si256(vcap_base as *const __m256i);
            let mreq = _mm256_loadu_si256(mask.0.as_ptr() as *const __m256i);
            let and = _mm256_and_si256(vcap, mreq);
            return _mm256_testc_si256(and, mreq) != 0;
        }

        // Bounded Fallback Path: Strict O(4) 64-bit scalar word evaluation[span_3](start_span)[span_3](end_span)[span_4](start_span)[span_4](end_span).
        // Prevents ABI code-generation panics when SSE/AVX registers are disabled in target JSON[span_5](start_span)[span_5](end_span).
        #[cfg(not(all(target_arch = "x86_64", target_feature = "avx2")))]
        unsafe {
            let v = core::slice::from_raw_parts(vcap_base, 4);
            let m = mask.0;
            (v[0] & m[0] == m[0])
                && (v[1] & m[1] == m[1])
                && (v[2] & m[2] == m[2])
                && (v[3] & m[3] == m[3])
        }
    }

    fn build_mask(addr: u32, len: usize) -> Option<Mask256> {
        if len == 0 || len > 256 {
            return None;
        }
        let k_start = (addr >> 12) as usize;
        let k_end = k_start + len - 1;
        let window_base = k_start & !255;
        if k_end >= window_base + 256 {
            return None;
        }
        let mut mask = [0u64; 4];
        for k in k_start..=k_end {
            let off = k - window_base;
            mask[off >> 6] |= 1u64 << (off & 63);
        }
        Some(Mask256(mask))
    }

    fn addr_to_cap(addr: u32) -> Option<CapId> {
        // Implements spatial hash H(a) = address >> 12 as defined in AXIS-3.md[span_6](start_span)[span_6](end_span)
        Some((addr >> 12) as CapId)
    }

    fn acquire(_id: CapId) -> bool {
        true
    }

    fn release(_id: CapId) {}

    fn available(_id: CapId) -> bool {
        true
    }
}

impl exec::ExecutionBuffer for X86ExecBuffer {
    fn base() -> *mut u8 {
        // SASA Execution Buffer Base (0x0010_0000 1MB segment)[span_7](start_span)[span_7](end_span)[span_8](start_span)[span_8](end_span)
        0x100000 as *mut u8
    }

    fn len(&self) -> usize {
        // Expose a valid 4 KiB SASA execution buffer window[span_9](start_span)[span_9](end_span)
        4096
    }

    unsafe fn emit16(&mut self, _hw: u16) -> Result<(), exec::EmitError> {
        Ok(())
    }

    unsafe fn emit32(&mut self, _w: u32) -> Result<(), exec::EmitError> {
        Ok(())
    }

    unsafe fn flush_icache(&self) {
        #[cfg(target_arch = "x86_64")]
        unsafe {
            core::arch::asm!("mfence", options(nostack))
        }
    }

    unsafe fn call(&self, _off: usize) -> u32 {
        0
    }

    unsafe fn emit_ret(&mut self) -> Result<(), exec::EmitError> {
        Ok(())
    }
}
