# Milestone.md — HR-OS Language & Capability Log

> **Purpose:** vital running log distinguishing what is **Holy Rust language** (JIT-compiled,
> composable, persistent) vs **HR-OS REPL built-ins** (console service verbs, not compiled,
> not composable). Updated every time the surface changes.
>
> **Version at this entry: 0.2.0**

---

## The three-layer taxonomy

| Layer                         | Members                                                                                                          | Compiled to native?               | Composable in `fn`?                                            | Persistent via store? |
| ----------------------------- | ---------------------------------------------------------------------------------------------------------------- | --------------------------------- | -------------------------------------------------------------- | --------------------- |
| **L1 — Holy Rust language**   | expressions (`+ - * / %`), `let`, `fn`, calls, `poke`, `peek` (in fn bodies)                                     | ✅ JIT → EXEC_BUFFER              | ✅                                                             | ✅ (store/load)       |
| **L2 — Kernel service verbs** | top-level `poke`/`peek`, `cap_claim`, `cap_drop`, `reg_set_bit`, `reg_clr_bit`                                   | ❌ direct enforced MMIO (1–3 cyc) | ❌ (top-level only; poke/peek ARE compilable inside fn bodies) | ❌                    |
| **L3 — REPL meta-commands**   | `bench`, `store`, `load`, `store_list`, `sys_audit`, `flash_test`, `help`, `banner`, `pwm`, `pwm_duty`, `spi_tx` | ❌ immediate driver/kernel call   | ❌                                                             | n/a                   |

---

## Holy Rust language reference (v0.2.0)

### Statements (LL(1) grammar, left-to-right eval, no precedence)

| Syntax                | Semantics                                          | Capability gate                     |
| --------------------- | -------------------------------------------------- | ----------------------------------- |
| `EXPR;`               | evaluate arithmetic, print result                  | none                                |
| `let NAME = EXPR;`    | bind constant                                      | none                                |
| `fn NAME() { STMT* }` | define zero-arg function (body = L1/L2 statements) | checked per-stmt at definition time |
| `NAME();`             | call defined fn (threaded or native path)          | inherits body's gates               |
| `lit` operands        | u32 decimal / `0x` hex / `_` separators            | —                                   |

### Operators (left-assoc, no precedence)

`+ - * / %` — division by zero caught at parse time.

### Memory verbs

| Syntax           | Context   | Behavior                                |
| ---------------- | --------- | --------------------------------------- |
| `poke ADDR VAL;` | top-level | capability-enforced direct MMIO write   |
| `poke ADDR VAL;` | inside fn | compiled to native SW with inline guard |
| `peek ADDR;`     | top-level | enforced read + print                   |
| `peek ADDR;`     | inside fn | compiled to native LW                   |

### Capability verbs

`cap_claim NAME;` · `cap_drop NAME;` — resources: GPIOA GPIOB UART0 SPI0 I2C0 TIMER0 DMA0 SUPERUSER.

---

## REPL built-in reference (not language)

| Command            | Layer                       | Requires        | Output                                  |
| ------------------ | --------------------------- | --------------- | --------------------------------------- |
| `pwm PERIOD DUTY;` | L3 driver verb (Timer0 cap) | Timer0 token    | ARR/CCR1 echo                           |
| `pwm_duty DUTY;`   | L3 driver verb              | Timer0          | OK                                      |
| `spi_tx BYTE;`     | L3 driver verb (Spi0 cap)   | Spi0            | SPI RX=…                                |
| `store NAME;`      | persistence                 | fn exists       | STORED / STORE FULL / ERR               |
| `load NAME;`       | persistence                 | slot holds name | LOADED / NOT FOUND / NO STORE (riscv32) |
| `store_list;`      | persistence                 | —               | persisted names                         |
| `bench;`           | diagnostics                 | —               | threaded vs native cyc/exec             |
| `sys_audit`        | diagnostics                 | —               | SuperUser access log dump               |
| `flash_test;`      | silicon bring-up            | —               | FPEC model probe                        |
| `banner` / `help`  | meta                        | —               | —                                       |

---

## Embedded-standard assessment (the debate)

### What Holy Rust can do today

Interactive MMIO control, arithmetic/compute kernels as persistent fns, capability-scoped
peripheral access — sufficient for **sensor polling, actuator control, register-level device
bring-up, and deterministic test harnesses** on MCU-class targets.

### What disqualifies it from general embedded projects _today_

| Gap                                   | Impact                                                   | Stage                                     |
| ------------------------------------- | -------------------------------------------------------- | ----------------------------------------- |
| No loops (`loop N {}`) in grammar     | iterative algorithms must unroll manually                | AXIS-4 spec'd, not implemented            |
| `fn` are zero-arg                     | no parameterized drivers                                 | needs ABI design                          |
| No conditional branches               | control flow = compute only                              | needs bounded-if spec                     |
| Device verbs (pwm/spi_tx) are L3-only | cannot compose peripheral sequences into stored programs | promote to stream primitives              |
| riscv32 has no program store          | DTIM fully carved                                        | ITIM+PRCI on real HW, or re-balance carve |
| Flash persistence stub under QEMU     | store is SRAM-volatile across resets                     | real FPEC works; QEMU model limitation    |

### Verdict

**Not yet general-purpose embedded standard.** It is a deterministic control/scripting layer —
think "Forth with proofs" — suitable as a companion to a host-compiled no_std Rust kernel,
not a replacement for C/Zephyr application development. Closing the gaps above (loops first —
they unlock every other pattern) is the path from scripting-layer to application language.

---

## Version history

| Version | Date       | Surface change                                                                                                                                                               |
| ------- | ---------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 0.1.0   | 2026-08-22 | baseline: 4 axes, REPL, poke/peek/fn/let/caps, bench infra                                                                                                                   |
| 0.2.0   | 2026-08-23 | + pwm/pwm_duty/spi_tx driver verbs (L3), + store/load/store_list persistence, + bench command, + flash driver, + stack-slack ASSERTs, + scratch region, + hardware test plan |
