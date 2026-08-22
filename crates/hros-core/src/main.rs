//! HR-OS — Ring 0 boot entry (Phase 1)
//! See src/main.rs (reference impl).

#![no_std]
#![no_main]

use core::panic::PanicInfo;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {
        unsafe { core::arch::asm!("wfi") }
    }
}

#[cfg(target_arch = "arm")]
#[no_mangle]
pub extern "C" fn Reset() -> ! {
    unsafe {
        hros_kernel::memory::init_data_bss();
    }
    boot()
}

#[cfg(target_arch = "riscv32")]
#[unsafe(naked)]
#[no_mangle]
pub unsafe extern "C" fn Reset() -> ! {
    core::arch::naked_asm!(
        ".option push",
        ".option norelax",
        "la gp, __global_pointer$",
        "la sp, _stack_top",
        ".option pop",
        "tail rust_boot_riscv",
    )
}

#[cfg(target_arch = "riscv32")]
#[no_mangle]
unsafe extern "C" fn rust_boot_riscv() -> ! {
    unsafe {
        hros_kernel::memory::init_data_bss();
    }
    boot()
}

fn boot() -> ! {
    hros_drivers::uart::init();
    unsafe { core::arch::asm!("wfi") }
    hros_drivers::repl::run()
}
