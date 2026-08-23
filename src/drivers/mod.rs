//! Hardware Abstraction Layer & REPL interfaces.
//!
//! - [`uart`] — bare-metal UART driver with static RX ring buffer.
//! - [`repl`] — ASCII terminal state machine and command handler.
//! - [`pcie`] — ECAM enumerator + Autonomous DMA Ring (Axis 2).
//! - [`timer`] — SysTick/APIC/mtime driver (Axis 1).

pub mod flash;
pub mod pcie;
pub mod pstore;
pub mod pwm;
pub mod repl;
pub mod spi;
pub mod timer;
pub mod uart;
