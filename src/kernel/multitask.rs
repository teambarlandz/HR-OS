//! Preemptive multi-tasking (Phase 8a, ARM only).
//!
//! SysTick → PendSV → context switch between REPL task and counter tasks.
//! Proves Axis 1's 43-cycle switch end-to-end. See PROSPECTIVE-HARDWARE-TESTS.md C1.

#![allow(clippy::fn_to_numeric_cast)]

use core::arch::global_asm;

// ---------------------------------------------------------------------------
// PendSV handler + counter task bodies (pure asm)
// ---------------------------------------------------------------------------

pub const NUM_TASKS: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum TaskState {
    Dead = 0,
    Ready = 1,
    Running = 2,
}

/// TCB — repr(C) matches the asm field offsets:
/// sp=+0, pc=+4, state=+8, counter=+12, total=16 bytes.
#[repr(C)]
pub struct Tcb {
    pub sp: u32,
    pub pc: u32,
    pub state: u32,
    pub counter: u32,
}

impl Tcb {
    pub const fn dead() -> Self {
        Self {
            sp: 0,
            pc: 0,
            state: 0,
            counter: 0,
        }
    }
}

pub static mut CURRENT_TASK: usize = 0;
pub static mut TCBS: [Tcb; NUM_TASKS + 1] = [
    Tcb::dead(),
    Tcb::dead(),
    Tcb::dead(),
    Tcb::dead(),
    Tcb::dead(),
];

global_asm!(
    ".section .text.PendSV_Handler, \"ax\"",
    ".thumb_func",
    ".global PendSV_Handler",
    "PendSV_Handler:",
    // Scan for a Ready task != current
    "   ldr r1, ={current}",
    "   ldr r2, [r1]",
    "   movs r6, #0",
    "8:",
    "   cmp r6, r2",
    "   beq 9f",
    "   ldr r3, ={tcbs}",
    "   movs r4, #16",
    "   lsls r5, r6, #4",
    "   adds r3, r3, r5",
    "   ldr r5, [r3, #8]",
    "   cmp r5, #1",
    "   beq 1f",
    "9:",
    "   adds r6, r6, #1",
    "   cmp r6, #5",
    "   blt 8b",
    // No other ready: return immediately (zero side effects)
    "   bx lr",
    // ---- Do the switch ----
    "1:",
    "   mrs r0, psp",               // current PSP
    "   stmdb r0!, {{r4-r11}}",     // save callee-saved regs
    "   ldr r3, ={tcbs}",           // save SP to current TCB
    "   movs r4, #16",
    "   lsls r5, r2, #4",
    "   adds r3, r3, r5",
    "   str r0, [r3]",              // tcbs[cur].sp = new SP
    "   movs r5, #1",               // Ready
    "   str r5, [r3, #8]",          // tcbs[cur].state = Ready
    // Load next task
    "   ldr r1, ={current}",
    "   str r6, [r1]",              // CURRENT_TASK = next idx
    "   ldr r3, ={tcbs}",
    "   movs r4, #16",
    "   lsls r5, r6, #4",
    "   adds r3, r3, r5",
    "   ldr r0, [r3]",              // next task's saved SP
    "   ldmia r0!, {{r4-r11}}",     // restore callee-saved regs
    "   msr psp, r0",               // switch PSP
    "   movs r5, #2",               // Running
    "   str r5, [r3, #8]",          // tcbs[next].state = Running
    // Increment progress counter (offset 12)
    "   ldr r6, [r3, #12]",
    "   adds r6, r6, #1",
    "   str r6, [r3, #12]",
    // EXC_RETURN: Thread mode, PSP, no FPU
    "   ldr r0, ={exc_ret}",
    "   bx r0",
    current = sym crate::kernel::multitask::CURRENT_TASK,
    tcbs = sym crate::kernel::multitask::TCBS,
    exc_ret = const 0xFFFFFFFD_u32,
);

