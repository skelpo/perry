# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Perry is a native TypeScript compiler written in Rust that compiles TypeScript source code directly to native executables. It uses SWC for TypeScript parsing and Cranelift for code generation.

**Current Version:** 0.2.43

## Workflow Requirements

**IMPORTANT:** Follow these practices for every code change:

1. **Update CLAUDE.md**: After making any code changes, update this file to document:
   - New features or fixes in the "Recent Fixes" section
   - Any new patterns, APIs, or important implementation details
   - Changes to build commands or architecture

2. **Increment Version**: Bump the version number with every change:
   - Use patch increments (e.g., 0.2.40 → 0.2.41) for bug fixes and small changes
   - Use minor increments (e.g., 0.2.x → 0.3.0) for new features
   - Update the "Current Version" field at the top of this file

3. **Commit Changes**: Always commit after completing a change:
   - Include both the code changes and CLAUDE.md updates in the same commit
   - Use descriptive commit messages that summarize the change

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
  - `promise.rs` - Promise implementation with closure-based callbacks
  - `builtins.rs` - Built-in functions (console.log, etc.)
- **perry-stdlib** - Standard library (Node.js API support: mysql2, redis, fetch, etc.)
- **perry-jsruntime** - JavaScript interop via QuickJS

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

## NaN-Boxing Implementation

Perry uses NaN-boxing to represent JavaScript values efficiently in 64 bits. Key tag constants in `perry-runtime/src/value.rs`:

```rust
TAG_UNDEFINED = 0x7FFC_0000_0000_0001  // undefined value
TAG_NULL      = 0x7FFC_0000_0000_0002  // null value
TAG_FALSE     = 0x7FFC_0000_0000_0003  // false
TAG_TRUE      = 0x7FFC_0000_0000_0004  // true
STRING_TAG    = 0x7FFF_0000_0000_0000  // String pointer (lower 48 bits)
POINTER_TAG   = 0x7FFD_0000_0000_0000  // Object/Array pointer (lower 48 bits)
INT32_TAG     = 0x7FFE_0000_0000_0000  // Int32 value (lower 32 bits)
```

### Important Runtime Functions

- `js_nanbox_string(ptr)` - Wrap a string pointer with STRING_TAG
- `js_nanbox_pointer(ptr)` - Wrap an object/array pointer with POINTER_TAG
- `js_get_string_pointer_unified(f64)` - Extract raw pointer from NaN-boxed or raw string
- `js_jsvalue_to_string(f64)` - Convert any NaN-boxed value to string
- `js_is_truthy(f64)` - Proper JavaScript truthiness semantics

### Module-Level Variables

Module-level variables are stored in global data slots:
- **Strings**: Stored as F64 (NaN-boxed), NOT I64 raw pointers
- **Arrays/Objects**: Stored as I64 (raw pointers)
- Functions access module variables via `module_var_data_ids` mapping

## Promise System

Promises use closure-based callbacks (`ClosurePtr`) instead of raw function pointers:

```rust
pub type ClosurePtr = *const crate::closure::ClosureHeader;

pub struct Promise {
    state: PromiseState,
    value: f64,
    reason: f64,
    on_fulfilled: ClosurePtr,  // Closure, not raw fn pointer
    on_rejected: ClosurePtr,
    next: *mut Promise,
}
```

Callbacks are invoked via `js_closure_call1(closure, value)` which properly passes the closure environment.

## Known Working Features

- Basic arithmetic, comparisons, logical operators
- Variables, constants, type annotations
- Functions (regular, async, arrow, closures)
- Classes with constructors, methods, inheritance
- Arrays with methods (push, pop, map, filter, find, etc.)
- Objects with property access (dot and bracket notation)
- Template literals with interpolation
- Promises with .then(), .catch(), .finally()
- Promise.resolve(), Promise.reject()
- async/await
- try/catch/finally
- fetch() with custom headers
- Multi-module compilation with imports/exports

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

## Recent Fixes (v0.2.37-0.2.42)

### v0.2.42
- Fix native module method calls (pool.execute, redis.get, etc.) crashing with SIGSEGV
- Extract raw pointers from NaN-boxed objects using `js_nanbox_get_pointer` for:
  mysql2, ioredis, ws, events, lru-cache, commander, decimal.js, big.js,
  bignumber.js, pg, mongodb, better-sqlite3, sharp, cheerio, nodemailer,
  dayjs, moment, node-cron, rate-limiter-flexible
- Extract NaN-boxed string arguments properly for SQL queries, Redis keys,
  WebSocket messages, and EventEmitter event names
- Extract NaN-boxed array pointers for execute params

### v0.2.41
- Fix mysql.createPool() returning number instead of object
- NaN-box native module return values with POINTER_TAG

### v0.2.40
- Fix Promise.catch() crash - closures invoked properly with js_closure_call1
- Add Promise.reject() static method
- Fix bracket notation `obj['key']` SIGSEGV
- Fix module-level const in template literals SIGSEGV
- Improve string concatenation fallback handling

### v0.2.39
- Promise callback system rewritten to use ClosurePtr

### v0.2.38
- Fix bracket notation property access for NaN-boxed string keys

### v0.2.37
- Fix undefined truthiness (undefined now properly falsy)
- NaN-box string literals with STRING_TAG
- Fix fetch() with NaN-boxed URL strings
- Add js_is_truthy() runtime function
- Fix uninitialized variables (now TAG_UNDEFINED, not 0.0)
- Special handling for undefined/null/NaN/Infinity identifiers

## Debugging Tips

1. **Print HIR**: Use `--print-hir` to see the intermediate representation
2. **Keep object files**: Use `--keep-intermediates` to inspect .o files
3. **Check value types**: NaN-boxed values can be inspected by their bit patterns
4. **Module init order**: Entry module calls `_perry_init_*` for each imported module

## Common Issues

### SIGSEGV in string operations
- Check if string pointers are being extracted from NaN-boxed values
- Use `js_get_string_pointer_unified()` for strings that might be NaN-boxed

### Promise callbacks not firing
- Ensure callbacks are closures, not raw function pointers
- Check that `js_promise_run_microtasks()` is being called in the event loop

### Cross-module variable access
- Module-level strings are F64 (NaN-boxed), not I64 pointers
- Check `module_level_locals` for proper type info
