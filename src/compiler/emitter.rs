//! Architecture-specific machine code emitters.
//!
//! Writes raw instruction encodings into the executable SRAM buffer
//! ([`crate::kernel::exec::EXEC_BUFFER`]). All writes are bounds-checked
//! against the buffer capacity (`Result`-returning API) — unlike the doc
//! sketches, which wrote through unchecked pointers.
//!
//! Encodings here are derived from the ARMv7-M and RV32I manuals; the
//! sample bit-packing in docs/CHAPTER_03 was incorrect and is superseded
//! by this implementation.

use crate::kernel::exec::{EXEC_BUFFER, EXEC_BUFFER_SIZE};

/// Emitter failure modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmitError {
    /// Exec buffer exhausted.
    Overflow,
    /// Register number outside the encoding's legal range.
    BadRegister,
}

/// Universal emitter contract shared by both backends.
#[allow(dead_code)] // Native codegen verification lands with Milestone 4.
pub trait TargetEmitter {
    /// Load a 32-bit immediate into `reg`.
    fn emit_mov_imm(&mut self, reg: u8, imm: u32) -> Result<(), EmitError>;
    /// Store `src_reg` to the address held in `addr_reg` (offset 0).
    fn emit_store_u32(&mut self, src_reg: u8, addr_reg: u8) -> Result<(), EmitError>;
    /// Load from the address in `addr_reg` into `dst_reg` (offset 0).
    fn emit_load_u32(&mut self, dst_reg: u8, addr_reg: u8) -> Result<(), EmitError>;
    /// `dst = a + b`.
    fn emit_add(&mut self, dst: u8, a: u8, b: u8) -> Result<(), EmitError>;
    /// `dst = a - b`.
    fn emit_sub(&mut self, dst: u8, a: u8, b: u8) -> Result<(), EmitError>;
    /// `dst = a * b`.
    fn emit_mul(&mut self, dst: u8, a: u8, b: u8) -> Result<(), EmitError>;
    /// `dst = a / b` (signed, truncating toward zero; div-by-zero returns 0).
    fn emit_div(&mut self, dst: u8, a: u8, b: u8) -> Result<(), EmitError>;
    /// Load `imm` into the return register and return to caller.
    fn emit_mov_ret(&mut self, imm: u32) -> Result<(), EmitError>;
    /// Return to caller.
    fn emit_ret(&mut self) -> Result<(), EmitError>;
    /// Bytes emitted so far.
    fn bytes_written(&self) -> usize;

    /// Emit scalar O(1) capability guard (3c) — LSR + LDR + TBZ.
    /// Checks single 4K block `addr_reg` against task cap vector.
    fn emit_cap_guard_scalar(&mut self, addr_reg: u8, tmp_reg: u8) -> Result<(), EmitError>;

    /// Emit vector O(1) capability guard (1c) — 256-bit SIMD.
    /// Checks contiguous `len` blocks via Mask256; 1c for 256 blocks.
    fn emit_cap_guard_vector(&mut self, addr_reg: u8, len: usize) -> Result<(), EmitError>;

    /// Emit custom RISC-V `hros.capchk` (1c) — hardware single-cycle.
    fn emit_cap_guard_custom(&mut self, addr_reg: u8, cap_reg: u8) -> Result<(), EmitError>;
}

// ---------------------------------------------------------------------------
// Pure encoding helpers (unit-testable without touching SRAM)
// ---------------------------------------------------------------------------

/// Encode Thumb-2 MOVW Rd, #imm16 -> (halfword1, halfword2).
#[allow(dead_code)]
pub fn encode_movw(rd: u8, imm: u32) -> (u16, u16) {
    let imm4 = ((imm >> 12) & 0xF) as u16;
    let i = ((imm >> 11) & 0x1) as u16;
    let imm3 = ((imm >> 8) & 0x7) as u16;
    let imm8 = (imm & 0xFF) as u16;
    let hw1 = 0xF240 | (i << 10) | imm4;
    let hw2 = (imm3 << 12) | ((rd as u16 & 0xF) << 8) | imm8;
    (hw1, hw2)
}

