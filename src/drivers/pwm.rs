//! TIM2 PWM driver (Axis 2 peripheral surface, Phase 6).
//!
//! Capability contract: every entry point requires `CapId::Timer0` claimed
//! (`0x4000_0000..=0x4000_03FF` in the registry address map). Registers are
//! programmed directly — no PAC, no HAL crate, pure volatile MMIO.

use crate::kernel::memory::{peek_u32, poke_u32};

/// Timer base, per architecture.
/// - ARM STM32F405: TIM2 @0x4000_0000 (Timer0 capability region)
/// - RISC-V SiFive E310: PWM0 @0x1001_5000 (same capability region)
#[cfg(target_arch = "riscv32")]
pub const TIM2_BASE: usize = 0x1001_5000;
#[cfg(not(target_arch = "riscv32"))]
pub const TIM2_BASE: usize = 0x4000_0000;

pub mod regs {
    use super::TIM2_BASE;
    pub const CR1: usize = TIM2_BASE;
    pub const DIER: usize = TIM2_BASE + 0x0C;
    pub const SR: usize = TIM2_BASE + 0x10;
    pub const PSC: usize = TIM2_BASE + 0x28;
    pub const ARR: usize = TIM2_BASE + 0x2C;
    pub const CCMR1: usize = TIM2_BASE + 0x18;
    pub const CCER: usize = TIM2_BASE + 0x20;
    pub const CCR1: usize = TIM2_BASE + 0x34;
}

pub mod rcc {
    /// ARM: RCC.APB1ENR bit0 enables TIM2. RISC-V: no gate needed.
    #[cfg(not(target_arch = "riscv32"))]
    pub const APB1ENR: usize = 0x4002_3800 + 0x40;
    #[cfg(not(target_arch = "riscv32"))]
    pub const TIM2EN: u32 = 1 << 0;
}

/// Enable TIM2 clock in RCC.APB1ENR.
#[inline(always)]
pub fn enable_clock() {
    #[cfg(not(target_arch = "riscv32"))]
    {
        let apb1 = peek_u32(rcc::APB1ENR);
        poke_u32(rcc::APB1ENR, apb1 | rcc::TIM2EN);
    }
}

/// Configure TIM2 CH1 as PWM output with `period_ticks` reload and `duty_ticks`
/// compare value. Returns `(arr, ccr1)` written, for caller verification.
///
/// # Safety-free notes
/// All accesses are volatile MMIO within the Timer0 capability region; caller
/// must hold `CapId::Timer0` (enforced one layer up in the parser).
pub fn configure(period_ticks: u32, duty_ticks: u32) -> (u32, u32) {
    enable_clock();

    // Gate time-base during config.
    poke_u32(regs::CR1, 0);

    // Prescaler 0 => f_clk = f_timer; period from caller.
    poke_u32(regs::PSC, 0);
    poke_u32(regs::ARR, period_ticks);

    // CH1: output compare, PWM mode 1 (OC1M=110), preload off.
    let ccmr = peek_u32(regs::CCMR1);
    poke_u32(regs::CCMR1, (ccmr & !0x007F) | (0b110 << 4));
    // Enable CH1 output.
    let ccer = peek_u32(regs::CCER);
    poke_u32(regs::CCER, ccer | (1 << 0));
    poke_u32(regs::CCR1, duty_ticks);

    // Up-counter, enable.
    poke_u32(regs::CR1, 1);
    (period_ticks, duty_ticks)
}

/// Update compare value live (duty change without reconfig).
pub fn set_duty(duty_ticks: u32) {
    poke_u32(regs::CCR1, duty_ticks);
}

/// Readback triple for verification: (ARR, CCR1, CR1).
pub fn snapshot() -> (u32, u32, u32) {
    (
        peek_u32(regs::ARR),
        peek_u32(regs::CCR1),
        peek_u32(regs::CR1),
    )
}
