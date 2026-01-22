//! Cranelift Code Generation for Perry
//!
//! Translates HIR to Cranelift IR and generates native machine code.

pub mod codegen;

pub use codegen::Compiler;