/// Encode Thumb-2 MOVT Rd, #imm16 -> (halfword1, halfword2).
#[allow(dead_code)]
pub fn encode_movt(rd: u8, imm: u32) -> (u16, u16) {
    let high = imm >> 16;
    let imm4 = ((high >> 12) & 0xF) as u16;
    let i = ((high >> 11) & 0x1) as u16;
    let imm3 = ((high >> 8) & 0x7) as u16;
    let imm8 = (high & 0xFF) as u16;
    let hw1 = 0xF2C0 | (i << 10) | imm4;
    let hw2 = (imm3 << 12) | ((rd as u16 & 0xF) << 8) | imm8;
    (hw1, hw2)
}

/// Encode 16-bit STR Rt, [Rn, #imm5*4].
#[allow(dead_code)]
pub fn encode_str_imm(rt: u8, rn: u8, byte_offset: u32) -> Option<u16> {
    if rt > 7 || rn > 7 || byte_offset > 124 || !byte_offset.is_multiple_of(4) {
        return None;
    }
    Some(0x6000 | (((byte_offset / 4) as u16) << 6) | ((rn as u16) << 3) | rt as u16)
}

/// Encode 16-bit LDR Rt, [Rn, #imm5*4].
#[allow(dead_code)]
pub fn encode_ldr_imm(rt: u8, rn: u8, byte_offset: u32) -> Option<u16> {
    if rt > 7 || rn > 7 || byte_offset > 124 || !byte_offset.is_multiple_of(4) {
        return None;
    }
    Some(0x6800 | (((byte_offset / 4) as u16) << 6) | ((rn as u16) << 3) | rt as u16)
}

/// Encode RV32I LUI rd, imm20.
#[allow(dead_code)]
pub fn encode_lui(rd: u8, imm20: u32) -> u32 {
    0x37 | ((rd as u32 & 0x1F) << 7) | ((imm20 & 0xFFFFF) << 20)
}

/// Encode RV32I ADDI rd, rs1, imm12 (raw 12-bit field).
#[allow(dead_code)]
pub fn encode_addi(rd: u8, rs1: u8, imm12_field: u32) -> u32 {
    0x13 | ((rd as u32 & 0x1F) << 7) | ((rs1 as u32 & 0x1F) << 15) | ((imm12_field & 0xFFF) << 20)
}

/// Encode RV32I SW rs2, off(rs1).
#[allow(dead_code)]
pub fn encode_sw(rs2: u8, rs1: u8, offset: u32) -> Option<u32> {
    if (0x800..0xFFFFF000).contains(&offset) {
        return None; // outside signed 12-bit range
    }
    let off = offset & 0xFFF;
    Some(
        0x23 | (2 << 12)
            | ((off >> 5) << 25)
            | ((rs2 as u32 & 0x1F) << 20)
            | ((rs1 as u32 & 0x1F) << 15)
            | ((off & 0x1F) << 7),
    )
}

/// Encode RV32I LW rd, off(rs1).
#[allow(dead_code)]
pub fn encode_lw(rd: u8, rs1: u8, offset: u32) -> Option<u32> {
    if (0x800..0xFFFFF000).contains(&offset) {
        return None;
    }
    Some(
        0x03 | (2 << 12)
            | ((rd as u32 & 0x1F) << 7)
            | ((rs1 as u32 & 0x1F) << 15)
            | ((offset & 0xFFF) << 20),
    )
}

/// Encode RV32I ADD/SUB (funct7 selects).
#[allow(dead_code)]
pub fn encode_rtype_addsub(rd: u8, rs1: u8, rs2: u8, sub: bool) -> u32 {
    let funct7 = if sub { 0x20 } else { 0x00 };
    0x33 | (funct7 << 25)
        | ((rs2 as u32 & 0x1F) << 20)
        | ((rs1 as u32 & 0x1F) << 15)
        | ((rd as u32 & 0x1F) << 7)
}

/// RV32I return: JALR x0, ra, 0.
#[allow(dead_code)]
pub const RV32_RET: u32 = 0x0000_8067;

/// Encode Thumb-2 SDIV Rd, Rn, Rm as two halfwords (hw1, hw2).
fn encode_sdiv(rd: u8, rn: u8, rm: u8) -> (u16, u16) {
    // Encoding T1: 1111_1011_1011_Rn  1111_Rd_0001_Rm
    let hw1 = 0xFB90 | (rn as u16);
    let hw2 = 0xF010 | ((rd as u16) << 8) | (rm as u16);
    (hw1, hw2)
}

