//! Preemptive multi-tasking (Phase 8a, ARM only).
//!
//! SysTick -> PendSV -> context switch between REPL task and spawned tasks.

#![allow(clippy::fn_to_numeric_cast)]

use core::arch::global_asm;

pub const NUM_TASKS: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum TaskState {
    Dead = 0,
    Ready = 1,
    Running = 2,
}

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
    "   bx lr",
    "1:",
    "   mrs r0, psp",
    "   stmdb r0!, {{r4-r11}}",
    "   ldr r3, ={tcbs}",
    "   movs r4, #16",
    "   lsls r5, r2, #4",
    "   adds r3, r3, r5",
    "   str r0, [r3]",
    "   movs r5, #1",
    "   str r5, [r3, #8]",
    "   ldr r1, ={current}",
    "   str r6, [r1]",
    "   ldr r3, ={tcbs}",
    "   movs r4, #16",
    "   lsls r5, r6, #4",
    "   adds r3, r3, r5",
    "   ldr r0, [r3]",
    "   ldmia r0!, {{r4-r11}}",
    "   msr psp, r0",
    "   movs r5, #2",
    "   str r5, [r3, #8]",
    "   ldr r6, [r3, #12]",
    "   adds r6, r6, #1",
    "   str r6, [r3, #12]",
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
    "   ldr r1, [r0, #28]",
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
    "   ldr r1, [r0, #44]",
    "   adds r1, r1, #1",
    "   str r1, [r0, #44]",
    "   b 1b",
    tcbs = sym crate::kernel::multitask::TCBS,
);

extern "C" {
    fn PendSV_Handler();
    fn task_body_1();
    fn task_body_2();
}

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

/// Spawn a counter task proving context switching works.
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
    let sp = stack_top;
    unsafe {
        core::ptr::write_volatile((sp - 4) as *mut u32, 0x0100_0000);
        core::ptr::write_volatile((sp - 8) as *mut u32, body_addr | 1);
        for off in [12u32, 16, 20, 24, 28, 32] {
            core::ptr::write_volatile((sp - off) as *mut u32, 0);
        }
    }
    (*tcbs)[task_idx + 1].sp = sp - 64;
    (*tcbs)[task_idx + 1].pc = body_addr;
    (*tcbs)[task_idx + 1].state = TaskState::Ready as u32;
    true
}

/// Install multi-tasking handlers into SRAM vector table.
///
/// # Safety
/// Call after boot_relocate_vectors(). Before enabling SysTick.
pub unsafe fn install_task_handlers() {
    let vt = core::ptr::addr_of_mut!(crate::kernel::interrupt::RAM_VECTOR_TABLE) as *mut u32;
    extern "C" {
        fn PendSV_Handler();
    }
    core::ptr::write_volatile(vt.add(14), PendSV_Handler as *const () as u32);
    core::ptr::write_volatile(vt.add(15), sys_tick_pend as *const () as u32);
    core::ptr::write_volatile((0xE000_ED20usize + 12) as *mut u32, 0xFF00_0000);
}

extern "C" fn sys_tick_pend() {
    unsafe {
        core::ptr::write_volatile(0xE000_ED04 as *mut u32, 1 << 28);
    }
}

/// Spawn a JIT task that runs threaded stream from EXEC_BUFFER partition.
///
/// # Safety
/// Call after VTOR relocation and install_task_handlers().
pub unsafe fn spawn_jit_task(slot_idx: usize) -> bool {
    if slot_idx >= NUM_TASKS - 1 {
        return false;
    }
    let tcbs = core::ptr::addr_of_mut!(TCBS);
    let stack_top = task_stack_top(slot_idx + 1);
    // Entry point: threaded_task_runner (loops forever on slot's stream)
    // For Phase 8a, use counter tasks as placeholder — JIT runner is Phase 8b.
    let body: unsafe extern "C" fn() = if slot_idx == 0 {
        task_body_1
    } else {
        task_body_2
    };
    let body_addr = body as *const () as u32;
    let sp = stack_top;
    unsafe {
        core::ptr::write_volatile((sp - 4) as *mut u32, 0x0100_0000);
        core::ptr::write_volatile((sp - 8) as *mut u32, body_addr | 1);
        for off in [12u32, 16, 20, 24, 28, 32] {
            core::ptr::write_volatile((sp - off) as *mut u32, 0);
        }
    }
    (*tcbs)[slot_idx + 1].sp = sp - 64;
    (*tcbs)[slot_idx + 1].pc = body_addr;
    (*tcbs)[slot_idx + 1].state = TaskState::Ready as u32;
    true
}

/// Emit fn body words into a JIT slot.
///
/// # Safety
/// Single-writer per slot. EXEC_BUFFER region is RWX.
pub unsafe fn emit_fn_into_slot(slot_idx: usize, words: &[usize]) -> Result<(), ()> {
    if slot_idx >= NUM_TASKS - 1 || words.len() > 256 {
        return Err(());
    }
    let bases: [usize; 4] = [0x2000_2000, 0x2000_2400, 0x2000_2800, 0x2000_2C00];
    let base = bases[slot_idx];
    for (i, &w) in words.iter().enumerate() {
        core::ptr::write_volatile((base + i * 4) as *mut u32, w as u32);
    }
    crate::kernel::exec::flush_instruction_cache();
    Ok(())
}

fn jit_slot_base_addr(slot_idx: usize) -> usize {
    match slot_idx {
        0 => 0x2000_2000,
        1 => 0x2000_2400,
        2 => 0x2000_2800,
        _ => 0x2000_2C00,
    }
}
