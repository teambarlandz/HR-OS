//! hros-cap — O(1) linear capability engine (Phase 2B) — bitfield registry + linear tokens + vector 1c
//! See docs/technical/AXIS-3.md and src/capabilities/* (reference impl).

#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]

use core::sync::atomic::{AtomicU32, Ordering};

/// Total tracked resources (8 words x 32 bits = 256 bits = 1 MiB per window).
pub const MAX_RESOURCES: usize = 256;
const WORDS: usize = MAX_RESOURCES / 32;

/// 256-bit request mask (4×64) vs task vector `Vcap ∈ {0,1}²⁵⁶`.
#[repr(C, align(32))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mask256(pub [u64; 4]);
impl Mask256 {
    pub const ZERO: Self = Self([0; 4]);
}

/// Build mask for `[addr, addr+len*4096)` (len ≤256, single 256-window).
#[inline(always)]
pub fn build_mask(addr: u32, len: usize) -> Option<Mask256> {
    if len == 0 || len > 256 { return None; }
    let k_start = (addr >> 12) as usize;
    let k_end = k_start + len - 1;
    let window_base = k_start & !255;
    if k_end >= window_base + 256 { return None; }
    let mut mask = [0u64; 4];
    for k in k_start..=k_end {
        let off = k - window_base;
        mask[off >> 6] |= 1u64 << (off & 63);
    }
    Some(Mask256(mask))
}

/// Scalar predicate P(addr,C) — 3c (1 LSR + 1 LDR + 1 TBZ)
#[inline(always)]
pub fn verify_scalar(addr: u32, vcap_base: *const u64) -> bool {
    let k = (addr >> 12) as usize;
    let off = k & 255; // within window
    let word = off >> 6;
    let bit = off & 63;
    unsafe { (*vcap_base.add(word) >> bit) & 1 == 1 }
}

/// Vector predicate — 1c for 256 bits: (Vcap & Mreq) == Mreq
#[inline(always)]
pub fn verify_vector(_addr: u32, mask: Mask256, vcap_base: *const u64) -> bool {
    unsafe {
        let v = core::slice::from_raw_parts(vcap_base, 4);
        let m = mask.0;
        (v[0] & m[0] == m[0]) && (v[1] & m[1] == m[1]) && (v[2] & m[2] == m[2]) && (v[3] & m[3] == m[3])
    }
}

pub mod audit {
    #[derive(Copy, Clone)] pub struct AuditEntry { pub addr: u32, pub val: u32, pub timestamp_cycles: u32 }
    pub struct AuditLog { pub buffer: [AuditEntry; 16], pub head: usize, pub count: usize }
    impl AuditLog { pub const fn new() -> Self { Self { buffer: [AuditEntry{addr:0,val:0,timestamp_cycles:0};16], head:0, count:0 } } }
}

pub mod registry {
    use super::*;
    #[repr(C, align(4))] pub struct RegistryBits(pub [AtomicU32; WORDS]);
    pub static REGISTRY_BITS: RegistryBits = RegistryBits([const { AtomicU32::new(0) }; WORDS]);
    #[inline(always)] pub fn available(id: usize) -> bool { REGISTRY_BITS.0[id/32].load(Ordering::Acquire) & (1 << (id % 32)) == 0 }
    #[inline(always)] pub fn acquire(id: usize) -> bool {
        let mask = 1u32 << (id % 32);
        REGISTRY_BITS.0[id/32].fetch_or(mask, Ordering::AcqRel) & mask == 0
    }
    #[inline(always)] pub fn release(id: usize) { REGISTRY_BITS.0[id/32].fetch_and(!(1u32 << (id%32)), Ordering::AcqRel); }
    #[inline(always)] pub fn is_claimed(id: usize) -> bool { !available(id) }

    /// Vector-enabled range check: tries 1c vector path, falls back to scalar loop.
    #[inline(always)]
    pub fn verify_range_contiguous(addr: u32, len: usize) -> bool {
        if let Some(mask) = build_mask(addr, len) {
            let window_base = ((addr >> 12) as usize) & !255;
            let u32_word = window_base >> 5;
            let u64_base = unsafe { REGISTRY_BITS.0.as_ptr().add(u32_word) as *const u64 };
            verify_vector(addr, mask, u64_base)
        } else {
            // scalar fallback
            for off in 0..len {
                let a = addr + (off as u32 * 4096);
                let k = (a >> 12) as usize % 256;
                if available(k) { return false; }
            }
            true
        }
    }
}

pub mod tokens {
    use core::marker::PhantomData;
    use super::registry;
    pub trait HardwareResource { const RESOURCE_ID: u16; const NAME: &'static str; }
    pub struct Cap<T: HardwareResource> { id: u16, _p: PhantomData<T> }
    pub fn claim<T: HardwareResource>() -> Option<Cap<T>> {
        if registry::acquire(T::RESOURCE_ID as usize) { Some(Cap{id:T::RESOURCE_ID, _p:PhantomData}) } else { None }
    }
    pub fn drop_cap<T: HardwareResource>(c: Cap<T>) { registry::release(c.id as usize); }
    pub struct GpioA; impl HardwareResource for GpioA { const RESOURCE_ID: u16 = 0; const NAME: &'static str = "GPIOA"; }
    pub struct GpioB; impl HardwareResource for GpioB { const RESOURCE_ID: u16 = 1; const NAME: &'static str = "GPIOB"; }
}

pub use registry::{acquire, available, release};
pub use tokens::{Cap, HardwareResource};
