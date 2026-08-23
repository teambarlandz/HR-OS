//! Native-vs-threaded execution benchmark (RoadMap M4 / BENCHMARK.md).
//!
//! Builds one fixed stream — `lit 2, lit 3, add, halt` — and measures:
//!   T_threaded : cycles per `run_threaded_stream` dispatch (steady state)
//!   T_first    : cycles for first `compile_and_run` (scan+emit+fence+jump)
//!   T_native   : cycles per steady-state call into compiled EXEC_BUFFER code
//!
//! Cycle sources: DWT->CYCCNT (ARM) / mcycle CSR (RISC-V).
use crate::compiler::native::compile_and_run;
use crate::compiler::parser::MAX_STREAM_WORDS;
use crate::compiler::primitives::{add_prim, halt_prim, lit_prim};
use crate::kernel::exec::{execute_sram_buffer, flush_instruction_cache, run_threaded_stream};

const ITERATIONS: usize = 1000;

#[inline(always)]
fn cycles() -> u32 {
    #[cfg(target_arch = "arm")]
    unsafe {
        // TRCENA | CYCCNTENA
        let demcr = 0xE000_EDFC as *mut u32;
        core::ptr::write_volatile(demcr, core::ptr::read_volatile(demcr) | 1 << 24);
        let ctrl = 0xE000_1000 as *mut u32;
        core::ptr::write_volatile(ctrl, core::ptr::read_volatile(ctrl) | 1);
        core::ptr::read_volatile(0xE000_1004 as *const u32)
    }
    #[cfg(target_arch = "riscv32")]
    {
        let c: u32;
        // SAFETY: mcycle CSR read; standard on RV32I.
        unsafe { core::arch::asm!("csrr {}, mcycle", out(reg) c) };
        c
    }
    #[cfg(not(any(target_arch = "arm", target_arch = "riscv32")))]
    {
        0
    }
}

/// Build `[lit 2, lit 3, add, halt]`.
fn build_stream() -> ([usize; MAX_STREAM_WORDS], usize) {
    let mut s = [0usize; MAX_STREAM_WORDS];
    s[0] = lit_prim as *const () as usize;
    s[1] = 2;
    s[2] = lit_prim as *const () as usize;
    s[3] = 3;
    s[4] = add_prim as *const () as usize;
    s[5] = halt_prim as *const () as usize;
    (s, 6)
}

/// Runs the benchmark; writes report lines through `emit`.
pub fn run_bench(emit: &mut dyn FnMut(&[u8])) {
    let (stream, len) = build_stream();

    // threaded steady-state (warm once)
    // SAFETY: parser-shaped stream; single-threaded REPL contract.
    unsafe { run_threaded_stream(stream.as_ptr()) };
    let t0 = cycles();
    for _ in 0..ITERATIONS {
        // SAFETY: see above.
        unsafe { run_threaded_stream(stream.as_ptr()) };
        core::hint::black_box(());
    }
    let threaded = cycles().wrapping_sub(t0) / ITERATIONS as u32;

    emit(b"bench: stream = lit 2, lit 3, add, halt\r\n");
    emit(b"bench: iterations = 1000\r\n");

    // native first call (compile + exec)
    let t0 = cycles();
    // SAFETY: static stream; EXEC_BUFFER owned by REPL path only.
    let first = unsafe { compile_and_run(&stream, len, true) };
    let first_cycles = cycles().wrapping_sub(t0);

    match first {
        Ok(_) => {
            // SAFETY: EXEC_BUFFER just written; fence required before jump.
            unsafe { flush_instruction_cache() };
            let t0 = cycles();
            for _ in 0..ITERATIONS {
                // SAFETY: buffer holds valid machine code from compile_and_run.
                let v = unsafe { execute_sram_buffer(0) };
                core::hint::black_box(v);
            }
            let native = cycles().wrapping_sub(t0) / ITERATIONS as u32;

            let mut buf = [0u8; 96];
            emit(format_u64(
                b"bench: threaded = ",
                threaded as u64,
                b" cyc/exec\r\n",
                &mut buf,
            ));
            emit(format_u64(
                b"bench: first-call (compile+exec) = ",
                first_cycles as u64,
                b" cyc\r\n",
                &mut buf,
            ));
            emit(format_u64(
                b"bench: native    = ",
                native as u64,
                b" cyc/exec\r\n",
                &mut buf,
            ));
            if native > 0 {
                let x100 = (threaded as u64 * 100) / native as u64;
                emit(format_u64(
                    b"bench: speedup  = x",
                    x100,
                    b"/100\r\n",
                    &mut buf,
                ));
            }
        }
        Err(_) => {
            emit(b"bench: native path unavailable on this target\r\n");
            let mut buf = [0u8; 96];
            emit(format_u64(
                b"bench: threaded = ",
                threaded as u64,
                b" cyc/exec\r\n",
                &mut buf,
            ));
        }
    }
}

/// Minimal decimal formatter into `prefix + number + suffix`.
fn format_u64<'a>(prefix: &[u8], v: u64, suffix: &[u8], buf: &'a mut [u8]) -> &'a [u8] {
    let mut n = 0usize;
    for &b in prefix {
        buf[n] = b;
        n += 1;
    }
    let mut digits = [0u8; 20];
    let mut d = 0usize;
    let mut x = v;
    loop {
        digits[d] = b'0' + (x % 10) as u8;
        d += 1;
        x /= 10;
        if x == 0 {
            break;
        }
    }
    while d > 0 {
        d -= 1;
        buf[n] = digits[d];
        n += 1;
    }
    for &b in suffix {
        buf[n] = b;
        n += 1;
    }
    &buf[..n]
}