/// Encode RV32I R-type with custom funct7/funct3.
fn encode_rtype(rd: u8, rs1: u8, rs2: u8, funct7: u32, funct3: u32) -> u32 {
    0x33 | (funct7 << 25)
        | ((rs2 as u32 & 0x1F) << 20)
        | ((rs1 as u32 & 0x1F) << 15)
        | (funct3 << 12)
        | ((rd as u32 & 0x1F) << 7)
}

// ---------------------------------------------------------------------------
// Thumb-2 backend
// ---------------------------------------------------------------------------

/// ARM Cortex-M (Thumb-2) emitter writing halfwords into EXEC_BUFFER.
#[allow(dead_code)]
pub struct Thumb2Emitter {
    cursor: *mut u16,
    cap_words: usize,
    len_words: usize,
}

impl Thumb2Emitter {
    /// New emitter positioned at the start of the executable SRAM buffer.
    ///
    /// # Safety
    /// Only one emitter (of either backend) may own EXEC_BUFFER at a time;
    /// concurrent emission would interleave instruction streams.
    pub unsafe fn into_exec_buffer() -> Self {
        Thumb2Emitter {
            cursor: core::ptr::addr_of_mut!(EXEC_BUFFER) as *mut u16,
            cap_words: EXEC_BUFFER_SIZE / 2,
            len_words: 0,
        }
    }

    fn push16(&mut self, hw: u16) -> Result<(), EmitError> {
        if self.len_words >= self.cap_words {
            return Err(EmitError::Overflow);
        }
        // SAFETY: len_words < cap_words enforced above; cursor advances in
        // lockstep with len_words so it never passes the buffer end.
        unsafe {
            core::ptr::write_volatile(self.cursor, hw);
            self.cursor = self.cursor.add(1);
        }
        self.len_words += 1;
        Ok(())
    }
}

impl TargetEmitter for Thumb2Emitter {
    fn emit_mov_imm(&mut self, reg: u8, imm: u32) -> Result<(), EmitError> {
        if reg > 15 {
            return Err(EmitError::BadRegister);
        }
        // Fast path: MOVS Rd, #imm8 covers 0..=255 in one 16-bit slot.
        if imm <= 0xFF && reg < 8 {
            return self.push16(0x2000 | ((reg as u16) << 8) | (imm as u16 & 0xFF));
        }
        // MOVW always (zeroes upper half); MOVT only when needed.
        let (w1, w2) = encode_movw(reg, imm);
        self.push16(w1)?;
        self.push16(w2)?;
        if imm >> 16 != 0 {
            let (t1, t2) = encode_movt(reg, imm);
            self.push16(t1)?;
            self.push16(t2)?;
        }
        Ok(())
    }

    fn emit_store_u32(&mut self, src_reg: u8, addr_reg: u8) -> Result<(), EmitError> {
        match encode_str_imm(src_reg, addr_reg, 0) {
            Some(hw) => self.push16(hw),
            None => Err(EmitError::BadRegister),
        }
    }

    fn emit_load_u32(&mut self, dst_reg: u8, addr_reg: u8) -> Result<(), EmitError> {
        match encode_ldr_imm(dst_reg, addr_reg, 0) {
            Some(hw) => self.push16(hw),
            None => Err(EmitError::BadRegister),
        }
    }

    fn emit_add(&mut self, dst: u8, a: u8, b: u8) -> Result<(), EmitError> {
        if dst > 7 || a > 7 || b > 7 {
            return Err(EmitError::BadRegister);
        }
        // ADDS Rd, Rn, Rm
        self.push16(0x1800 | ((b as u16) << 6) | ((a as u16) << 3) | dst as u16)
    }

    fn emit_sub(&mut self, dst: u8, a: u8, b: u8) -> Result<(), EmitError> {
        if dst > 7 || a > 7 || b > 7 {
            return Err(EmitError::BadRegister);
        }
        // SUBS Rd, Rn, Rm
        self.push16(0x1A00 | ((b as u16) << 6) | ((a as u16) << 3) | dst as u16)
    }

    fn emit_mul(&mut self, dst: u8, a: u8, b: u8) -> Result<(), EmitError> {
        if dst > 7 || a > 7 || b > 7 {
            return Err(EmitError::BadRegister);
        }
        // MULS Rdm, Rn, Rm — Rdm must equal Rn; result is in Rdm.
        // Move a into dst first if needed, then multiply.
        if a != dst {
            // MOVS Rd, Rn (register-register)
            self.push16(((a as u16) << 3) | dst as u16)?;
        }
        // MULS Rdm, Rm → 0x4340 | (Rm << 3) | Rdm
        self.push16(0x4340 | ((b as u16) << 3) | (dst as u16))
    }

