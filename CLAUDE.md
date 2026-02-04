# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Perry is a native TypeScript compiler written in Rust that compiles TypeScript source code directly to native executables. It uses SWC for TypeScript parsing and Cranelift for code generation.

**Current Version:** 0.2.81

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
BIGINT_TAG    = 0x7FFA_0000_0000_0000  // BigInt pointer (lower 48 bits)
STRING_TAG    = 0x7FFF_0000_0000_0000  // String pointer (lower 48 bits)
POINTER_TAG   = 0x7FFD_0000_0000_0000  // Object/Array pointer (lower 48 bits)
INT32_TAG     = 0x7FFE_0000_0000_0000  // Int32 value (lower 32 bits)
```

### Important Runtime Functions

- `js_nanbox_string(ptr)` - Wrap a string pointer with STRING_TAG
- `js_nanbox_pointer(ptr)` - Wrap an object/array pointer with POINTER_TAG
- `js_nanbox_bigint(ptr)` - Wrap a BigInt pointer with BIGINT_TAG
- `js_nanbox_get_bigint(f64)` - Extract BigInt pointer from NaN-boxed value
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

## Cross-Platform Development

Perry supports development on macOS with deployment to Linux via multiple methods:

### GitHub Actions CI/CD (Templates)
- `templates/github-actions/ci.yml` - Tests on Ubuntu and macOS for every push/PR
- `templates/github-actions/release.yml` - Builds release binaries on version tags
- Copy to `.github/workflows/` to activate

### Docker Support
- `Dockerfile` - Multi-stage build with `compiler` and `runtime` targets
- `Dockerfile.dev` - Full development environment with Rust toolchain
- `docker-compose.yml` - Development setup with MySQL, Redis, PostgreSQL for testing

### Quick Start
```bash
# Build Linux binary via Docker
docker compose run --rm perry myfile.ts -o myfile

