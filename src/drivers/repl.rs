//! Interactive bare-metal REPL (doc ch.5).
//!
//! The REPL *is* the kernel control loop: no syscalls, no scheduler, no
//! preemption. State machine: Idle -> Reading -> Evaluating -> Printing ->
//! Idle. Line editing supports backspace, Ctrl-U (kill line) and Ctrl-C
//! (cancel).

use crate::capabilities::{registry, tokens};
use crate::compiler::parser::{Compiler, NameBuf, Outcome};
use crate::drivers::uart;
use crate::kernel::memory;

/// Maximum input line length.
const LINE_MAX: usize = 128;

/// REPL states per the streaming evaluation cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Idle,
    Reading,
    Evaluating,
    Printing,
}

static mut LINE_BUF: [u8; LINE_MAX] = [0; LINE_MAX];
static mut LINE_LEN: usize = 0;

/// Persistent compiler state — fn definitions must survive across lines.
///
/// SAFETY: accessed only from `repl::run` on the single boot thread; no
/// interrupt handler touches it (UART IRQ path only feeds the ring buffer).
static mut COMPILER: Compiler = Compiler::new();

/// Run the REPL forever. This is the final boot destination.
pub fn run() -> ! {
    // Drain any leftover hardware UART buffer bytes prior to REPL startup
    while uart::poll_get_byte().is_some() {}

    let mut state = State::Idle;
    // Outcome of the Evaluating phase, consumed by the Printing phase.
    let mut pending: Option<Result<Outcome, crate::compiler::parser::ParseError>> = None;
    prompt();
    loop {
        state = match state {
            // Idle/Reading: poll the UART for input bytes.
            State::Idle | State::Reading => match uart::poll_get_byte() {
                Some(byte) => feed(byte),
                None => State::Reading,
            },
            // Evaluating: line complete — single-pass compile.
            State::Evaluating => {
                let line = take_line();
                pending = compile(line);
                State::Printing
            }
            // Printing: direct console writes, then re-arm the prompt.
            State::Printing => {
                match pending.take() {
                    Some(Ok(outcome)) => execute(outcome),
                    Some(Err(err)) => {
                        uart::write_str(b"ERR ");
                        uart::write_str(error_text(err));
                    }
                    None => {}
                }
                prompt();
                State::Idle
            }
        };
    }
}

fn prompt() {
    uart::write_str(b"holy> ");
}

/// Process one incoming byte (Reading state logic).
fn feed(byte: u8) -> State {
    let byte = if byte == b'\t' { b' ' } else { byte };
    match byte {
        b'\r' | b'\n' => {
            uart::write_str(b"\r\n");
            State::Evaluating
        }
        0x08 | 0x7F => {
            // Backspace / DEL.
            let len = line_len();
            if len > 0 {
                set_line_len(len - 1);
                uart::write_str(b"\x08 \x08");
            }
            State::Reading
        }
        0x15 => {
            // Ctrl-U: kill line.
            let len = line_len();
            for _ in 0..len {
                uart::write_str(b"\x08 \x08");
            }
            set_line_len(0);
            State::Reading
        }
        0x03 => {
            // Ctrl-C: cancel.
            set_line_len(0);
            uart::write_str(b"^C\r\n");
            prompt();
            State::Idle
        }
        0x20..=0x7E => {
            let len = line_len();
            if len < LINE_MAX {
                // SAFETY: bounds checked above; single-threaded access.
                unsafe { LINE_BUF[len] = byte };
                set_line_len(len + 1);
                uart::put_byte(byte); // echo
            }
            State::Reading
        }
        _ => State::Reading, // ignore control bytes we don't handle
    }
}

fn line_len() -> usize {
    // SAFETY: single-threaded REPL access to static line state.
    unsafe { LINE_LEN }
}

fn set_line_len(len: usize) {
    // SAFETY: single-threaded REPL access to static line state.
    unsafe { LINE_LEN = len };
}

fn take_line() -> &'static [u8] {
    let len = line_len();
    set_line_len(0);
    // SAFETY: static buffer, single-threaded; slice bounded by written len.
    unsafe { &LINE_BUF[..len] }
}

