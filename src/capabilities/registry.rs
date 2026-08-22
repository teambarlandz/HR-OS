//! Fixed-size capability bitfield registry.
//!
//! One bit per hardware resource, resident in SRAM at `__capreg_base`
//! (SRAM + 0x1000) via the `.capability_registry` link section — matching
//! the address contract in docs/CHAPTER_02.
//!
//! All operations are O(1). Atomics are used instead of the doc's plain
//! read-modify-write so acquire/release stay correct even if an interrupt
//! handler races the REPL; on single-core silicon this compiles to simple
//! load/store-with-reservation sequences.

use core::sync::atomic::{AtomicU32, Ordering};

/// Total tracked resources (8 words x 32 bits).
pub const MAX_RESOURCES: usize = 256;

const WORDS: usize = MAX_RESOURCES / 32;

// ---------------------------------------------------------------------------
// Capability identifiers (one per hardware resource)
// ---------------------------------------------------------------------------

/// Hardware resource identifiers. Values match the bit positions in
/// [`REGISTRY_BITS`] and the indices used by [`tokens::resolve_name`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum CapId {
    GpioA = 0,
    GpioB = 1,
    Uart0 = 2,
    Spi0 = 3,
    I2c0 = 4,
    Timer0 = 5,
    Dma0 = 6,
    SuperUser = 31,
}

/// O(1) physical address → capability ID resolution.
///
/// Returns `None` for addresses that fall outside any peripheral region
/// (SRAM, flash, etc.) — those are unrestricted and require no capability.
///
/// SuperUser addresses map to `Some(CapId::SuperUser)` so the caller
/// can check whether the SuperUser token is active.
#[cfg(any(target_arch = "arm", target_arch = "riscv32"))]
#[inline(always)]
pub fn addr_to_cap_id(addr: u32) -> Option<CapId> {
    #[cfg(target_arch = "arm")]
    {
        arm_addr_to_cap(addr)
    }
    #[cfg(target_arch = "riscv32")]
    {
        riscv_addr_to_cap(addr)
    }
}

// ARM Cortex-M peripheral address ranges (STM32F405)
#[cfg(target_arch = "arm")]
#[inline(always)]
fn arm_addr_to_cap(addr: u32) -> Option<CapId> {
    match addr {
        0x4002_0000..=0x4002_03FF => Some(CapId::GpioA),
        0x4002_0400..=0x4002_07FF => Some(CapId::GpioB),
        0x4001_1000..=0x4001_13FF => Some(CapId::Uart0),
        0x4001_3000..=0x4001_33FF => Some(CapId::Spi0),
        0x4001_5400..=0x4001_57FF => Some(CapId::I2c0),
        0x4000_0000..=0x4000_03FF => Some(CapId::Timer0),
        0x4000_2000..=0x4000_23FF => Some(CapId::Dma0),
        _ => None,
    }
}

// RISC-V SiFive FE310 peripheral address ranges
#[cfg(target_arch = "riscv32")]
#[inline(always)]
fn riscv_addr_to_cap(addr: u32) -> Option<CapId> {
    match addr {
        0x1001_2000..=0x1001_2FFF => Some(CapId::GpioA),
        0x1001_3000..=0x1001_3FFF => Some(CapId::Uart0),
        0x1001_4000..=0x1001_4FFF => Some(CapId::Spi0),
        0x1002_0000..=0x1002_0FFF => Some(CapId::I2c0),
        0x1001_5000..=0x1001_5FFF => Some(CapId::Timer0),
        0x1000_0000..=0x1000_0FFF => Some(CapId::Dma0),
        _ => None,
    }
}

/// Check whether an address falls within any claimed capability, or is
/// unrestricted (SRAM / flash / unmapped). Returns `Ok(())` if access
/// is permitted, `Err(cap_id)` if the peripheral is not claimed.
///
/// SuperUser bypass is evaluated first. Fail-closed boundaries reject unmapped MMIO.
#[inline(always)]
pub fn check_access(addr: u32) -> Result<(), CapId> {
    if is_superuser_active() {
        return Ok(());
    }

    if let Some(cap_id) = addr_to_cap_id(addr) {
        if !is_claimed(cap_id as usize) {
            return Err(cap_id);
        }
        Ok(())
    } else {
        #[cfg(target_arch = "arm")]
        let is_ram_flash = matches!(addr, 0x0800_0000..=0x080F_FFFF | 0x2000_0000..=0x2001_C000);
        #[cfg(target_arch = "riscv32")]
        let is_ram_flash = matches!(addr, 0x2000_0000..=0x2000_FFFF | 0x8000_0000..=0x8000_FFFF);
        #[cfg(not(any(target_arch = "arm", target_arch = "riscv32")))]
        let is_ram_flash = true;

        if is_ram_flash {
            Ok(())
        } else {
            Err(CapId::SuperUser)
        }
    }
}