# Development with test databases
docker compose up -d mysql redis
docker compose run --rm perry-dev cargo test
```

See `docs/CROSS_PLATFORM.md` for detailed documentation on:
- GitHub Actions workflows
- Docker compilation
- Cross-compilation with `cross`
- Alternative approaches (Multipass, Lima, Codespaces, Nix)

## Recent Fixes (v0.2.37-0.2.81)

**Milestone: v0.2.49** - Full production worker running as native binary (MySQL, LLM APIs, string parsing, scoring)

### v0.2.81
- Fix cross-module function calls via re-exports causing argument count mismatch errors
  - Extended the `imported_func_param_counts` propagation to handle `export * from "./module"` re-exports
  - Previously only direct imports were tracked, so re-exported functions with optional params would fail
  - Example: If module B exports `queryFunc(a, b, c?)` and module A has `export * from "./B"`,
    imports from A would not know the full param count, causing wrapper signature mismatches
  - Fix: Re-export propagation loop now iterates over both classes AND functions until no new entries are added
  - Supports chained re-exports (A re-exports B which re-exports C)

### v0.2.80
- Add codegen integration for Fastify HTTP framework
  - Added "fastify" to NATIVE_MODULES list in ir.rs
  - Added 30+ extern function declarations in codegen.rs for all Fastify FFI functions
  - Added NativeMethodCall mappings for all Fastify app methods:
    - Constructor: `Fastify()` / `Fastify({ options })` via default export pattern
    - Route methods: `app.get()`, `app.post()`, `app.put()`, `app.delete()`, etc.
    - Lifecycle: `app.addHook()`, `app.setErrorHandler()`, `app.register()`
    - Server: `app.listen()`
  - Added context/request/reply method mappings for handlers
  - Added fastify to modules using `js_nanbox_get_pointer` for handle extraction
  - Added HIR lowering for default import function calls (e.g., `import F from 'fastify'; F()`)
    - Uses method name "default" for default export calls
  - Full TypeScript API now supported:
    ```typescript
    import Fastify from 'fastify';
    const app = Fastify();
    app.get('/users/:id', async (req, reply) => {
      return { id: req.params.id };
    });
    app.listen({ port: 3000 });
    ```

### v0.2.79
- Add Fastify-compatible native HTTP framework runtime
  - New module: `crates/perry-stdlib/src/fastify/` with Fastify-like API
  - `mod.rs` - Core data structures (FastifyApp, Route, Hooks, RoutePattern)
  - `router.rs` - Route pattern parsing and matching (supports `:param` and `*` wildcard)
  - `context.rs` - Unified context for Fastify and Hono style handlers
  - `app.rs` - Route registration, hooks, plugins
  - `server.rs` - Hyper-based server with event loop
  - FFI functions for route handlers, request/response context
  - Supports: GET/POST/PUT/DELETE/PATCH/HEAD/OPTIONS routes
  - Supports: Lifecycle hooks (onRequest, preHandler, etc.)
  - Supports: Plugins with URL prefix
  - Supports: Hono-style context methods (c.json(), c.text(), c.req.param())
  - Re-exported promise functions (js_promise_run_microtasks, js_promise_state, js_is_promise) for stdlib

### v0.2.78
- Fix cross-module function calls with optional parameters causing signature mismatch errors
  - Functions with optional parameters can now be called with different argument counts from other modules
  - Example: `executeQuery(query, params?, options?)` can be called as `executeQuery('SELECT 1')`,
    `executeQuery('SELECT ?', [42])`, or `executeQuery('SELECT ?', [42], { timeout: 1000 })`
  - Root cause: wrapper functions were being declared with call-site arity instead of full function signature
  - Fix: Added `imported_func_param_counts` map to propagate function param counts between modules during compilation
  - When calling with fewer args than params, missing arguments are padded with `undefined`
  - Also added fallback: if wrapper is already declared with different signature, find existing declaration and adapt

### v0.2.77
- Add GitHub Actions CI/CD workflow templates (in `templates/github-actions/`)
  - `ci.yml` - Tests on Ubuntu and macOS, uploads build artifacts
  - `release.yml` - Builds release binaries for Linux x86_64, macOS x86_64/aarch64
  - Templates are not active by default; copy to `.github/workflows/` to enable
- Add Docker support for cross-platform development
  - `Dockerfile` - Multi-stage build (builder, compiler, runtime stages)
  - `Dockerfile.dev` - Development environment with full Rust toolchain
  - `docker-compose.yml` - Dev setup with MySQL, Redis, PostgreSQL services
  - `.dockerignore` - Excludes unnecessary files from Docker build context
- Add cross-platform development documentation (`docs/CROSS_PLATFORM.md`)
  - Covers GitHub Actions, Docker, cross-compilation, and alternative approaches

### v0.2.76
- Fix Error objects not displaying in console.log/console.error
  - Added Error object formatting in `format_jsvalue()` and `format_jsvalue_for_json()` in builtins.rs
  - Error objects now display as "Error: <message>" instead of empty/invalid output
  - Check `OBJECT_TYPE_ERROR` tag to distinguish Error objects from regular objects/arrays
- Fix `new Error(message)` passing corrupted message string
  - Use `js_get_string_pointer_unified` to extract string pointer from NaN-boxed value in codegen
- Fix console.error with multiple arguments not displaying values
  - Added multi-argument spread support for console.error (was only implemented for console.log)
  - console.error("prefix:", errorObj, "suffix") now works correctly

### v0.2.75
- Fix ethers.js module returning wrong values
  - Added `getAddress()` - returns EIP-55 checksummed Ethereum addresses
  - Added `parseEther()` - parses ether string to BigInt wei (uses 18 decimals)
  - Added `formatEther()` - formats BigInt wei to ether string (uses 18 decimals)
  - Fixed `parseUnits()` and `parseEther()` returning garbage values
    - Added `is_bigint_expr` detection for NativeMethodCall returning BigInt
    - Now properly marks local variables as `is_bigint` when assigned from ethers BigInt functions
  - Fixed chained `toString()` on BigInt-returning ethers methods (e.g., `parseEther('1.5').toString()`)
    - Added special handling in codegen to detect BigInt-returning NativeMethodCall and call `js_bigint_to_string`
  - Implemented Keccak-256 hash for EIP-55 address checksumming in pure Rust

### v0.2.74
- Fix pool.getConnection() - full support for getting connections and calling methods on them
  - Extended `detect_native_instance_creation_with_context` to track variables assigned from `await pool.getConnection()`
  - `PoolConnection` class type now tracked through await expressions
  - Added `js_mysql2_pool_connection_execute` for prepared statements on pool connections
  - Codegen now routes `PoolConnection` methods to correct FFI functions (`query`, `execute`, `release`)
  - Fixed `js_mysql2_pool_connection_release` to enter tokio runtime context before dropping connection
  - Full example now works: `const conn = await pool.getConnection(); await conn.query(...); conn.release();`

### v0.2.73
- Partial fix for pool.getConnection() - connection acquisition works but method calls on the connection crash
  - Implemented `MysqlPoolConnectionHandle` wrapper with proper connection lifecycle
  - `js_mysql2_pool_get_connection` now acquires actual connections from the pool
  - Connection handle is NaN-boxed with POINTER_TAG for proper extraction
  - Promise resolution correctly passes the NaN-boxed handle to JavaScript
  - `js_mysql2_connection_query` now handles both regular and pool connections
  - ~~**Known issue**: After `await pool.getConnection()`, calling methods like `conn.release()` crashes~~ (fixed in v0.2.74)

### v0.2.72
- Fix mysql2 config parsing using wrong fields
  - Changed `parse_mysql_config` to use field names (`host`, `user`, `password`, etc.) instead of field indices
  - Now uses `js_object_get_field_by_name` for proper field lookup
  - Fixes issue where `user` and `password` fields were swapped
- Fix pool.getConnection() SIGSEGV crash
  - Added extern function declarations for `js_mysql2_pool_get_connection` and `js_mysql2_pool_connection_release`
  - Added method mappings in codegen for `getConnection` and `release`
  - Now returns proper error message instead of crashing

### v0.2.71
- Fix BigInt.toString() SIGSEGV crash
  - BigInt variables were stored as I64 (raw pointer) but values were NaN-boxed F64
  - Changed BigInt storage to F64 (is_pointer=false) to match NaN-boxed representation
  - BigInt.toString() now correctly extracts pointer via `js_nanbox_get_bigint`

### v0.2.70
- Fix ethers.formatUnits() and ethers.parseUnits() SIGSEGV crash
  - formatUnits: Extract BigInt from NaN-boxed value using `js_nanbox_get_bigint`
  - parseUnits: Extract string from NaN-boxed value using `js_get_string_pointer_unified`
  - formatUnits: NaN-box return string with STRING_TAG
  - parseUnits: NaN-box return BigInt with BIGINT_TAG

### v0.2.69
- Fix parseInt() and parseFloat() SIGSEGV crash
  - String arguments were bitcast instead of properly extracted from NaN-boxed values
  - Now use `js_get_string_pointer_unified` to extract the raw string pointer
  - `parseInt(process.env.REDIS_PORT || '6379')` now works correctly

### v0.2.68
- Fix ioredis `new Redis()` returning number instead of object when called without await
  - `new Redis()` now works synchronously like real ioredis (connects lazily on first command)
  - Changed `js_ioredis_new` to return Handle synchronously instead of Promise
  - NaN-box returned handle with POINTER_TAG so it's recognized as an object
  - Add ioredis to the list of modules that use `js_nanbox_get_pointer` for method calls
- This matches real ioredis API where `new Redis()` is synchronous and connection happens lazily

### v0.2.67
- Fix native instance method calls returning 0 when instance is awaited
  - `await new Redis()`, `await new WebSocket()`, etc. now properly register native instances
  - HIR lowering now handles `ast::Expr::Await(ast::Expr::New(...))` pattern
  - Methods like `redis.set()`, `redis.get()` now correctly call the native FFI functions
  - Added handling in both exported variable declarations and local variable declarations

### v0.2.66
- Fix await not propagating promise rejections (SIGSEGV crash)
  - Added `js_promise_reason()` runtime function to get rejection reason
  - Updated await codegen to check if promise was rejected and throw the rejection reason
  - Await now properly handles both I64 (raw pointer) and F64 (NaN-boxed) promise values
  - Functions returning `Promise<T>` type now work correctly with await rejection handling

### v0.2.65
- Fix async error strings using wrong NaN-box tag (POINTER_TAG instead of STRING_TAG)
  - Error messages from async operations (mysql2, redis, fetch, etc.) now use `JSValue::string_ptr()`
    instead of `JSValue::pointer()` for proper type identification
  - Fixed in spawn_for_promise, spawn_for_promise_deferred, and create_error_value
- This fixes crashes when error messages were being printed or handled as object pointers

### v0.2.64
- Fix JS runtime BigInt conversion - V8 BigInt values now properly converted to native Perry BigInt
  - Added BigInt handling in `v8_to_native()` to convert V8 BigInt to native BigIntHeader
  - Added BigInt handling in `native_to_v8()` to convert native BigInt back to V8
  - Uses BIGINT_TAG (0x7FFA) for NaN-boxing BigInt pointers
- Fix JS runtime module loading for bare module specifiers (e.g., "ethers", "@noble/hashes")
  - `js_load_module` now properly resolves bare module names using node_modules resolution
  - Added NodeModuleLoader integration for consistent module resolution
- Add Node.js built-in module stubs for JS runtime compatibility
  - Stub implementations for: net, tls, http, https, crypto, fs, path, os, stream, buffer,
    util, events, assert, url, querystring, string_decoder, zlib
  - Note: Ethers.js still requires CommonJS require() support which is partially implemented

### v0.2.63
- Fix Cranelift verifier type mismatch errors when passing string/pointer values to certain functions
- Fix Array.includes() with string values - NaN-box string values and use jsvalue comparison for proper content matching
- Fix Set.has(), Set.add(), Set.delete() with string values - NaN-box strings for proper comparison
- Fix function call arguments with i32 type (from loop optimization) not being converted to f64/i64
  - Added i32 -> f64 conversion using `fcvt_from_sint`
  - Added i32 -> i64 conversion using `sextend`
  - Fixed in: FuncRef calls, ExternFuncRef calls, closure calls
- Add js_closure_call4 support for closures with 4 arguments

### v0.2.62
- Fix mysql2 pool.query() hanging indefinitely when MySQL server is unavailable
- Added timeouts to all mysql2 operations to prevent indefinite hangs:
  - Pool acquire timeout: 10 seconds (when getting connection from pool)
  - Query timeout: 30 seconds (wraps all query operations with tokio::time::timeout)
  - Connection timeout: 10 seconds (for createConnection and close operations)
- Operations now error gracefully with descriptive messages instead of hanging:
  - "Query timed out after 30 seconds (MySQL server may be unavailable)"
  - "Connection timed out after 10 seconds (MySQL server may be unavailable)"
- Affected functions in pool.rs: createPool, pool.query, pool.execute, pool.end
- Affected functions in connection.rs: createConnection, connection.query,
  connection.end, beginTransaction, commit, rollback

### v0.2.61
- Fix Promise.all returning tiny float numbers instead of string values with async promises
- Root cause: When capturing string variables in closures, raw I64 pointers were bitcast to F64
  instead of being properly NaN-boxed with STRING_TAG (0x7FFF)
- Fix 1 (capture storage): When storing captured string/pointer values in closures, use
  `js_nanbox_string` for strings and `js_nanbox_pointer` for objects/arrays instead of raw bitcast
- Fix 2 (closure calls): Always use `js_closure_call*` functions when calling local variables
  (they must be closures if being called), instead of requiring `is_closure` flag to be set
- Affected pattern: `async function delay(ms, value) { return new Promise(resolve => setTimeout(() => resolve(value), ms)); }`
  - The `value` parameter was extracted from NaN-box to I64 pointer for efficiency
  - When captured by inner closure `() => resolve(value)`, the I64 was incorrectly bitcast to F64
  - This produced tiny denormalized floats like `2.18e-308` when printed

### v0.2.60
- Fix ioredis SIGSEGV crash when calling Redis methods (set, get, etc.)
- Root causes fixed:
  1. **Codegen**: ioredis connection IDs are simple f64 numbers (1.0, 2.0, etc.), not NaN-boxed pointers
     - Changed from `js_nanbox_get_pointer` to `fcvt_to_sint` for extracting connection handles
     - Same pattern as fetch response IDs
  2. **Runtime**: String values from Redis operations must be allocated on main thread
     - Changed from `queue_promise_resolution` to `queue_deferred_resolution` for string results
     - Strings created in async Tokio workers were using invalid thread-local arenas
  3. **NaN-boxing**: Redis result strings should use STRING_TAG (0x7FFF), not POINTER_TAG (0x7FFD)
     - Changed all `JSValue::pointer(str as *const u8)` to `JSValue::string_ptr(str)`
  4. **Symbol collision**: Renamed `js_call_method` to `js_native_call_method` in codegen
     - Matches the symbol rename done in perry-runtime v0.2.59
- Note: ioredis API in Perry returns a Promise from `new Redis()`, use `await new Redis()` pattern

### v0.2.59
- Fix ethers.js duplicate symbol linker error when using perry-jsruntime
- Root cause: Both `perry-runtime` and `perry-jsruntime` defined `js_call_method` and `js_call_value`
  - `perry-runtime/src/object.rs` had `js_call_method` for native closure dispatch
  - `perry-runtime/src/closure.rs` had `js_call_value` for native closure calls
  - `perry-jsruntime/src/interop.rs` had the same functions for V8 JavaScript calls
- When linking with jsruntime (which includes runtime via re-exports), both definitions conflicted
- Solution: Rename the native closure versions to avoid collision:
  - `js_call_method` -> `js_native_call_method` in perry-runtime/src/object.rs
  - `js_call_value` -> `js_native_call_value` in perry-runtime/src/closure.rs
- The V8 versions in perry-jsruntime keep the original names (used by codegen for JS runtime fallback)

### v0.2.58
- Fix mysql2 pool.query() and pool.execute() hanging indefinitely
- Root cause: perry-runtime uses **thread-local arenas** for memory allocation
- Async database operations run on tokio worker threads, but JSValue allocation happened there
- Memory allocated on worker threads was invalid/inaccessible from the main thread
- Solution: Implement deferred JSValue creation with `spawn_for_promise_deferred()`
  1. Async block extracts raw Rust data on worker thread (no JSValue allocation)
  2. Raw data is queued with a converter function
  3. Converter runs on main thread during `js_stdlib_process_pending()`
  4. JSValues created safely using main thread's arena allocator
- Added `RawQueryResult`, `RawRowData`, `RawColumnInfo`, `RawValue` types for thread-safe data transfer
- Updated mysql2 pool.query(), pool.execute(), connection.query() to use deferred conversion
- Also fixed error string creation in spawn_for_promise - now deferred to main thread

### v0.2.57
- Fix cross-module array exports returning garbage (e.g., `9222246136947933184` instead of array)
- Arrays exported from one module and imported in another were not properly NaN-boxed
- Root causes fixed:
  1. Export side: NaN-box array pointers with POINTER_TAG when storing to export globals
  2. Import side: HIR lowering now generates proper array method expressions (ArrayJoin, ArrayMap, etc.) for imported arrays via `ExternFuncRef`
  3. Codegen: All array methods (join, map, filter, forEach, reduce) now detect `ExternFuncRef` and extract pointer from NaN-boxed value using `js_nanbox_get_pointer`
  4. PropertyGet: Handle `.length` on `ExternFuncRef` arrays using `js_dynamic_array_length`
- Test results: `CHAIN_NAMES.join(', ')` now returns `"ethereum, base, bnb"` instead of garbage

### v0.2.56
- Fix `string.split('').slice(0, 5)` returning empty array
- Issue: array slice was using `js_string_slice` instead of `js_array_slice` for arrays
- Root causes fixed:
  1. Add `split` to methods that return arrays in local variable type inference
  2. Mark `split()` results as NaN-boxed arrays (`is_pointer = false`, `is_array = true`)
  3. Add special handling for `.slice()` on arrays to call `js_array_slice`
  4. Detect array slice for chained calls like `str.split('').slice()` by checking callee method
  5. Extract array pointer from NaN-boxed value using `js_nanbox_get_pointer` before calling `js_array_slice`

### v0.2.55
- Implement Promise.all() - takes array of promises, returns promise that resolves with array of results
- Add `js_promise_all(promises_arr: i64)` runtime function in promise.rs
- Handles empty arrays (resolves immediately with empty array)
- Handles mixed promises and non-promise values
- Properly waits for all promises to resolve before completing
- Rejects immediately if any promise rejects

### v0.2.54
- Fix ioredis "Unknown class: Redis" error
- Add handler for `new Redis(config?)` in Expr::New codegen
- Register Redis as a native handle class (uses f64, not i64 pointers)
- `new Redis()` now correctly calls `js_ioredis_new` and returns a Promise

### v0.2.53
- Fix `array.join()` returning garbage - NaN-box result with STRING_TAG instead of bitcast
- Fix `string.includes()` and `array.includes()` returning 1/0 instead of true/false
- Fix Promise unwrapping when async function returns `new Promise(...)`
- Add `js_promise_resolve_with_promise` runtime function for Promise chaining
- When async function returns a Promise, outer promise now adopts inner promise's eventual value

### v0.2.52
- Fix async/await returning garbage data from nested async function calls
- Await results are already NaN-boxed values, not raw pointers - set `is_pointer = false` to prevent double-boxing
- Previously, returning an await result would strip STRING_TAG and incorrectly re-box with POINTER_TAG

### v0.2.51
- Fix boolean representation - use NaN-boxed TAG_TRUE/TAG_FALSE (0x7FFC_0000_0000_0004/0003) instead of 0.0/1.0
- Fix boolean comparison - use integer comparison on bit patterns instead of fcmp (NaN != NaN)
- Fix console.log boolean literals - route through js_console_log_dynamic for proper formatting
- Fix array printing crash (SIGSEGV) - check array validity before accessing object keys_array
- Add JSON-like object formatting to console.log output with format_object_as_json and format_jsvalue_for_json
- Improve array/object detection in format_jsvalue to safely handle pointers

### v0.2.50
- Fix critical BigInt corruption - BigInt values were being stored as bitcast pointers instead of NaN-boxed values
- Add BIGINT_TAG (0x7FFA) for proper BigInt NaN-boxing
- Add `js_nanbox_bigint(ptr)`, `js_nanbox_get_bigint(f64)`, `js_nanbox_is_bigint(f64)` runtime functions
- Add `is_bigint()`, `as_bigint_ptr()`, `bigint_ptr()` methods to JSValue
- Update BigInt literal compilation to use NaN-boxing
- Update BigInt arithmetic to extract pointers via `js_nanbox_get_bigint` before operations
- Add BigInt comparison support using `js_bigint_cmp`
- Update `format_jsvalue` to detect BigInt and format with "n" suffix
- Fix BigInt function parameters - set `is_bigint` flag based on parameter type
- Change BigInt ABI from i64 to f64 (NaN-boxed) for consistent handling
- BigInt addition, subtraction, multiplication, division, comparisons now work correctly
- BigInt in function parameters and nested expressions now work correctly

### v0.2.48
- Fix string.split() returning corrupted array elements
- NaN-box string pointers with STRING_TAG when storing in split result array

### v0.2.46
- Fix string.split(), indexOf(), includes(), startsWith(), endsWith() SIGSEGV
- Fix ArrayIndexOf/ArrayIncludes HIR to detect string vs array and use correct runtime functions
- Extract NaN-boxed string pointers for all string method arguments (needle, delimiter, prefix, suffix, etc.)

### v0.2.44
- Fix string `===` comparison SIGSEGV - extract string pointers from NaN-boxed values
- Fix switch statements with string cases - use `js_string_equals` instead of `fcmp`

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

### Async operations hanging or returning garbage
- **Root cause**: perry-runtime uses thread-local arenas for memory allocation
- Async operations (mysql2, pg, etc.) run on tokio worker threads
- JSValue objects created on worker threads use the wrong arena
- **Solution**: Use `spawn_for_promise_deferred()` instead of `spawn_for_promise()`
- Return raw Rust data from async block, convert to JSValue on main thread
- The converter function runs during `js_stdlib_process_pending()` on main thread