global_asm!(
    ".section .text.task_body_1, \"ax\"",
    ".thumb_func",
    ".global task_body_1",
    "task_body_1:",
    "   ldr r0, ={tcbs}",
    "1:",
    "   ldr r1, [r0, #28]",         // TCBS[1].counter = 1*16+12 = 28
    "   adds r1, r1, #1",
    "   str r1, [r0, #28]",
    "   b 1b",
    tcbs = sym crate::kernel::multitask::TCBS,
);

global_asm!(
    ".section .text.task_body_2, \"ax\"",
    ".thumb_func",
    ".global task_body_2",
    "task_body_2:",
    "   ldr r0, ={tcbs}",
    "1:",
    "   ldr r1, [r0, #44]",         // TCBS[2].counter = 2*16+12 = 44
    "   adds r1, r1, #1",
    "   str r1, [r0, #44]",
    "   b 1b",
    tcbs = sym crate::kernel::multitask::TCBS,
);

extern "C" {
    pub fn PendSV_Handler();
    pub fn task_body_1();
    pub fn task_body_2();
}

// ---------------------------------------------------------------------------
// Rust-side task management
// ---------------------------------------------------------------------------

extern "C" {
    static _task_stack_1: u32;
    static _task_stack_2: u32;
    static _task_stack_3: u32;
}

fn task_stack_top(idx: usize) -> u32 {
    match idx {
        1 => core::ptr::addr_of!(_task_stack_1) as u32,
        2 => core::ptr::addr_of!(_task_stack_2) as u32,
        3 => core::ptr::addr_of!(_task_stack_3) as u32,
        _ => core::ptr::addr_of!(_task_stack_1) as u32,
    }
}

/// Spawn a counter task that proves context switching works.
///
/// # Safety
/// Call after VTOR relocation and install_task_handlers().
pub unsafe fn spawn_counter_task(task_idx: usize) -> bool {
    if task_idx >= NUM_TASKS - 1 {
        return false;
    }
    let tcbs = core::ptr::addr_of_mut!(TCBS);
    let stack_top = task_stack_top(task_idx + 1);

    let body: unsafe extern "C" fn() = if task_idx == 0 {
        task_body_1
    } else {
        task_body_2
    };
    let body_addr = body as *const () as u32;

    // Initial exception return frame on task's stack.
    let sp = stack_top;
    unsafe {
        core::ptr::write_volatile((sp - 4) as *mut u32, 0x0100_0000); // xPSR Thumb
        core::ptr::write_volatile((sp - 8) as *mut u32, body_addr | 1); // PC thumb
        for off in [12u32, 16, 20, 24, 28, 32] {
            core::ptr::write_volatile((sp - off) as *mut u32, 0);
        }
    }

    (*tcbs)[task_idx + 1].sp = sp - 64; // reserve callee-saved space below HW frame
    (*tcbs)[task_idx + 1].pc = body_addr;
    (*tcbs)[task_idx + 1].state = TaskState::Ready as u32;
    true
}

/// Install multi-tasking handlers into SRAM vector table.
///
/// # Safety
/// Call after boot_relocate_vectors(). Before enabling SysTick.
pub unsafe fn install_task_handlers() {
    // RAM_VECTOR_TABLE is the SRAM vector table (VTOR target).
    // PendSV slot = word 14, SysTick = word 15.
    let vt = core::ptr::addr_of_mut!(crate::kernel::interrupt::RAM_VECTOR_TABLE) as *mut u32;

    core::ptr::write_volatile(vt.add(14), PendSV_Handler as *const () as u32);
    core::ptr::write_volatile(vt.add(15), sys_tick_pend as *const () as u32);

    // PendSV priority lowest
    core::ptr::write_volatile((0xE000_ED20usize + 12) as *mut u32, 0xFF00_0000);
}

extern "C" fn sys_tick_pend() {
    unsafe {
        core::ptr::write_volatile(0xE000_ED04 as *mut u32, 1 << 28); // PENDSVSET
    }
}
