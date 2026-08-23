//! STM32F4 flash programming (Phase 7 — program persistence).
//!
//! Unlocks FPEC, erases sector SNB, programs words. On QEMU's stm32f4xx
//! model the FPEC may be a stub; callers must verify by read-back.

use crate::kernel::memory::{peek_u32, poke_u32};

pub const FLASH_BASE: usize = 0x0800_0000;
/// Program store: last 16K of the mapped 128K flash.
pub const STORE_BASE: usize = FLASH_BASE + 0x1C000;
pub const STORE_LEN: usize = 16 * 1024;

mod regs {
    pub const KEYR: usize = 0x4002_3C04;
    pub const SR: usize = 0x4002_3C0C;
    pub const CR: usize = 0x4002_3C10;
}

const KEY1: u32 = 0x4567_0123;
const KEY2: u32 = 0xCDEF_89AB;
const CR_LOCK: u32 = 1 << 31;
const CR_PG: u32 = 1 << 0;
const CR_SER: u32 = 1 << 1;
const CR_STRT: u32 = 1 << 16;
const SR_BSY: u32 = 1 << 16;

fn wait_not_busy() {
    while peek_u32(regs::SR) & SR_BSY != 0 {
        core::hint::spin_loop();
    }
}

/// Unlock FPEC. Idempotent.
pub fn unlock() {
    if peek_u32(regs::CR) & CR_LOCK != 0 {
        poke_u32(regs::KEYR, KEY1);
        poke_u32(regs::KEYR, KEY2);
    }
}

pub fn lock() {
    poke_u32(regs::CR, peek_u32(regs::CR) | CR_LOCK);
}

/// Erase the program-store sector (SNB selects sector index; store lives in
/// the last 16K sector of the 128K window => sector index depends on layout).
/// Returns true when BSY clears without error bit set.
pub fn erase_store_sector(sector_index: u8) -> bool {
    unlock();
    wait_not_busy();
    let cr = peek_u32(regs::CR);
    // SER + SNB[4:7] + STRT
    poke_u32(
        regs::CR,
        (cr & !(0xF << 3)) | CR_SER | (((sector_index as u32) & 0xF) << 3),
    );
    poke_u32(regs::CR, peek_u32(regs::CR) | CR_STRT);
    wait_not_busy();
    let ok = peek_u32(regs::SR) & (1 << 4 | 1 << 5) == 0; // EOP cleared separately; ERR bits 4-5
    poke_u32(regs::CR, cr & !CR_SER);
    ok
}

/// Program one word at `STORE_BASE + byte_off` (must be 4-byte aligned).
pub fn program_word(byte_off: usize, word: u32) {
    unlock();
    wait_not_busy();
    poke_u32(regs::CR, peek_u32(regs::CR) | CR_PG);
    unsafe {
        core::ptr::write_volatile((STORE_BASE + byte_off) as *mut u32, word);
    }
    wait_not_busy();
    poke_u32(regs::CR, peek_u32(regs::CR) & !CR_PG);
}

/// Verify whether the model honors programming: write marker, read back.
pub fn self_test() -> bool {
    unlock();
    erase_store_sector(11); // 128K image: sectors 0..=3 x16K, 4 x64K, 5..=11 x128K...
    program_word(0, 0xCAFE_F00D);
    let v = peek_u32(STORE_BASE);
    lock();
    v == 0xCAFE_F00D
}