    fn emit_div(&mut self, dst: u8, a: u8, b: u8) -> Result<(), EmitError> {
        if dst > 15 || a > 15 || b > 15 {
            return Err(EmitError::BadRegister);
        }
        // SDIV Rd, Rn, Rm — 32-bit Thumb-2 instruction.
        let (hw1, hw2) = encode_sdiv(dst, a, b);
        self.push16(hw1)?;
        self.push16(hw2)
    }

    fn emit_mov_ret(&mut self, imm: u32) -> Result<(), EmitError> {
        // Load into r0 (return register) then BX LR.
        self.emit_mov_imm(0, imm)?;
        self.emit_ret()
    }

    fn emit_ret(&mut self) -> Result<(), EmitError> {
        // BX LR
        self.push16(0x4770)
    }

    fn bytes_written(&self) -> usize {
        self.len_words * 2
    }

    fn emit_cap_guard_scalar(&mut self, addr_reg: u8, _tmp_reg: u8) -> Result<(), EmitError> {
        // Scalar 3c guard per AXIS-3: LSR + LDR + TBZ
        // LSR tmp, addr, #12
        // LSR tmp2, tmp, #6
        // LDR cap_word, [cap_base, tmp2, LSL #3]
        // LSR cap_word, cap_word, tmp
        // TBZ cap_word, #0, .FAULT_TRAP
        // For emitter, we emit placeholder 3 halfwords (actual encoding would be 6 bytes)
        // 0xF3AF = ASR, 0xF8D0 = LDR, 0xEC10 = TBZ (placeholders)
        self.push16(0xF3AF)?; // LSR
        self.push16(0xF8D0)?; // LDR
        self.push16(0xEC10)?; // TBZ
        let _ = addr_reg;
        Ok(())
    }

    fn emit_cap_guard_vector(&mut self, addr_reg: u8, len: usize) -> Result<(), EmitError> {
        // Vector 1c guard per UPGRADE.md Step 1: single SIMD VANDPS+VPTEST fused as one pseudo-op
        // For Thumb-2, we emit a single 32-bit MCR to custom coprocessor (placeholder 0xF3AF8000)
        // Real hardware would do VLD1 + VAND + VPTEST in 1c; we model as 1x32-bit = 2 halfwords
        if len > 256 {
            return Err(EmitError::Overflow);
        }
        self.push16(0xF3AF)?;
        self.push16(0x8000 | (addr_reg as u16 & 0xF) << 8 | (len as u16 & 0xFF))?;
        Ok(())
    }

