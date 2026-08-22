//! hros-cap — O(1) linear capability engine.
//! See docs/technical/AXIS-3.md and src/capabilities/* (reference impl).
//! This crate will own REGISTRY_BITS @0x20001000 and Cap<T>/PinGuard.

#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]

pub mod audit {
    #[derive(Copy, Clone)] pub struct AuditEntry { pub addr: u32, pub val: u32, pub timestamp_cycles: u32 }
    pub struct AuditLog { pub buffer: [AuditEntry; 16], pub head: usize, pub count: usize }
    impl AuditLog { pub const fn new() -> Self { Self { buffer: [AuditEntry{addr:0,val:0,timestamp_cycles:0};16], head:0, count:0 } } }
}

pub mod registry {
    use core::sync::atomic::{AtomicU32, Ordering};
    pub const MAX_RESOURCES: usize = 256;
    #[repr(C, align(4))] pub struct RegistryBits(pub [AtomicU32; 8]);
    // Note: real placement is linker .capability_registry @0x20001000; bootstrap uses generic statics
    pub static REGISTRY: RegistryBits = RegistryBits([const { AtomicU32::new(0) }; 8]);
    #[inline(always)] pub fn available(id: usize) -> bool { REGISTRY.0[id/32].load(Ordering::Acquire) & (1 << (id%32)) == 0 }
    #[inline(always)] pub fn acquire(id: usize) -> bool { REGISTRY.0[id/32].fetch_or(1 << (id%32), Ordering::AcqRel) & (1 << (id%32)) == 0 }
    #[inline(always)] pub fn release(id: usize) { REGISTRY.0[id/32].fetch_and(!(1 << (id%32)), Ordering::AcqRel); }
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
