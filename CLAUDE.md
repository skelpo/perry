# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Perry is a native TypeScript compiler written in Rust that compiles TypeScript source code directly to native executables. It uses SWC for TypeScript parsing and Cranelift for code generation.

## Build Commands

```bash
# Build all crates (debug)
cargo build

# Build all crates (release)
cargo build --release

# Build just the CLI
cargo build -p perry

# Build the runtime library (required for linking)
cargo build --release -p perry-runtime

# Run tests
cargo test

# Run tests for a specific crate
cargo test -p perry-hir

# Run a specific test
cargo test -p perry-parser test_name

# Check code without building
cargo check

# Format code
cargo fmt

# Lint code
cargo clippy
```

## Compiling TypeScript

```bash
# Compile a TypeScript file to executable
cargo run -- test_factorial.ts

# Compile with custom output name
cargo run -- test_factorial.ts -o factorial

# Print HIR for debugging
cargo run -- test_factorial.ts --print-hir

# Produce object file only (no linking)
cargo run -- test_factorial.ts --no-link

# Keep intermediate .o files
cargo run -- test_factorial.ts --keep-intermediates
```

## Architecture

The compiler follows a multi-stage pipeline:

```
TypeScript (.ts) → Parse (SWC) → AST → Lower → HIR → Transform → Codegen (Cranelift) → .o → Link (cc) → Executable
```

### Crate Structure

- **perry** - CLI driver that orchestrates the pipeline
- **perry-parser** - SWC wrapper for TypeScript parsing
- **perry-types** - Type system definitions (Void, Boolean, Number, String, Array, Object, Function, Union, Promise, etc.)
- **perry-hir** - High-level IR structures and AST→HIR lowering
  - `ir.rs` - HIR data structures (Module, Class, Function, Statement, Expression)
  - `lower.rs` - Lowering context and AST to HIR conversion
- **perry-transform** - IR transformation passes (closure conversion, async lowering)
- **perry-codegen** - Cranelift-based native code generation
- **perry-runtime** - Runtime library linked into executables
  - `value.rs` - JSValue representation (NaN-boxing)
  - `object.rs`, `array.rs`, `string.rs`, `bigint.rs`, `closure.rs` - Heap types
  - `builtins.rs` - Built-in functions (console.log, etc.)
- **perry-stdlib** - Standard library (placeholder for Node.js API support)

### Key Data Flow

1. `perry_parser::parse_typescript()` produces SWC's `Module` AST
2. `perry_hir::lower_module()` converts AST to typed HIR with unique IDs
3. `perry_codegen::Compiler::compile_module()` generates native object code
4. System linker (`cc`) links object file with `libperry_runtime.a`

### HIR Structure

The HIR (`crates/perry-hir/src/ir.rs`) represents a simplified, typed intermediate form:
- **Module**: Contains globals, functions, classes, and init statements
- **Function**: Name, params with types, return type, body, async flag
- **Class**: Name, fields, constructor, instance/static methods
- **Statement**: Let, Expr, Return, If, While, For, Break, Continue, Throw, Try
- **Expression**: Literals, variable access (LocalGet/Set, GlobalGet/Set), operations, calls, object/array literals

## Test Files

Root-level `test_*.ts` files serve as integration tests for various language features:
- `test_factorial.ts` - Recursive functions
- `test_for.ts` - For loop
- `test_break_continue.ts` - Break and continue statements
- `test_class.ts`, `test_class_method.ts` - Class definitions
- `test_array.ts`, `test_array_loop.ts` - Array operations
- `test_bigint.ts` - BigInt support
- `test_closure.ts` - Closure handling
- `test_string.ts` - String operations

To test a feature, compile and run:
```bash
cargo run --release -- test_factorial.ts && ./test_factorial
```