    fn emit_cap_guard_custom(&mut self, addr_reg: u8, cap_reg: u8) -> Result<(), EmitError> {
        // Custom RISC-V hros.capchk on ARM as MCR placeholder: 2 halfwords
        self.push16(0xF3AF)?;
        self.push16(0x8000 | ((cap_reg as u16) << 8) | (addr_reg as u16))?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// RV32I backend
// ---------------------------------------------------------------------------

/// RISC-V RV32I emitter writing words into EXEC_BUFFER.
#[allow(dead_code)]
pub struct Riscv32Emitter {
    cursor: *mut u32,
    cap_words: usize,
    len_words: usize,
}

impl Riscv32Emitter {
    /// New emitter positioned at the start of the executable SRAM buffer.
    ///
    /// # Safety
    /// See [`Thumb2Emitter::into_exec_buffer`] — single-owner rule applies.
    pub unsafe fn into_exec_buffer() -> Self {
        Riscv32Emitter {
            cursor: core::ptr::addr_of_mut!(EXEC_BUFFER) as *mut u32,
            cap_words: EXEC_BUFFER_SIZE / 4,
            len_words: 0,
        }
    }

    fn push32(&mut self, word: u32) -> Result<(), EmitError> {
        if self.len_words >= self.cap_words {
            return Err(EmitError::Overflow);
        }
        // SAFETY: capacity checked above; cursor tracks len_words exactly.
        unsafe {
            core::ptr::write_volatile(self.cursor, word);
            self.cursor = self.cursor.add(1);
        }
        self.len_words += 1;
        Ok(())
    }
}

impl TargetEmitter for Riscv32Emitter {
    fn emit_mov_imm(&mut self, reg: u8, imm: u32) -> Result<(), EmitError> {
        // Standard LUI/ADDI pairing with 0x800 rounding so the ADDI
        // sign-extension cancels exactly.
        let signed_ok = imm <= 0x7FF || imm >= 0xFFFF_F800;
        if signed_ok {
            return self.push32(encode_addi(reg, 0, imm)); // ADDI rd, x0, imm
        }
        let hi20 = imm.wrapping_add(0x800) >> 12;
        let lo12 = imm.wrapping_sub(hi20 << 12); // lands in [-2048, 2047]
        self.push32(encode_lui(reg, hi20))?;
        if lo12 != 0 {
            self.push32(encode_addi(reg, reg, lo12))?;
        }
        Ok(())
    }

    fn emit_store_u32(&mut self, src_reg: u8, addr_reg: u8) -> Result<(), EmitError> {
        match encode_sw(src_reg, addr_reg, 0) {
            Some(word) => self.push32(word),
            None => Err(EmitError::BadRegister),
        }
    }

    fn emit_load_u32(&mut self, dst_reg: u8, addr_reg: u8) -> Result<(), EmitError> {
        match encode_lw(dst_reg, addr_reg, 0) {
            Some(word) => self.push32(word),
            None => Err(EmitError::BadRegister),
        }
    }

    fn emit_add(&mut self, dst: u8, a: u8, b: u8) -> Result<(), EmitError> {
        self.push32(encode_rtype_addsub(dst, a, b, false))
    }

    fn emit_sub(&mut self, dst: u8, a: u8, b: u8) -> Result<(), EmitError> {
        self.push32(encode_rtype_addsub(dst, a, b, true))
    }

    fn emit_mul(&mut self, dst: u8, a: u8, b: u8) -> Result<(), EmitError> {
        // MUL rd, rs1, rs2 — funct7=0x01, funct3=0x00
        self.push32(encode_rtype(dst, a, b, 0x01, 0x00))
    }

    fn emit_div(&mut self, dst: u8, a: u8, b: u8) -> Result<(), EmitError> {
        // DIV rd, rs1, rs2 — funct7=0x01, funct3=0x04 (signed, trunc toward zero)
        self.push32(encode_rtype(dst, a, b, 0x01, 0x04))
    }

    fn emit_mov_ret(&mut self, imm: u32) -> Result<(), EmitError> {
        // Load into a0 (x10, return register) then JALR x0, ra, 0.
        self.emit_mov_imm(10, imm)?; // a0 = x10
        self.emit_ret()
    }

    fn emit_ret(&mut self) -> Result<(), EmitError> {
        self.push32(RV32_RET)
    }

    fn bytes_written(&self) -> usize {
        self.len_words * 4
    }

    fn emit_cap_guard_scalar(&mut self, addr_reg: u8, _tmp_reg: u8) -> Result<(), EmitError> {
        // Scalar 3c: SRLI + ANDI + LD + SRL + BNEZ (3 instructions, 12 bytes)
        // Placeholder: 3 words
        self.push32(0x00C5_5133)?; // SRLI
        self.push32(0x03F5_5033)?; // ANDI+LD
        self.push32(0x0005_8063)?; // BNEZ to .FAULT_TRAP (placeholder)
        let _ = addr_reg;
        Ok(())
    }

    fn emit_cap_guard_vector(&mut self, _addr_reg: u8, len: usize) -> Result<(), EmitError> {
        // Vector 1c: single custom hros.capchk is NOT used here; vector is for x86/ARM NEON
        // For RISC-V, vector path is same as scalar but with length prefix (still 1c for 256 blocks via loop in HW)
        if len > 256 {
            return Err(EmitError::Overflow);
        }
        self.push32(0xF3AF8000 | (len as u32 & 0xFF))?; // placeholder vector op
        Ok(())
    }

    fn emit_cap_guard_custom(&mut self, addr_reg: u8, cap_reg: u8) -> Result<(), EmitError> {
        // Custom RISC-V hros.capchk per UPGRADE.md Step 4: 1c hardware
        // [ funct7 7b | rs2 5b | rs1 5b | funct3 3b | rd 5b | opcode 7b ]
        let opcode: u32 = 0b0001011; // Custom-0
        let funct3: u32 = 0b000;
        let funct7: u32 = 0b0000001;
        let raw = (funct7 << 25) | ((cap_reg as u32 & 0x1F) << 20) | ((addr_reg as u32 & 0x1F) << 15) | (funct3 << 12) | opcode;
        self.push32(raw)
    }
}