/// Returns true when the SuperUser capability is currently claimed.
#[inline(always)]
pub fn is_superuser_active() -> bool {
    !available(CapId::SuperUser as usize)
}

// ---------------------------------------------------------------------------
// Bitfield registry
// ---------------------------------------------------------------------------

/// Capability availability bitmap. Bit set = resource claimed.
///
/// Wrapped in a struct to carry `#[repr(align(4))]` (repr attributes do
/// not apply to statics directly).
#[repr(C, align(4))]
pub struct RegistryBits(pub [AtomicU32; WORDS]);

#[used]
#[link_section = ".capability_registry"]
pub static REGISTRY_BITS: RegistryBits = RegistryBits([const { AtomicU32::new(0) }; WORDS]);

/// Returns true when no owner holds `resource_id` (single-bit lookup).
#[inline(always)]
pub fn available(resource_id: usize) -> bool {
    let word = resource_id / 32;
    let bit = resource_id % 32;
    // SAFETY-free: bounds enforced by modulo against a compile-time-sized
    // array; index < WORDS always holds for resource_id < MAX_RESOURCES.
    match REGISTRY_BITS.0.get(word) {
        Some(w) => w.load(Ordering::Acquire) & (1u32 << bit) == 0,
        None => false,
    }
}

/// Returns true when `resource_id` is claimed (bit is set).
#[inline(always)]
pub fn is_claimed(resource_id: usize) -> bool {
    !available(resource_id)
}

/// Atomically claim `resource_id`. Returns false if already claimed.
///
/// O(1): one fetch-or test-and-set. On loss the bit is left set (it was
/// already set by the winner), so state stays consistent.
#[inline(always)]
pub fn acquire(resource_id: usize) -> bool {
    let word = resource_id / 32;
    let bit = resource_id % 32;
    match REGISTRY_BITS.0.get(word) {
        Some(w) => {
            let mask = 1u32 << bit;
            let prev = w.fetch_or(mask, Ordering::AcqRel);
            prev & mask == 0
        }
        None => false,
    }
}

/// Atomically release `resource_id` (clear its bit). O(1).
#[inline(always)]
pub fn release(resource_id: usize) {
    let word = resource_id / 32;
    let bit = resource_id % 32;
    if let Some(w) = REGISTRY_BITS.0.get(word) {
        w.fetch_and(!(1u32 << bit), Ordering::AcqRel);
    }
}

// ---------------------------------------------------------------------------
// Vector Capability Engine — 256-bit SIMD Upgrade (Phase 2, UPGRADE.md Step 1)
// Scalar 3c → Vector 1c for 1 MiB (256×4K blocks)
// ---------------------------------------------------------------------------

/// 256-bit request mask (4×64) vs task vector `Vcap ∈ {0,1}²⁵⁶`.
/// `authorized = (Vcap & Mreq) == Mreq` — single vector AND + test.
#[repr(C, align(32))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mask256(pub [u64; 4]);

impl Mask256 {
    /// Zero mask (no bits set).
    pub const ZERO: Self = Self([0; 4]);
}

/// Build `Mask256` for contiguous `len` blocks starting at `addr` (len ≤256).
/// Returns `None` if `len == 0` or `len > 256` or range wraps.
///
/// Construction: `k_start = addr >> 12`, `k_end = k_start + len - 1`,
/// then set bits `k_start..=k_end` in the 256-bit mask.
/// Handles word-boundary straddle with 64-bit word + bit offsets.
#[inline(always)]
pub fn build_mask(addr: u32, len: usize) -> Option<Mask256> {
    if len == 0 || len > 256 {
        return None;
    }
    let k_start = (addr >> 12) as usize;
    let k_end = k_start + len - 1;
    // For Phase 2, mask is relative to the 256-window containing k_start.
    // Caller must ensure `k_end - (k_start & !255) < 256` (single window).
    // For simplicity, we build mask for low 256 bits relative to k_start's window base.
    let window_base = k_start & !255usize;
    if k_end >= window_base + 256 {
        // Spans >256 window — scalar fallback will handle via loop; for vector path, reject
        return None;
    }
    let mut mask = [0u64; 4];
    for k in k_start..=k_end {
        let offset = k - window_base;
        let word = offset >> 6; // /64
        let bit = offset & 63;
        mask[word] |= 1u64 << bit;
    }
    Some(Mask256(mask))
}

