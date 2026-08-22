//! hros-kernel — Ring 0 core infrastructure.
//! Re-exports memory, exec, interrupt from reference src/kernel/*.

#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]

pub mod memory {
    #[inline(always)] pub fn peek_u32(addr: usize) -> u32 { unsafe { core::ptr::read_volatile(addr as *const u32) } }
    #[inline(always)] pub fn poke_u32(addr: usize, val: u32) { unsafe { core::ptr::write_volatile(addr as *mut u32, val) } }
    pub fn reg_set_bit(addr: usize, bit: u8) { let v = peek_u32(addr) | (1u32 << bit); poke_u32(addr, v) }
    pub fn reg_clr_bit(addr: usize, bit: u8) { let v = peek_u32(addr) & !(1u32 << bit); poke_u32(addr, v) }
    pub unsafe fn init_data_bss() {
        extern "C" { static mut __sidata: u32; static mut __sdata: u32; static mut __edata: u32; static mut __sbss: u32; static mut __ebss: u32; }
        let mut src = core::ptr::addr_of!(__sidata);
        let mut dst = core::ptr::addr_of_mut!(__sdata);
        let end = core::ptr::addr_of_mut!(__edata);
        while dst < end { unsafe { core::ptr::write_volatile(dst, core::ptr::read_volatile(src)); src = src.add(1); dst = dst.add(1); } }
        let mut z = core::ptr::addr_of_mut!(__sbss);
        let bss_end = core::ptr::addr_of_mut!(__ebss);
        while z < bss_end { unsafe { core::ptr::write_volatile(z, 0); z = z.add(1); } }
    }
}
pub mod exec {
    pub const EXEC_BUFFER_SIZE: usize = 4096;
    #[repr(C, align(4))] pub struct ExecBuffer(pub [u8; 4096]);
    #[link_section = ".sram_code"] pub static mut EXEC_BUFFER: ExecBuffer = ExecBuffer([0; 4096]);
    pub unsafe fn flush_instruction_cache() {
        #[cfg(target_arch="arm")] unsafe { core::arch::asm!("dsb", "isb", options(nostack)) }
        #[cfg(target_arch="riscv32")] unsafe { core::arch::asm!("fence.i", options(nostack)) }
    }
}
pub mod interrupt {
    #[no_mangle] pub extern "C" fn fault_hang() -> ! { loop { unsafe { core::arch::asm!("wfi") } } }
    #[repr(C, align(1024))] pub struct VectorTable { pub initial_sp: u32, pub reset: unsafe extern "C" fn() -> !, pub handlers: [Option<unsafe extern "C" fn()>; 32] }
    pub static mut RAM_VECTOR_TABLE: VectorTable = VectorTable { initial_sp: 0, reset: fault_hang, handlers: [None; 32] };
}
pub const BANNER: &[u8] = b"Holy Rust REPL v0.1\r\n";