/// Evaluating phase: single-pass compile of one input line.
fn compile(line: &[u8]) -> Option<Result<Outcome, crate::compiler::parser::ParseError>> {
    if line.is_empty() {
        return None;
    }
    // SAFETY: single-threaded REPL; borrow confined to this call and
    // no reentrancy exists (no interrupts run the compiler).
    let compiler = unsafe { &mut *core::ptr::addr_of_mut!(COMPILER) };
    Some(compiler.parse(line))
}

fn error_text(err: crate::compiler::parser::ParseError) -> &'static [u8] {
    use crate::compiler::parser::ParseError::*;
    match err {
        LexError(_) => b"LEX\n",
        UnexpectedToken => b"UNEXPECTED TOKEN\n",
        UnsupportedOperator(_) => b"UNSUPPORTED OPERATOR\n",
        UnknownSymbol => b"UNKNOWN SYMBOL\n",
        DuplicateFn => b"FN REDEFINED\n",
        SymbolTableFull => b"SYMBOL TABLE FULL\n",
        FnTableFull => b"FN TABLE FULL\n",
        StreamFull => b"STREAM FULL\n",
        NameTooLong => b"NAME TOO LONG\n",
        DivByZero => b"DIV BY ZERO\n",
        MissingSemicolon => b"MISSING SEMICOLON\n",
        EmptyLine => b"\n",
        CapabilityViolation => b"E001: CAPABILITY_VIOLATION - Peripheral token not claimed\n",
    }
}

