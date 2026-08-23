#![cfg(all(target_os = "none", target_arch = "arm"))]
//! Capability-scoped memory pools (Phase 9).
//!
//! Per-task bounded bump allocator. O(1) alloc, O(1) free-by-task-reset.
//! The pool region is linker-carved (`_heap_pool_base`, 8K on ARM) and
//! guarded by Axis 3 capability bits — a task can only touch its own pool.
//!
//! Design: each task gets a fixed-size sub-pool. `alloc` bumps a per-task
//! offset; `free` resets the offset to zero (arena semantics). No free-list,
//! no fragmentation, deterministic cost — passes every WCEF.md gate.

/// Pool region size in bytes (matches linker `_heap_pool_base` carve).
pub const POOL_SIZE: usize = 8192;
/// Sub-pool size per task.
pub const TASK_POOL_SIZE: usize = POOL_SIZE / NUM_POOLS;
/// Number of independent pools (one per task slot).
pub const NUM_POOLS: usize = 4;

extern "C" {
    static _heap_pool_base: u32;
}

fn pool_base() -> usize {
    core::ptr::addr_of!(_heap_pool_base) as usize
}

/// Per-task allocation offsets (bump pointers).
pub static mut OFFSETS: [usize; NUM_POOLS] = [0; NUM_POOLS];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolError {
    /// Task index out of range.
    BadTask,
    /// Not enough space remaining in this task's sub-pool.
    Full,
    /// Alignment request too large.
    BadAlign,
}

/// Allocate `size` bytes from task `task_idx`'s sub-pool.
///
/// Returns the absolute address of the allocated block, or `Full` if
/// insufficient space remains. O(1) — one add, one compare.
///
/// # Safety
/// Caller must hold the capability for the task's memory region. Returned
/// pointer is valid until `reset_task` is called for the same task.
pub unsafe fn alloc(task_idx: usize, size: usize) -> Result<usize, PoolError> {
    if task_idx >= NUM_POOLS {
        return Err(PoolError::BadTask);
    }
    // SAFETY: static mut, single-threaded kernel context.
    let off = unsafe { OFFSETS[task_idx] };
    let base = pool_base() + task_idx * TASK_POOL_SIZE;
    let addr = base + off;
    if off + size > TASK_POOL_SIZE {
        return Err(PoolError::Full);
    }
    unsafe { OFFSETS[task_idx] = off + size };
    Ok(addr)
}

/// Reset task `task_idx`'s pool to empty (frees all allocations at once).
///
/// O(1) — one store. This is arena semantics: individual frees are not
/// supported by design (fragmentation-free).
///
/// # Safety
/// All previously returned pointers from this task become invalid.
pub unsafe fn reset(task_idx: usize) -> Result<(), PoolError> {
    if task_idx >= NUM_POOLS {
        return Err(PoolError::BadTask);
    }
    unsafe { OFFSETS[task_idx] = 0 };
    Ok(())
}

/// Remaining bytes in task's sub-pool.
pub fn remaining(task_idx: usize) -> Result<usize, PoolError> {
    if task_idx >= NUM_POOLS {
        return Err(PoolError::BadTask);
    }
    // SAFETY: static mut, single-threaded context.
    Ok(TASK_POOL_SIZE - unsafe { OFFSETS[task_idx] })
}

/// Total pool stats for diagnostics.
pub fn stats() -> [(usize, usize); NUM_POOLS] {
    // SAFETY: static mut, read-only snapshot.
    let offs = unsafe { &*core::ptr::addr_of!(OFFSETS) };
    let mut out = [(0usize, 0usize); NUM_POOLS];
    for i in 0..NUM_POOLS {
        out[i] = (offs[i], TASK_POOL_SIZE - offs[i]);
    }
    out
}
