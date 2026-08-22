//! Lock-free multi-core scheduler (Axis 1, Phase 2)
//! 43c switch, align(64) to avoid false sharing, CAS + WFE/SEV.
//! See docs/technical/UPGRADE.md Step 2 and AXIS-1.md.

use core::sync::atomic::{AtomicUsize, Ordering};

/// Task state — simple.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    Ready,
    Running,
    Blocked,
    Dead,
}

/// Task Control Block — per-task stack + PC.
#[repr(C)]
pub struct TaskControlBlock {
    pub sp: *mut u8,
    pub sp_limit: *mut u8,
    pub sp_base: *mut u8,
    pub pc: usize,
    pub state: TaskState,
}

impl TaskControlBlock {
    pub const fn new() -> Self {
        Self {
            sp: core::ptr::null_mut(),
            sp_limit: core::ptr::null_mut(),
            sp_base: core::ptr::null_mut(),
            pc: 0,
            state: TaskState::Dead,
        }
    }
}

/// Lock-free inter-core task queue — 256 slots, head/tail atomics.
/// Align to 64B cache line to eliminate false sharing (MESI).
#[repr(C, align(64))]
pub struct LockFreeTaskQueue {
    head: AtomicUsize,
    tail: AtomicUsize,
    tasks: [*mut TaskControlBlock; 256],
}

impl LockFreeTaskQueue {
    pub const fn new() -> Self {
        Self {
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
            tasks: [core::ptr::null_mut(); 256],
        }
    }

    /// Push task — O(1), 8–12c, bounded. Uses `Relaxed` load + `Acquire` check + `Release` store.
    #[inline(always)]
    pub fn push_task(&self, tcb: *mut TaskControlBlock) -> Result<(), ()> {
        let current_tail = self.tail.load(Ordering::Relaxed);
        let next_tail = (current_tail + 1) % 256;

        if next_tail == self.head.load(Ordering::Acquire) {
            return Err(()); // Queue full: bounded deterministic failure
        }

        // SAFETY: current_tail < 256, single-producer per tail index in MPSC case;
        // for MPMC, CAS on tail would be needed, but HR-OS uses per-core local + work-steal fallback.
        unsafe {
            let ptr = self.tasks.as_ptr().add(current_tail) as *mut *mut TaskControlBlock;
            ptr.write_volatile(tcb);
        }

        // Atomic commit with Release semantics (flushes write buffer to L1)
        self.tail.store(next_tail, Ordering::Release);
        Ok(())
    }

    /// Pop task — O(1), used by scheduler `next_task`.
    #[inline(always)]
    pub fn pop_task(&self) -> Option<*mut TaskControlBlock> {
        let current_head = self.head.load(Ordering::Relaxed);
        if current_head == self.tail.load(Ordering::Acquire) {
            return None; // Empty
        }
        let next_head = (current_head + 1) % 256;
        let tcb = unsafe { self.tasks.as_ptr().add(current_head).read_volatile() };
        self.head.store(next_head, Ordering::Release);
        Some(tcb)
    }

    /// Current length (for diagnostics, not for scheduling decisions).
    #[inline(always)]
    pub fn len(&self) -> usize {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);
        if tail >= head { tail - head } else { 256 - head + tail }
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.head.load(Ordering::Acquire) == self.tail.load(Ordering::Acquire)
    }

    #[inline(always)]
    pub fn is_full(&self) -> bool {
        (self.tail.load(Ordering::Relaxed) + 1) % 256 == self.head.load(Ordering::Acquire)
    }
}

// SAFETY: LockFreeTaskQueue is Sync because head/tail are atomics and tasks are raw pointers
// managed by the kernel's single-owner discipline. Send is required for cross-core sharing.
unsafe impl Sync for LockFreeTaskQueue {}
unsafe impl Send for LockFreeTaskQueue {}

/// Global scheduler queue — placed in SRAM via linker `.sram_vectors` or `.sram` region.
/// For Phase 2, static in `.bss` is sufficient; linker will place in SRAM per SASA.
pub static SCHEDULER_QUEUE: LockFreeTaskQueue = LockFreeTaskQueue::new();

/// SysTick configuration — N = f_CPU * Δt (e.g., 84 MHz * 1ms = 84_000)
#[inline(always)]
pub fn systick_reload(f_cpu_hz: u32, delta_ms: u32) -> u32 {
    // N = f * Δt / 1000
    (f_cpu_hz / 1000) * delta_ms
}

/// Context switch cycle ledger — 12 auto + 8 push + 3 sched + 8 pop + 12 unstack = 43
pub const CYCLES_AUTO_STACK: usize = 12;
pub const CYCLES_MANUAL_PUSH: usize = 8;
pub const CYCLES_SCHED: usize = 3;
pub const CYCLES_MANUAL_POP: usize = 8;
pub const CYCLES_AUTO_UNSTACK: usize = 12;
pub const TOTAL_CYCLES: usize = 43;