fn execute(outcome: Outcome) {
    match outcome {
        Outcome::Empty => {}
        Outcome::Help => print_help(),
        Outcome::Banner => uart::write_str(crate::kernel::BANNER),
        Outcome::Bound { name, value } => {
            uart::write_str(name.as_slice());
            uart::write_str(b" = ");
            write_value(value);
        }
        Outcome::FnDefined { name } => {
            uart::write_str(b"FN ");
            uart::write_str(name.as_slice());
            uart::write_line(b" DEFINED");
        }
        Outcome::Run(program) => match program.run() {
            Some(value) => {
                uart::write_str(b"= ");
                write_value(value);
            }
            None => uart::write_line(b"OK"),
        },
        Outcome::Claim(name) => do_claim(&name),
        Outcome::Drop(name) => do_drop(&name),
        Outcome::EnforcedPoke { addr, val } => match memory::enforced_poke_u32(addr, val) {
            Ok(()) => uart::write_line(b"OK"),
            Err(e) => {
                uart::write_str(e.as_bytes());
                uart::write_str(b"\n");
            }
        },
        Outcome::EnforcedPeek { addr } => match memory::enforced_peek_u32(addr) {
            Ok(value) => {
                uart::write_str(b"= ");
                write_value(value);
            }
            Err(e) => {
                uart::write_str(e.as_bytes());
                uart::write_str(b"\n");
            }
        },
        Outcome::SetBit { addr, bit } => {
            // Capability enforcement for reg_set_bit (doc ch.2).
            match memory::enforced_poke_u32(addr, memory::peek_u32(addr as usize) | (1u32 << bit)) {
                Ok(()) => uart::write_line(b"OK"),
                Err(e) => {
                    uart::write_str(e.as_bytes());
                    uart::write_str(b"\n");
                }
            }
        }
        Outcome::ClrBit { addr, bit } => {
            // Capability enforcement for reg_clr_bit (doc ch.2).
            match memory::enforced_poke_u32(addr, memory::peek_u32(addr as usize) & !(1u32 << bit))
            {
                Ok(()) => uart::write_line(b"OK"),
                Err(e) => {
                    uart::write_str(e.as_bytes());
                    uart::write_str(b"\n");
                }
            }
        }
        Outcome::SysAudit => handle_audit(),
        Outcome::Bench => crate::kernel::bench::run_bench(&mut |line| uart::write_str(line)),
        Outcome::Pwm { period, duty } => {
            let (arr, ccr) = crate::drivers::pwm::configure(period, duty);
            uart::write_str(b"PWM ARR=");
            uart::write_dec_u32(arr);
            uart::write_str(b" CCR1=");
            uart::write_line_u32(ccr);
        }
        Outcome::PwmDuty { duty } => {
            crate::drivers::pwm::set_duty(duty);
            uart::write_line(b"OK");
        }
        Outcome::SpiTx { byte } => {
            let rx = crate::drivers::spi::transfer_byte(byte);
            uart::write_str(b"SPI RX=");
            uart::write_line_u32(rx as u32);
        }
        #[cfg(not(target_arch = "arm"))]
        Outcome::Store(_) | Outcome::Load(_) | Outcome::StoreList => {
            uart::write_line(b"NO STORE ON THIS TARGET");
        }
        Outcome::Spawn(_) => {
            // Phase 8a: spawn uses asm counter tasks (spawned at boot).
            // JIT fn spawning deferred to Phase 8b.
            uart::write_line(b"SPAWN: counter tasks auto-spawned at boot");
        }
        #[cfg(target_arch = "arm")]
        Outcome::PoolAlloc { size } => {
            // SAFETY: pool is linker-carved SRAM; single-threaded REPL.
            match unsafe { crate::kernel::pool::alloc(0, size as usize) } {
                Ok(addr) => {
                    uart::write_str(b"POOL @");
                    uart::write_hex_u32(addr as u32);
                    uart::write_line(b"");
                }
                Err(_) => uart::write_line(b"POOL FULL"),
            }
        }
        #[cfg(target_arch = "arm")]
        Outcome::PoolFree => {
            // SAFETY: resets offset to 0; prior pointers invalid by design.
            unsafe {
                crate::kernel::pool::reset(0).unwrap_or(());
            }
            uart::write_line(b"POOL RESET");
        }
        #[cfg(target_arch = "arm")]
        Outcome::PoolStats => {
            for (i, (used, rem)) in crate::kernel::pool::stats().iter().enumerate() {
                uart::write_str(b"pool[");
                uart::write_dec_u32(i as u32);
                uart::write_str(b"] used=");
                uart::write_dec_u32(*used as u32);
                uart::write_str(b" free=");
                uart::write_dec_u32(*rem as u32);
                uart::write_line(b"");
            }
        }
        #[cfg(not(target_arch = "arm"))]
        Outcome::PoolAlloc { .. } | Outcome::PoolFree | Outcome::PoolStats => {
            uart::write_line(b"NO POOL ON THIS TARGET");
        }
        Outcome::FlashTest => {
            let ok = crate::drivers::flash::self_test();
            uart::write_str(b"FLASH SELF-TEST: ");
            uart::write_line(if ok {
                b"HONORS PROGRAMMING"
            } else {
                b"STUB (writes ignored)"
            });
        }
        #[cfg(target_arch = "arm")]
        Outcome::Store(name) => {
            use crate::compiler::parser::MAX_STREAM_WORDS;
            // SAFETY: single-threaded REPL; borrow confined to this call.
            let compiler = unsafe { &mut *core::ptr::addr_of_mut!(COMPILER) };
            let mut words = [0usize; MAX_STREAM_WORDS];
            match compiler.export_fn(name.as_slice(), &mut words) {
                Some(count) => {
                    let mut slot = None;
                    let mut probe = [0u8; 16];
                    for i in 0..crate::drivers::pstore::MAX_PROGRAMS {
                        if matches!(
                            crate::drivers::pstore::load(i, &mut probe, &mut []),
                            Err(crate::drivers::pstore::StoreError::NotFound)
                        ) {
                            slot = Some(i);
                            break;
                        }
                    }
                    match slot {
                        Some(i) => {
                            crate::drivers::pstore::save(i, name.as_slice(), &words[..count])
                                .unwrap_or(());
                            uart::write_line(b"STORED");
                        }
                        None => uart::write_line(b"STORE FULL"),
                    }
                }
                None => uart::write_line(b"ERR UNKNOWN FN"),
            }
        }

        #[cfg(target_arch = "arm")]
        Outcome::Load(name) => {
            // SAFETY: single-threaded REPL; borrow confined to this call.
            let compiler = unsafe { &mut *core::ptr::addr_of_mut!(COMPILER) };
            let mut words = [0usize; 64];
            let mut found = false;
            for i in 0..crate::drivers::pstore::MAX_PROGRAMS {
                let mut probe = [0u8; 16];
                match crate::drivers::pstore::load(i, &mut probe, &mut words) {
                    Ok(count) => {
                        if probe[..name.as_slice().len()] == *name.as_slice() {
                            match compiler.import_fn(name.as_slice(), &words[..count]) {
                                Ok(()) => uart::write_line(b"LOADED"),
                                Err(_) => uart::write_line(b"ERR IMPORT FAILED"),
                            }
                            found = true;
                            break;
                        }
                    }
                    Err(crate::drivers::pstore::StoreError::NoStore) => {
                        uart::write_line(b"NO STORE ON THIS TARGET");
                        found = true;
                        break;
                    }
                    _ => continue,
                }
            }
            if !found {
                uart::write_line(b"NOT FOUND");
            }
        }

        #[cfg(target_arch = "arm")]
        Outcome::StoreList => {
            // SAFETY: single-threaded REPL.
            let compiler = unsafe { &*core::ptr::addr_of_mut!(COMPILER) };
            let mut any = false;
            for (buf, len) in compiler.fn_names_iter() {
                any = true;
                uart::write_str(&buf[..len as usize]);
                uart::write_str(b"\r\n");
            }
            if !any {
                uart::write_line(b"(no fns)");
            }
        }
    }
}

