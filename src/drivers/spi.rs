//! SPI1 master driver (Phase 6). Capability: `CapId::Spi0`
//! (`0x4001_3000..=0x4001_33FF`). QEMU models SPI1 via the SSI device.

use crate::kernel::memory::{peek_u32, poke_u32};

/// SPI base, per architecture.
/// - ARM STM32F405: SPI1 @0x4001_3000
/// - RISC-V SiFive E310: SPI0 @0x1001_4000
#[cfg(target_arch = "riscv32")]
pub const SPI1_BASE: usize = 0x1001_4000;
#[cfg(not(target_arch = "riscv32"))]
pub const SPI1_BASE: usize = 0x4001_3000;

pub mod regs {
    use super::SPI1_BASE;
    #[cfg(target_arch = "riscv32")]
    pub const CTRL: usize = SPI1_BASE;
    #[cfg(target_arch = "riscv32")]
    pub const TXDATA: usize = SPI1_BASE + 0x18;
    #[cfg(target_arch = "riscv32")]
    pub const RXDATA: usize = SPI1_BASE + 0x20;
    #[cfg(not(target_arch = "riscv32"))]
    pub const CR1: usize = SPI1_BASE;
    #[cfg(not(target_arch = "riscv32"))]
    pub const SR: usize = SPI1_BASE + 0x08;
    #[cfg(not(target_arch = "riscv32"))]
    pub const DR: usize = SPI1_BASE + 0x0C;
}

pub mod rcc {
    /// ARM only; SiFive peripherals are always clocked.
    #[cfg(not(target_arch = "riscv32"))]
    pub const APB2ENR: usize = 0x4002_3800 + 0x44;
    #[cfg(not(target_arch = "riscv32"))]
    pub const SPI1EN: u32 = 1 << 12;
}

/// SR status bits used by this driver.
pub const SR_TXE: u32 = 1 << 1;
pub const SR_RXNE: u32 = 1 << 0;
pub const SR_BSY: u32 = 1 << 7;

/// Enable SPI1 clock.
#[inline(always)]
pub fn enable_clock() {
    #[cfg(not(target_arch = "riscv32"))]
    {
        let apb2 = peek_u32(rcc::APB2ENR);
        poke_u32(rcc::APB2ENR, apb2 | rcc::SPI1EN);
    }
}

/// Configure SPI1 as master, baud = fPCLK/`br_div_log2`, 8-bit frames.
/// `br_div_log2`: 0=fPCLK/2 .. 7=fPCLK/256.
pub fn configure(br_div_log2: u8) {
    enable_clock();
    #[cfg(target_arch = "riscv32")]
    {
        // SiFive SCKDIV bits [18:16]; polarity/phase default 0.
        poke_u32(regs::CTRL, ((br_div_log2 as u32) & 0x7) << 16);
    }
    #[cfg(not(target_arch = "riscv32"))]
    {
        let cr1 = (1 << 2) | (((br_div_log2 as u32) & 0x7) << 3);
        poke_u32(regs::CR1, cr1);
        poke_u32(regs::CR1, cr1 | (1 << 6));
    }
}

/// Blocking full-duplex transfer of one byte. Returns received byte.
///
/// QEMU note: with no slave attached the RX path returns whatever the SSI
/// model latches; the contract here is TX completes and RXNE drains.
pub fn transfer_byte(byte: u8) -> u8 {
    #[cfg(target_arch = "riscv32")]
    {
        // SiFive: write txdata; rxdata bit31 empty-flag clears when filled.
        poke_u32(regs::TXDATA, byte as u32);
        loop {
            let rx = peek_u32(regs::RXDATA);
            if rx & (1 << 31) == 0 {
                return (rx & 0xFF) as u8;
            }
            core::hint::spin_loop();
        }
    }
    #[cfg(not(target_arch = "riscv32"))]
    {
        while peek_u32(regs::SR) & SR_TXE == 0 {
            core::hint::spin_loop();
        }
        poke_u32(regs::DR, byte as u32);
        while peek_u32(regs::SR) & SR_RXNE == 0 {
            core::hint::spin_loop();
        }
        (peek_u32(regs::DR) & 0xFF) as u8
    }
}

/// Readback pair for verification: (CR1, SR).
pub fn snapshot() -> (u32, u32) {
    #[cfg(target_arch = "riscv32")]
    return (peek_u32(regs::CTRL), peek_u32(regs::TXDATA));
    #[cfg(not(target_arch = "riscv32"))]
    return (peek_u32(regs::CR1), peek_u32(regs::SR));
}