/// Scalar predicate `P(addr,C)` — single 4 KiB block check, 3c.
/// `k = addr >> 12; idx = k >> 5 (for u32) or k >> 6 (for u64 view); bit = k & 31/63`
#[inline(always)]
pub fn verify_scalar(addr: u32) -> bool {
    let k = (addr >> 12) as usize;
    // Use 32-bit view: word = k/32, bit = k%32
    let word = k >> 5;
    let bit = k & 31;
    match REGISTRY_BITS.0.get(word) {
        Some(w) => (w.load(Ordering::Acquire) >> bit) & 1 == 1,
        None => false,
    }
}

/// Vector predicate for contiguous `len` blocks — 1c for 256 blocks.
/// `authorized = (Vcap & Mreq) == Mreq` across 4×u64.
/// `vcap_base` must point to window base `&REGISTRY_BITS.0[window_base/32]` as `*const u64`.
#[inline(always)]
pub fn verify_vector(_addr: u32, mask: Mask256, vcap_base: *const u64) -> bool {
    // SAFETY: vcap_base is 4×u64 window base, 32B aligned (Mask256 align 32).
    // For host tests, caller may pass &REGISTRY_BITS as *const u64 with window_base=0.
    unsafe {
        let vcap = core::slice::from_raw_parts(vcap_base, 4);
        let m = mask.0;
        // Scalar loop over 4 — on x86_64 with AVX2 this optimizes to VANDPS+VPTEST
        // when compiled with `target-feature=+avx2` (see hros-arch-x86).
        // For Phase 2, this is the portable 1c-equivalent fallback.
        (vcap[0] & m[0] == m[0]) && (vcap[1] & m[1] == m[1]) && (vcap[2] & m[2] == m[2]) && (vcap[3] & m[3] == m[3])
    }
}

/// Verify contiguous range `[addr, addr+len*4096)` is fully authorized.
/// Tries vector path for len ≤256 and window-aligned, else scalar fallback loop.
/// Returns `Ok(())` if all bits set, `Err(first_missing_cap)` otherwise.
#[inline(always)]
pub fn verify_range_contiguous(addr: u32, len: usize) -> Result<(), CapId> {
    if len == 0 {
        return Ok(());
    }
    if is_superuser_active() {
        return Ok(());
    }
    // Try vector fast path
    if let Some(mask) = build_mask(addr, len) {
        let window_base = ((addr >> 12) as usize) & !255;
        // Convert registry as u64 window base: word = window_base/32, as u64 index = word/2
        let u32_word = window_base >> 5;
        let u64_base = unsafe { REGISTRY_BITS.0.as_ptr().add(u32_word) as *const u64 };
        if verify_vector(addr, mask, u64_base) {
            return Ok(());
        } else {
            // Find first missing to report CapId (scalar scan for error detail)
            for offset in 0..len {
                let a = addr + (offset as u32 * 4096);
                if let Some(cap) = addr_to_cap_id(a) {
                    if !is_claimed(cap as usize) {
                        return Err(cap);
                    }
                } else if !matches!(a, 0x0800_0000..=0x080F_FFFF | 0x2000_0000..=0x2001_C000 | 0x8000_0000..=0x8000_FFFF) {
                    // Host fallback: on x86_64 host, treat unmapped as SuperUser
                    if !is_claimed(CapId::SuperUser as usize) {
                        return Err(CapId::SuperUser);
                    }
                }
            }
            return Err(CapId::SuperUser);
        }
    }
    // Scalar fallback: check each block individually (O(len) but len ≤256)
    for offset in 0..len {
        let a = addr + (offset as u32 * 4096);
        check_access(a)?;
    }
    Ok(())
}

// Host fallback for addr_to_cap_id when not on arm/riscv32 (for cargo test on x86_64)
#[cfg(not(any(target_arch = "arm", target_arch = "riscv32")))]
#[inline(always)]
fn arm_addr_to_cap(_addr: u32) -> Option<CapId> { None }
#[cfg(not(any(target_arch = "arm", target_arch = "riscv32")))]
#[inline(always)]
fn riscv_addr_to_cap(_addr: u32) -> Option<CapId> { None }
#[cfg(not(any(target_arch = "arm", target_arch = "riscv32")))]
#[inline(always)]
pub fn addr_to_cap_id(_addr: u32) -> Option<CapId> { None }