fn write_value(value: u32) {
    uart::write_hex_u32(value);
    uart::write_str(b" (");
    uart::write_dec_u32(value);
    uart::write_line(b")");
}

fn do_claim(name: &NameBuf) {
    match tokens::resolve_name(name.as_slice()) {
        Some(id) => {
            if registry::acquire(id as usize) {
                uart::write_str(b"CAP CLAIMED ");
                uart::write_str(name.as_slice());
                uart::write_str(b" id=");
                uart::write_dec_u32(id as u32);
                uart::write_str(b"\n");
            } else {
                uart::write_str(b"CAP BUSY ");
                uart::write_line(name.as_slice());
            }
        }
        None => {
            uart::write_str(b"ERR UNKNOWN RESOURCE ");
            uart::write_line(name.as_slice());
        }
    }
}

fn do_drop(name: &NameBuf) {
    match tokens::resolve_name(name.as_slice()) {
        Some(id) => {
            if registry::available(id as usize) {
                uart::write_str(b"CAP NOT HELD ");
                uart::write_line(name.as_slice());
            } else {
                registry::release(id as usize);
                uart::write_str(b"CAP RELEASED ");
                uart::write_line(name.as_slice());
            }
        }
        None => {
            uart::write_str(b"ERR UNKNOWN RESOURCE ");
            uart::write_line(name.as_slice());
        }
    }
}

fn handle_audit() {
    unsafe {
        use crate::capabilities::audit::SUPERUSER_AUDIT_LOG;
        let log = core::ptr::addr_of_mut!(SUPERUSER_AUDIT_LOG);
        let count = (*log).total_audits();
        uart::write_str(b"--- SUPERUSER AUDIT LOG ---\r\n");
        uart::write_str(b"Total Unsafe Operations: ");
        uart::write_dec_u32(count as u32);
        uart::write_str(b"\r\nRecent Events:\r\n");

        for entry in (*log).entries().iter() {
            if entry.addr != 0 {
                uart::write_str(b"ADDR: ");
                uart::write_hex_u32(entry.addr);
                uart::write_str(b" | VAL: ");
                uart::write_hex_u32(entry.val);
                uart::write_str(b" | CYCLES: ");
                uart::write_dec_u32(entry.timestamp_cycles);
                uart::write_str(b"\r\n");
            }
        }
    }
}

fn print_help() {
    uart::write_line(
        b"commands:\r\n\
          peek ADDR;              read u32 from address (requires capability)\r\n\
          poke ADDR VAL;          write u32 to address (requires capability)\r\n\
          reg_set_bit ADDR BIT;   set register bit (requires capability)\r\n\
          reg_clr_bit ADDR BIT;   clear register bit (requires capability)\r\n\
          cap_claim NAME;         claim peripheral (GPIOA GPIOB UART0 SPI0 I2C0 TIMER0 DMA0 SUPERUSER)\r\n\
          cap_drop NAME;          release peripheral\r\n\
          let NAME = EXPR;        bind constant\r\n\
          fn NAME() { ... }       define callable body\r\n\
          EXPR;                   evaluate (+ - * / % left-to-right)\r\n\
          sys_audit               dump SuperUser audit log\r\n\
          bench                   native vs threaded JIT benchmark\r\n\
          pwm PERIOD DUTY;        TIM2 PWM config (Timer0 cap)\r\n\
          pwm_duty DUTY;          live duty update (Timer0 cap)\r\n\
          spi_tx BYTE;            SPI1 transfer (Spi0 cap)\r\n\
          banner                  reprint banner",
    );
}
