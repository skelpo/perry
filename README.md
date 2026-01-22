# Perry

A native TypeScript compiler written in Rust that compiles TypeScript source code directly to native executables.

**Status:** 59/59 tests passing | Active Development

## Overview

Perry takes TypeScript code and compiles it directly to native machine code. By default, it produces standalone executables with no runtime dependencies. For compatibility with pure JavaScript npm packages, an optional V8 runtime can be embedded. It uses [SWC](https://swc.rs/) for TypeScript parsing and [Cranelift](https://cranelift.dev/) for native code generation.

## Example Applications

Perry successfully compiles real-world TypeScript applications. All examples in the `example-code/` directory compile to native executables:

| Example | Description | Runtime Mode |
|---------|-------------|--------------|
| **http-server** | Basic HTTP server | Native only |
| **express-postgres** | Express.js with PostgreSQL | V8 runtime |
| **koa-redis** | Koa.js with Redis caching | V8 runtime |
| **fastify-redis-mysql** | Fastify with Redis + MySQL | V8 runtime |
| **blockchain-demo** | Blockchain simulation with WebSocket | V8 runtime |
| **hono-mongodb** | Hono.js with MongoDB + JWT auth | V8 runtime |

```bash
# Compile and run an example
cargo run --release -- example-code/http-server/main.ts
./main

# Examples requiring npm packages need V8 runtime
cargo run --release -- example-code/hono-mongodb/src/index.ts --enable-js-runtime
./index
```

### Key Features

- **No Runtime Required** - Produces standalone native executables (default mode)
- **Optional V8 Runtime** - Embed V8 for JavaScript npm package compatibility (`--enable-js-runtime`)
- **Fast Compilation** - Direct TypeScript to native code compilation
- **Small Binaries** - Output binaries are typically 2-5MB (15-20MB with V8)
- **Type-Safe** - Leverages TypeScript's type system for optimization
- **Comprehensive Standard Library** - Built-in implementations of fs, path, crypto, Date, JSON, Math, etc.

## Performance

Perry generates highly optimized native code that outperforms Node.js on most benchmarks. The compiler includes several optimizations:

- **FMA (Fused Multiply-Add)** - Combines multiply and add into single instructions
- **CSE (Common Subexpression Elimination)** - Caches repeated computations like `x*x`
- **Loop Unrolling** - Unrolls tight loops up to 8x to reduce branch overhead
- **Bounds Check Elimination** - Removes redundant array bounds checks
- **Native i32 Loop Counters** - Uses integer registers for loop variables

### Benchmark Results

*Median of 5 runs on macOS ARM64 (Apple Silicon) with Node.js v24*

| Benchmark | Perry | Node.js | Speedup |
|-----------|-----------|---------|---------|
| loop_overhead | 50ms | 59ms | **1.2x** |
| array_write | 7ms | 9ms | **1.3x** |
| array_read | 4ms | 12ms | **3.0x** |
| fibonacci | 621ms | 1318ms | **2.1x** |
| math_intensive | 22ms | 66ms | **3.0x** |
| object_create | 2ms | 7ms | **3.5x** |
| string_concat | 2ms | 5ms | **2.5x** |
| method_calls | 5ms | 15ms | **3.0x** |
| nested_loops | 25ms | 30ms | **1.2x** |
| prime_sieve | 6ms | 8ms | **1.3x** |
| binary_trees | 3ms | 8ms | **2.7x** |
| factorial | 70ms | 116ms | **1.7x** |
| closure | 14ms | 63ms | **4.5x** |
| mandelbrot | 25ms | 47ms | **1.9x** |

**Average speedup: 2.2x faster than Node.js**

> Benchmarks are in `benchmarks/suite/`. Run with: `cd benchmarks/suite && ./run_benchmarks.sh`

## Installation

```bash
# Clone the repository
git clone https://github.com/aspect-build/perry.git
cd perry

# Build in release mode
cargo build --release

# The binary will be at target/release/perry
```

## Quick Start

```bash
# Compile a TypeScript file to an executable
perry build main.ts

# Run with custom output name
perry build main.ts -o myapp

# Run the test suite
./run_tests.sh
```

---

## Supported Features

### Core Language - Full Support

| Feature | Status | Notes |
|---------|--------|-------|
| **Numbers** | ✅ Full | 64-bit floating point (f64) |
| **Strings** | ✅ Full | UTF-8, all common methods |
| **Booleans** | ✅ Full | true/false, logical operators |
| **Arrays** | ✅ Full | Typed and mixed-type arrays |
| **Objects** | ✅ Full | Object literals and field access |
| **BigInt** | ✅ Full | 256-bit integer support |
| **Enums** | ✅ Full | Numeric and string enums |

### Operators - Full Support

| Feature | Status | Notes |
|---------|--------|-------|
| **Arithmetic** | ✅ Full | +, -, *, /, % |
| **Comparison** | ✅ Full | ==, !=, <, >, <=, >=, ===, !== |
| **Logical** | ✅ Full | &&, \|\|, ! |
| **Bitwise** | ✅ Full | &, \|, ^, ~, <<, >>, >>> |
| **Increment/Decrement** | ✅ Full | ++, -- (pre and post) |
| **Compound Assignment** | ✅ Full | +=, -=, *=, /=, %=, **=, &=, \|=, ^=, <<=, >>=, >>>= |
| **Exponentiation** | ✅ Full | ** operator |
| **Ternary** | ✅ Full | condition ? a : b |
| **Nullish Coalescing** | ✅ Full | ?? operator |
| **Optional Chaining** | ✅ Full | obj?.prop, obj?.[index] |

### Control Flow - Full Support

| Feature | Status | Notes |
|---------|--------|-------|
| **If/Else** | ✅ Full | All branching patterns |
| **While Loops** | ✅ Full | Including break/continue |
| **For Loops** | ✅ Full | Standard for loops |
| **For-Of Loops** | ✅ Full | Array iteration |
| **For-In Loops** | ✅ Full | Object key iteration |
| **Switch** | ✅ Full | With fallthrough support |
| **Break/Continue** | ✅ Full | Works in all loops |
| **Try-Catch-Finally** | ✅ Full | Exception handling with throw |

### Functions - Full Support

| Feature | Status | Notes |
|---------|--------|-------|
| **Function Declaration** | ✅ Full | Named functions |
| **Arrow Functions** | ✅ Full | () => {} syntax |
| **Parameters** | ✅ Full | Multiple parameters |
| **Default Parameters** | ✅ Full | Parameters with defaults |
| **Rest Parameters** | ✅ Full | ...args syntax |
| **Return Values** | ✅ Full | Explicit returns |
| **Recursion** | ✅ Full | Self-referential calls |
| **Closures** | ✅ Full | Including mutable captures |
| **Higher-Order Functions** | ✅ Full | Functions as arguments/returns |

### Classes - Full Support

| Feature | Status | Notes |
|---------|--------|-------|
| **Class Declaration** | ✅ Full | Basic class syntax |
| **Constructors** | ✅ Full | With parameters |
| **Instance Fields** | ✅ Full | this.field access |
| **Instance Methods** | ✅ Full | Method calls on instances |
| **Private Fields (#)** | ✅ Full | ES2022 #privateField syntax |
| **Static Methods** | ✅ Full | Class-level methods |
| **Static Fields** | ✅ Full | Class-level properties |
| **Getters/Setters** | ✅ Full | get/set accessors |
| **Inheritance** | ✅ Full | extends keyword |
| **Super Calls** | ✅ Full | super() constructor calls |

### Type System - Full Support

| Feature | Status | Notes |
|---------|--------|-------|
| **Type Annotations** | ✅ Full | Explicit type declarations |
| **Type Inference** | ✅ Full | Automatic type detection |
| **Generics** | ✅ Full | Monomorphization (like Rust) |
| **Interfaces** | ✅ Full | Interface declarations |
| **Type Aliases** | ✅ Full | type X = ... declarations |
| **Union Types** | ✅ Full | string \| number support |
| **Type Guards** | ✅ Full | typeof operator |
| **Constraint Checking** | ✅ Full | T extends Foo constraints |

### Arrays - Full Support

| Feature | Status | Notes |
|---------|--------|-------|
| **Array Literals** | ✅ Full | [1, 2, 3] syntax |
| **Indexing** | ✅ Full | arr[i] read/write |
| **Length** | ✅ Full | arr.length property |
| **push/pop** | ✅ Full | Array mutation |
| **shift/unshift** | ✅ Full | Array mutation |
| **indexOf/includes** | ✅ Full | Search methods |
| **slice** | ✅ Full | Array slicing |
| **splice** | ✅ Full | Array modification |
| **map/filter/reduce** | ✅ Full | Functional methods |
| **forEach** | ✅ Full | Iteration method |
| **Mixed-Type Arrays** | ✅ Full | (string\|number)[] via NaN-boxing |
| **Destructuring** | ✅ Full | let [a, b] = arr |
| **Spread Operator** | ✅ Full | [...arr, x] syntax |

### Strings - Full Support

| Feature | Status | Notes |
|---------|--------|-------|
| **String Literals** | ✅ Full | "string" and 'string' |
| **Template Literals** | ✅ Full | \`Hello ${name}\` |
| **Concatenation** | ✅ Full | str1 + str2 |
| **length** | ✅ Full | str.length |
| **indexOf** | ✅ Full | Find substring |
| **slice** | ✅ Full | Extract substring |
| **substring** | ✅ Full | Extract substring |
| **split** | ✅ Full | Split by delimiter |
| **trim** | ✅ Full | Remove whitespace |
| **toLowerCase** | ✅ Full | Convert case |
| **toUpperCase** | ✅ Full | Convert case |
| **replace** | ✅ Full | String/regex replacement |

### Standard Library - Full Support

| Module | Status | Functions |
|--------|--------|-----------|
| **console** | ✅ Full | log() for numbers, strings, booleans |
| **fs** | ✅ Full | readFileSync, writeFileSync, existsSync, mkdirSync, unlinkSync |
| **path** | ✅ Full | join, dirname, basename, extname, resolve |
| **process** | ✅ Full | process.env.VARNAME |
| **JSON** | ✅ Full | parse, stringify |
| **Math** | ✅ Full | floor, ceil, round, abs, sqrt, pow, min, max, random |
| **Date** | ✅ Full | Date.now(), new Date(), getTime(), toISOString(), component getters |
| **crypto** | ✅ Full | randomBytes, randomUUID, sha256, md5 |
| **Map** | ✅ Full | get, set, has, delete, size, clear |
| **Set** | ✅ Full | add, has, delete, size, clear |
| **os** | ✅ Full | platform, arch, hostname, homedir, tmpdir, totalmem, freemem, uptime, type, release |
| **Buffer** | ✅ Full | from, alloc, allocUnsafe, byteLength, isBuffer, concat; instance: length, toString, slice, equals, copy, write |
| **child_process** | ✅ Full | execSync, spawnSync |
| **net** | ⚠️ Partial | createServer, createConnection (sync operations only) |

### Modules - Full Support

| Feature | Status | Notes |
|---------|--------|-------|
| **import/export** | ✅ Full | ES modules |
| **Named Imports** | ✅ Full | import { x } from 'mod' |
| **Default Imports** | ✅ Full | import x from 'mod' |
| **Re-exports** | ✅ Full | export { x } from 'mod' |
| **require()** | ✅ Full | CommonJS for built-in modules |

### Node.js Core Modules

Perry includes native Rust implementations of essential Node.js core modules. These compile directly to native code with no Node.js runtime required.

#### OS Module

```typescript
import * as os from 'os';

console.log(os.platform());   // "darwin", "linux", or "win32"
console.log(os.arch());       // "x64", "arm64", etc.
console.log(os.hostname());   // Machine hostname
console.log(os.homedir());    // User home directory
console.log(os.tmpdir());     // Temp directory path
console.log(os.totalmem());   // Total memory in bytes
console.log(os.freemem());    // Free memory in bytes
console.log(os.uptime());     // System uptime in seconds
console.log(os.type());       // OS name (e.g., "Darwin", "Linux")
console.log(os.release());    // OS release version
```

#### Buffer Module

```typescript
// Static methods
const buf1 = Buffer.from("hello");           // Create from string
const buf2 = Buffer.alloc(10, 0);            // Create zeroed buffer
const buf3 = Buffer.allocUnsafe(10);         // Create uninitialized buffer
const len = Buffer.byteLength("hello");      // Get byte length (5)
const isBuffer = Buffer.isBuffer(buf1);      // Type check (true)
const combined = Buffer.concat([buf1, buf2]); // Concatenate buffers

// Instance properties and methods
console.log(buf1.length);                    // 5
console.log(buf1.toString());                // "hello"
console.log(buf1.equals(Buffer.from("hello"))); // 1 (true)
const slice = buf1.slice(0, 3);              // Buffer containing "hel"
buf1.copy(buf2);                             // Copy buf1 into buf2
buf2.write("world", 0);                      // Write string to buffer
```

#### Child Process Module

```typescript
import { execSync, spawnSync } from 'child_process';

// Execute shell command synchronously
const output = execSync('echo hello');
console.log(output.toString());  // "hello\n"

// Spawn process with arguments
const result = spawnSync('ls', ['-la']);
console.log(result.stdout.toString());
console.log(result.status);  // Exit code
```

#### Net Module (Partial)

```typescript
import * as net from 'net';

// Create a TCP server
const server = net.createServer();
server.listen(3000, '127.0.0.1', () => {
  const addr = server.address();
  console.log(`Server listening on port ${addr.port}`);
});

// Create a TCP client connection
const client = net.createConnection(3000, '127.0.0.1');
client.destroy();
server.close();
```

> **Note:** The net module currently supports synchronous operations. Full async event-based networking requires additional runtime support.

### Other Features

| Feature | Status | Notes |
|---------|--------|-------|
| **setTimeout** | ✅ Full | Async timer support |
| **setInterval/clearInterval** | ✅ Full | Periodic timer support |
| **Async/Await** | ✅ Full | Async function support |
| **Promise** | ✅ Full | `new Promise((resolve, reject) => {...})` |
| **Spread in Calls** | ✅ Full | `fn(...args)` spreads array as arguments |
| **Dynamic new** | ✅ Full | `new Constructor()` with dynamic callees |
| **RegExp** | ⚠️ Partial | string.replace() works; regex.test() not yet |
| **Decorators** | ⚠️ Partial | @log method decorator (compile-time) |

---

## Known Limitations

### Not Yet Supported

| Feature | Priority | Notes |
|---------|----------|-------|
| **regex.test()** | Medium | Use string.replace() as workaround |
| **Method Chaining** | Low | Use intermediate variables |
| **Object Destructuring** | Low | Array destructuring works |
| **Dynamic Imports** | Low | Use static imports |
| **eval() / new Function()** | Never | Security/AOT incompatible |
| **Reflection** | Never | AOT incompatible |

### Runtime Characteristics

- **No Garbage Collection** - Memory is not automatically freed (use for short-running programs or manage manually)
- **No Runtime Type Checking** - Types are erased at compile time
- **Single-Threaded** - No multi-threading support yet

---

## Running Tests

```bash
# Run the full test suite (59 tests)
./run_tests.sh

# Test a single file
cargo run -- test-files/test_example.ts -o /tmp/test && /tmp/test

# Run with verbose output
cargo run -- test-files/test_example.ts -o /tmp/test
```

### Test Categories

| Category | Tests | Description |
|----------|-------|-------------|
| Core Language | 15+ | Variables, operators, control flow |
| Classes | 8+ | Inheritance, private fields, static, getters/setters |
| Functions | 6+ | Closures, rest params, defaults |
| Arrays | 5+ | Methods, slice, splice, mixed types |
| Strings | 5+ | Methods, split, replace |
| Types | 5+ | Generics, unions, type guards |
| Standard Library | 10+ | fs, path, JSON, Math, Date, crypto, Map, Set |
| Integration | 3 | Multi-feature tests |

---

## Architecture

```
TypeScript (.ts)                    JavaScript (.js)
      │                                   │
      ▼                                   │ (--enable-js-runtime)
┌─────────────┐                           │
│   Parser    │  SWC TypeScript Parser    │
│   (SWC)     │                           │
└─────────────┘                           │
      │                                   │
      ▼                                   │
┌─────────────┐                           │
│   Lowering  │  AST → HIR (High-level IR)│
│   (HIR)     │                           │
└─────────────┘                           │
      │                                   │
      ▼                                   │
┌─────────────┐                           │
│  Transform  │  Closure conversion,      │
│             │  monomorphization         │
└─────────────┘                           │
      │                                   │
      ▼                                   │
┌─────────────┐                           │
│   Codegen   │  Cranelift code generation│
│ (Cranelift) │                           │
└─────────────┘                           │
      │                                   │
      ▼                                   ▼
┌─────────────┐                    ┌─────────────┐
│   Linking   │  System linker ────│  V8 Engine  │ (optional)
└─────────────┘       (cc)         └─────────────┘
      │                                   │
      └───────────────┬───────────────────┘
                      ▼
                 Executable
```

### Key Design Decisions

1. **NaN-Boxing** - Values are stored as 64-bit floats with special bit patterns for pointers, enabling union types without runtime overhead

2. **Monomorphization** - Generics are specialized at compile time (like Rust), generating optimized code for each type instantiation

3. **Static Dispatch** - No virtual tables; method calls are resolved at compile time

4. **No GC** - Memory management is explicit; suitable for short-running programs or careful resource management

---

## Project Structure

```
perry/
├── crates/
│   ├── perry/              # CLI driver
│   ├── perry-parser/       # SWC wrapper
│   ├── perry-types/        # Type system
│   ├── perry-hir/          # High-level IR
│   │   ├── ir.rs              # IR definitions
│   │   ├── lower.rs           # AST → HIR
│   │   └── monomorph.rs       # Generics specialization
│   ├── perry-transform/    # IR transformations
│   ├── perry-codegen/      # Cranelift codegen
│   │   └── codegen.rs         # Main code generator
│   ├── perry-runtime/      # Runtime library
│   │   ├── string.rs          # String operations
│   │   ├── array.rs           # Array operations
│   │   ├── value.rs           # NaN-boxing
│   │   ├── os.rs              # OS module (platform, memory, etc.)
│   │   ├── buffer.rs          # Buffer module
│   │   ├── child_process.rs   # Child process module
│   │   ├── net.rs             # TCP networking
│   │   ├── timer.rs           # setTimeout/setInterval
│   │   ├── promise.rs         # Promise support
│   │   └── ...
│   ├── perry-jsruntime/    # V8 JavaScript runtime (optional)
│   │   ├── lib.rs             # Runtime initialization
│   │   ├── bridge.rs          # Native ↔ V8 value conversion
│   │   ├── interop.rs         # FFI functions
│   │   └── modules.rs         # Module loader
│   ├── perry-stdlib/       # Standard library
│   └── perry-diagnostics/  # Error reporting
├── test-files/                 # 59 test files
├── example-code/              # Real-world example applications
│   ├── http-server/              # Basic HTTP server
│   ├── express-postgres/         # Express + PostgreSQL
│   ├── koa-redis/                # Koa + Redis
│   ├── fastify-redis-mysql/      # Fastify + Redis + MySQL
│   ├── blockchain-demo/          # Blockchain simulation
│   └── hono-mongodb/             # Hono + MongoDB + JWT
├── run_tests.sh               # Test runner script
├── FEATURE_AUDIT.md           # Detailed feature status
└── README.md                  # This file
```

---

## Development

```bash
# Build all crates
cargo build

# Run tests
cargo test

# Build release
cargo build --release

# Check code
cargo check

# Format code
cargo fmt

# Lint
cargo clippy

# Run the full TypeScript test suite
./run_tests.sh
```

### Adding New Features

1. **HIR Definition** - Add expression/statement type to `crates/perry-hir/src/ir.rs`
2. **Lowering** - Handle AST → HIR conversion in `crates/perry-hir/src/lower.rs`
3. **Monomorphization** - Update if generics-related in `crates/perry-hir/src/monomorph.rs`
4. **Code Generation** - Generate Cranelift IR in `crates/perry-codegen/src/codegen.rs`
5. **Runtime** - Add runtime functions if needed in `crates/perry-runtime/`
6. **Tests** - Create test file in `test-files/test_feature.ts`

---

## Native Libraries

Perry includes native Rust implementations of **27 popular npm packages**. When you import these packages, they compile directly to native code - no Node.js required.

### Supported Packages

| Category | Packages |
|----------|----------|
| **Database** | mysql2, pg, mongodb, better-sqlite3, ioredis |
| **Security** | bcrypt, argon2, jsonwebtoken, crypto |
| **HTTP** | axios, node-fetch, ws, nodemailer |
| **Data** | cheerio, sharp, zlib, lodash |
| **Date/Time** | dayjs, moment, date-fns, node-cron |
| **Utilities** | uuid, nanoid, slugify, validator, dotenv, rate-limiter-flexible |

See [docs/native-libraries.md](docs/native-libraries.md) for full API documentation.

---

## JavaScript Runtime (V8)

By default, Perry produces **fully native executables** with no JavaScript runtime. However, some npm packages are pure JavaScript and cannot be natively compiled. For these cases, Perry can optionally embed the V8 JavaScript engine.

### Compilation Modes

#### Native Only (Default)

```bash
# Standard compilation - no V8, smallest binary
perry build main.ts -o myapp
```

- **Binary size:** ~2-5 MB
- **Startup time:** Instant
- **Supported imports:** Native Rust implementations, local TypeScript files
- **Best for:** Performance-critical applications, simple programs

#### With V8 Runtime

```bash
# Enable V8 for JavaScript module support
perry build main.ts -o myapp --enable-js-runtime
```

- **Binary size:** ~15-20 MB (includes V8)
- **Startup time:** Slightly slower (V8 initialization)
- **Supported imports:** All of the above + pure JavaScript npm packages
- **Best for:** Applications using npm packages without native implementations

### How It Works

When `--enable-js-runtime` is enabled:

1. **TypeScript files** are still compiled to native code
2. **JavaScript files** (`.js`, `.mjs`) are loaded and executed by V8 at runtime
3. **Cross-boundary calls** use NaN-boxing for seamless type conversion

```typescript
// main.ts - This is compiled to native code
import { someFunction } from './my-module.js';  // Runs in V8

const result = someFunction("hello", 42);  // Native ↔ V8 interop
console.log(result);  // Native code
```

### Module Resolution Priority

```
import X from "module"
         │
         ▼
1. Native Rust implementation (mysql2, pg, axios, etc.) → Native code
2. Local TypeScript file (.ts, .tsx)                    → Native code
3. Local JavaScript file (.js, .mjs)                    → V8 runtime
4. node_modules JavaScript package                      → V8 runtime
```

### Limitations

The V8 runtime has some restrictions:

| Feature | Status |
|---------|--------|
| Pure JS packages | ✅ Supported |
| ESM modules | ✅ Supported |
| CommonJS modules | ✅ Supported (auto-wrapped) |
| Native addons (C/C++) | ❌ Not supported |
| Dynamic require() | ❌ Not supported |
| Node.js built-in APIs | ⚠️ Limited (only what Perry implements) |

### Example

```typescript
// test_js_import.ts
import { greet, add, PI } from './my-module.js';

console.log(greet("World"));  // "Hello, World!"
console.log(add(10, 20));     // 30
console.log(PI);              // 3.14159
```

```javascript
// my-module.js (pure JavaScript)
export function greet(name) {
    return `Hello, ${name}!`;
}

export function add(a, b) {
    return a + b;
}

export const PI = 3.14159;
```

```bash
# Compile with JS runtime support
perry build test_js_import.ts --enable-js-runtime

# Run
./test_js_import
# Output:
# Hello, World!
# 30
# 3.14159
```

---

## Commands

### `perry build`

Compiles TypeScript source code to a native executable.

```bash
perry build <input.ts> [options]

Options:
  -o, --output <name>      Output executable name
  --enable-js-runtime      Embed V8 for JavaScript module support
  --print-hir              Print HIR for debugging
  --no-link                Produce object file only (no linking)
  --keep-intermediates     Keep intermediate .o files
```

### `perry check`

Validates TypeScript code for compatibility with native compilation.

```bash
perry check <path> [options]

Options:
  --check-deps       Check dependencies in node_modules
  --fix              Automatically fix issues where possible
  --fix-dry-run      Show what fixes would be applied
```

### `perry doctor`

Diagnose your development environment and check for required tools.

```bash
perry doctor
```

---

## Recent Improvements

The following features and fixes were recently added to improve real-world application support:

### Compiler Features
- **Spread operator in function calls** - `fn(...args)` now properly unpacks arrays as function arguments
- **Promise constructor** - `new Promise((resolve, reject) => {...})` fully supported
- **Dynamic new expressions** - `new SomeVar()` works with dynamic/imported constructors
- **Cross-module class resolution** - Classes can be imported and used across module boundaries
- **Exported function symbols** - Functions exported from modules generate proper linker symbols

### Runtime Functions
- **setInterval/clearInterval** - Periodic timer support for recurring callbacks
- **Timer integration** - Intervals properly tick during async/await execution

### Multi-Module Compilation
- **Package subpath exports** - Resolves `import { x } from 'pkg/subpath'` using package.json exports
- **Duplicate file handling** - Files with same basename in different directories get unique object file names
- **Module init deduplication** - Each module gets a unique init function name based on full path

### Type System Fixes
- **i64/f64 type conversion** - Comprehensive fixes for Cranelift type mismatches
- **JWT module support** - Proper type handling for jsonwebtoken sign/verify/decode
- **Object field access** - Fixed pointer types in property get/set operations

---

## Roadmap

### Completed

- [x] Core language features (variables, operators, control flow)
- [x] Functions and closures with mutable captures
- [x] Classes with inheritance, private fields, getters/setters
- [x] Generics with monomorphization
- [x] Standard library (fs, path, JSON, Math, Date, crypto)
- [x] Union types and type guards
- [x] Array methods (map, filter, reduce, etc.)
- [x] String methods and regex replace
- [x] Exception handling (try-catch-finally)
- [x] Decorators (basic @log)
- [x] ES modules and CommonJS require()
- [x] Optional V8 runtime for JavaScript npm packages (`--enable-js-runtime`)
- [x] OS module (platform, arch, hostname, memory info, etc.)
- [x] Buffer module (from, alloc, toString, slice, equals, copy, write)
- [x] Child process module (execSync, spawnSync)
- [x] Promise constructor with resolve/reject callbacks
- [x] setInterval/clearInterval timers
- [x] Spread operator in function calls
- [x] Dynamic new expressions with imported constructors
- [x] Cross-module class and function exports

### In Progress / Future

- [ ] Full regex support (regex.test(), match())
- [ ] Object destructuring
- [ ] Net module async operations (event-based TCP)
- [ ] Stream module
- [ ] More decorator types
- [ ] Improved error messages
- [ ] Source maps for debugging
- [ ] WASM target
- [ ] Multi-threading support

---

## Contributing

Contributions are welcome! Please read the [CLAUDE.md](CLAUDE.md) file for development guidelines and the [FEATURE_AUDIT.md](FEATURE_AUDIT.md) for detailed feature status.

## License

[License information here]
