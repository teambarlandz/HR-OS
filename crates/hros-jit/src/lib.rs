//! hros-jit — Single-pass streaming JIT.
//! See src/compiler/* (reference impl). This crate will own Lexer, Compiler, TargetEmitter, native.rs.

#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]

pub mod lexer {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)] pub enum Token<'a> { Identifier(&'a [u8]), Literal(u32), Semicolon, Eof, Error(&'static str) }
    pub struct Lexer<'a> { stream: &'a [u8], cursor: usize }
    impl<'a> Lexer<'a> {
        pub fn new(s: &'a [u8]) -> Self { Self{stream:s,cursor:0} }
        pub fn next_token(&mut self) -> Token<'a> {
            while self.cursor < self.stream.len() && self.stream[self.cursor]==b' ' { self.cursor+=1; }
            if self.cursor >= self.stream.len() { return Token::Eof; }
            let b = self.stream[self.cursor];
            if b.is_ascii_alphabetic() {
                let s=self.cursor; while self.cursor < self.stream.len() && self.stream[self.cursor].is_ascii_alphanumeric() { self.cursor+=1; }
                Token::Identifier(&self.stream[s..self.cursor])
            } else if b.is_ascii_digit() {
                let mut v=0u32; while self.cursor < self.stream.len() && self.stream[self.cursor].is_ascii_digit() { v=v*10+(self.stream[self.cursor]-b'0') as u32; self.cursor+=1; }
                Token::Literal(v)
            } else { self.cursor+=1; Token::Error("unexpected") }
        }
    }
}
pub mod emitter {
    #[derive(Debug,Copy,Clone)] pub enum EmitError { Overflow, BadRegister }
    pub trait TargetEmitter {
        fn emit_mov_imm(&mut self, reg: u8, imm: u32) -> Result<(), EmitError>;
        fn emit_ret(&mut self) -> Result<(), EmitError>;
        fn bytes_written(&self) -> usize;
    }
}
pub mod parser { pub struct Compiler; impl Compiler { pub const fn new() -> Self { Self } } }
