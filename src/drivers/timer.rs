//! SysTick / APIC / mtime driver (Axis 1, Phase 2/3)
//! N = f_CPU * Δt, 84 MHz * 1ms = 84_000 ticks, 1c window

/// Calculate reload value N for given frequency and delta_ms
#[inline(always)]
pub fn systick_reload(f_cpu_hz: u32, delta_ms: u32) -> u32 {
    (f_cpu_hz / 1000) * delta_ms
}

/// ARM SysTick MMIO (Cortex-M)
#[cfg(target_arch = "arm")]
pub mod arm {
    pub const STK_CTRL: usize = 0xE000E010;
    pub const STK_LOAD: usize = 0xE000E014;
    pub const STK_VAL: usize = 0xE000E018;
    pub const CTRL_ENABLE: u32 = 0x01;
    pub const CTRL_TICKINT: u32 = 0x02;
    pub const CTRL_CLKSOURCE: u32 = 0x04;
    pub const CTRL_BITS: u32 = CTRL_ENABLE | CTRL_TICKINT | CTRL_CLKSOURCE; // 0x07

    /// Configure SysTick for `ticks` reload, returns ticks
    #[inline(always)]
    #[allow(clippy::missing_safety_doc)]
    pub unsafe fn configure(ticks: u32) {
        unsafe {
            core::ptr::write_volatile(STK_LOAD as *mut u32, ticks);
            core::ptr::write_volatile(STK_VAL as *mut u32, 0);
            core::ptr::write_volatile(STK_CTRL as *mut u32, CTRL_BITS);
            core::arch::asm!("dsb", "isb", options(nostack));
        }
    }
}

/// RISC-V CLINT mtime (SiFive)
#[cfg(target_arch = "riscv32")]
pub mod riscv {
    pub const MTIME: usize = 0x0200BFF8;
    pub const MTIMECMP: usize = 0x02004000;
    #[inline(always)]
    #[allow(clippy::missing_safety_doc)]
    pub unsafe fn configure(_ticks: u32) {
        unsafe { core::arch::asm!("fence.i", options(nostack)) }
    }
}
