//! Program persistence store (Phase 7).
//!
//! A fixed SRAM window (`_pstore_base .. _pstore_top`, 4K) holding up to
//! MAX_PROGRAMS named JIT images. ARM-only today: riscv32 DTIM is fully
//! carved and returns Err on all ops.
//!
//! Layout per slot: [name_len u8 | name 16B | len u32 | words len*u32]

use crate::kernel::memory::poke_u32;

pub const MAX_PROGRAMS: usize = 8;
pub const NAME_MAX: usize = 16;
pub const SLOT_WORDS: usize = 64;

extern "C" {
    static mut _pstore_base: u32;
    static mut _pstore_top: u32;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreError {
    NoStore,
    NotFound,
    Full,
    TooLarge,
}

/// Slot base address by index. Returns None when no store exists.
fn slot_addr(idx: usize) -> Option<usize> {
    let base = core::ptr::addr_of!(_pstore_base) as usize;
    let top = core::ptr::addr_of!(_pstore_top) as usize;
    if base == 0 || top == 0 || top <= base {
        return None;
    }
    Some(base + idx * (NAME_MAX + 1 + 4 + SLOT_WORDS * 4))
}

/// Write `name` + body words into slot `idx`.
pub fn save(idx: usize, name: &[u8], words: &[usize]) -> Result<(), StoreError> {
    let addr = slot_addr(idx).ok_or(StoreError::NoStore)?;
    if name.len() > NAME_MAX || words.len() > SLOT_WORDS {
        return Err(StoreError::TooLarge);
    }
    crate::kernel::memory::poke_u32(addr, name.len() as u32);
    for (i, &b) in name.iter().enumerate() {
        poke_u32(addr + 4 + i, b as u32);
    }
    crate::kernel::memory::poke_u32(addr + 4 + NAME_MAX, words.len() as u32);
    for (i, &w) in words.iter().enumerate() {
        crate::kernel::memory::poke_u32(addr + 4 + NAME_MAX + 4 + i * 4, w as u32);
    }
    Ok(())
}

/// Read slot into (name_len, name bytes, words written into out). Returns word count.
pub fn load(idx: usize, name_out: &mut [u8], words_out: &mut [usize]) -> Result<usize, StoreError> {
    let addr = slot_addr(idx).ok_or(StoreError::NoStore)?;
    let name_len = crate::kernel::memory::peek_u32(addr) as usize;
    if name_len == 0 || name_len > NAME_MAX {
        return Err(StoreError::NotFound); // empty slot
    }
    for (i, item) in name_out.iter_mut().enumerate().take(name_len) {
        *item = crate::kernel::memory::peek_u32(addr + 4 + i) as u8;
    }
    let count = crate::kernel::memory::peek_u32(addr + 4 + NAME_MAX) as usize;
    if count > SLOT_WORDS || count > words_out.len() {
        return Err(StoreError::TooLarge);
    }
    for (i, item) in words_out.iter_mut().enumerate().take(count) {
        *item = crate::kernel::memory::peek_u32(addr + 4 + NAME_MAX + 4 + i * 4) as usize;
    }
    Ok(count)
}
