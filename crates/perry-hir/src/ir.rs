//! HIR (High-level Intermediate Representation) definitions
//!
//! The HIR is a typed, lowered representation of TypeScript that is
//! easier to compile to native code than the raw AST.

use perry_types::{FuncId, GlobalId, LocalId, Type, TypeParam};

/// Known native module names that map to stdlib implementations.
/// These are npm packages that have native Rust replacements.
pub const NATIVE_MODULES: &[&str] = &[
    "mysql2",
    "mysql2/promise",
    "pg",
    "uuid",
    "bcrypt",
    // Note: ioredis removed - native impl uses functions, not class instantiation
    // Users can use the native redis functions via direct FFI or use JS runtime
    "node-fetch",
    "ws",
    "zlib",
    "crypto",
    // Tier 3
    "dotenv",
    "dotenv/config",  // Side-effect import that auto-calls dotenv.config()
    "jsonwebtoken",
    "nanoid",
    "slugify",
    "validator",
    "ethers",
    // Node.js built-ins
    "events",
    "os",
    "buffer",
    "child_process",
    "net",
    "stream",
    // Utility libraries
    "lru-cache",
    "commander",
    "big.js",
    "decimal.js",
    "bignumber.js",
];

/// Check if a module path refers to a native stdlib module
pub fn is_native_module(path: &str) -> bool {
    NATIVE_MODULES.contains(&path)
}

/// The kind of module being imported, determining how it's executed
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleKind {
    /// Native TypeScript compiled to machine code (default for .ts/.tsx files)
    NativeCompiled,
    /// Native Rust stdlib implementation (mysql2, pg, etc.)
    NativeRust,
    /// V8-interpreted JavaScript (fallback for .js modules)
    /// This requires explicit opt-in and user confirmation
    Interpreted,
}

impl Default for ModuleKind {
    fn default() -> Self {
        ModuleKind::NativeCompiled
    }
}

/// Determine the module kind for a given import path
pub fn determine_module_kind(source: &str, resolved_path: Option<&std::path::Path>) -> ModuleKind {
    // First check if it's a native Rust stdlib module
    if is_native_module(source) {
        return ModuleKind::NativeRust;
    }

    // Check the resolved path extension
    if let Some(path) = resolved_path {
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            match ext {
                "ts" | "tsx" => return ModuleKind::NativeCompiled,
                "js" | "mjs" | "cjs" => return ModuleKind::Interpreted,
                _ => {}
            }
        }
    }

    // Default to native compiled (assume TypeScript)
    ModuleKind::NativeCompiled
}

/// Unique identifier for a class
pub type ClassId = u32;

/// Unique identifier for an enum
pub type EnumId = u32;

/// Unique identifier for an interface
pub type InterfaceId = u32;

/// Unique identifier for a type alias
pub type TypeAliasId = u32;

/// A complete HIR module (corresponds to one TypeScript file)
#[derive(Debug, Clone)]
pub struct Module {
    /// Module name/path
    pub name: String,
    /// Imports from other modules
    pub imports: Vec<Import>,
    /// Exports from this module
    pub exports: Vec<Export>,
    /// Class definitions
    pub classes: Vec<Class>,
    /// Interface definitions
    pub interfaces: Vec<Interface>,
    /// Type alias definitions
    pub type_aliases: Vec<TypeAlias>,
    /// Enum definitions
    pub enums: Vec<Enum>,
    /// Global variable declarations
    pub globals: Vec<Global>,
    /// Function definitions
    pub functions: Vec<Function>,
    /// Top-level statements to execute
    pub init: Vec<Stmt>,
    /// Exported native module instances: (export_name, module_name, class_name)
    /// This tracks variables like `export const pool = new Pool(...)` from pg
    pub exported_native_instances: Vec<(String, String, String)>,
    /// Exported object literals: export_name
    /// This tracks variables like `export const config = { ... }`
    pub exported_objects: Vec<String>,
    /// Exported functions that need globals for cross-module value passing
    /// This tracks functions like `export function foo() { ... }` or `export async function bar() { ... }`
    /// that may be imported and used as values (not just called) by other modules
    pub exported_functions: Vec<(String, FuncId)>,
}

/// An enum definition
#[derive(Debug, Clone)]
pub struct Enum {
    pub id: EnumId,
    pub name: String,
    pub members: Vec<EnumMember>,
    pub is_exported: bool,
}

/// An enum member
#[derive(Debug, Clone)]
pub struct EnumMember {
    pub name: String,
    pub value: EnumValue,
}

/// Value of an enum member
#[derive(Debug, Clone)]
pub enum EnumValue {
    /// Numeric value (auto-incremented or explicit)
    Number(i64),
    /// String value
    String(String),
}

/// An interface definition
#[derive(Debug, Clone)]
pub struct Interface {
    pub id: InterfaceId,
    pub name: String,
    /// Generic type parameters (e.g., T, K in interface<T, K>)
    pub type_params: Vec<TypeParam>,
    /// Extended interfaces
    pub extends: Vec<Type>,
    /// Property signatures
    pub properties: Vec<InterfaceProperty>,
    /// Method signatures
    pub methods: Vec<InterfaceMethod>,
    pub is_exported: bool,
}

/// A property in an interface
#[derive(Debug, Clone)]
pub struct InterfaceProperty {
    pub name: String,
    pub ty: Type,
    pub optional: bool,
    pub readonly: bool,
}

/// A method signature in an interface
#[derive(Debug, Clone)]
pub struct InterfaceMethod {
    pub name: String,
    /// Method's own type parameters (separate from interface's)
    pub type_params: Vec<TypeParam>,
    pub params: Vec<(String, Type, bool)>, // name, type, optional
    pub return_type: Type,
}

/// A type alias definition
#[derive(Debug, Clone)]
pub struct TypeAlias {
    pub id: TypeAliasId,
    pub name: String,
    /// Generic type parameters
    pub type_params: Vec<TypeParam>,
    /// The aliased type
    pub ty: Type,
    pub is_exported: bool,
}

/// An import declaration
#[derive(Debug, Clone)]
pub struct Import {
    /// Source module path (e.g., "./utils" or "fs")
    pub source: String,
    /// Import specifiers
    pub specifiers: Vec<ImportSpecifier>,
    /// True if this imports from a native stdlib module (mysql2, pg, etc.)
    pub is_native: bool,
    /// The kind of module (native compiled, native Rust, or V8 interpreted)
    pub module_kind: ModuleKind,
    /// Resolved absolute path to the module file (if available)
    pub resolved_path: Option<String>,
}

/// Import specifier
#[derive(Debug, Clone)]
pub enum ImportSpecifier {
    /// Named import: import { foo, bar as baz } from "..."
    Named {
        imported: String,
        local: String,
    },
    /// Default import: import foo from "..."
    Default {
        local: String,
    },
    /// Namespace import: import * as foo from "..."
    Namespace {
        local: String,
    },
}

/// An export declaration
#[derive(Debug, Clone)]
pub enum Export {
    /// Named export: export { foo, bar as baz }
    Named {
        local: String,
        exported: String,
    },
    /// Re-export: export { foo } from "..."
    ReExport {
        source: String,
        imported: String,
        exported: String,
    },
    /// Export all: export * from "..."
    ExportAll {
        source: String,
    },
}

/// A class definition
#[derive(Debug, Clone)]
pub struct Class {
    pub id: ClassId,
    pub name: String,
    /// Generic type parameters (e.g., T, K, V in class<T, K, V>)
    pub type_params: Vec<TypeParam>,
    /// Parent class (for inheritance)
    pub extends: Option<ClassId>,
    /// Parent class name (for inheritance from imported classes where ClassId may not be known)
    pub extends_name: Option<String>,
    /// Native parent class (module_name, class_name) - e.g., ("events", "EventEmitter")
    pub native_extends: Option<(String, String)>,
    /// Instance fields
    pub fields: Vec<ClassField>,
    /// Constructor (if any)
    pub constructor: Option<Function>,
    /// Instance methods
    pub methods: Vec<Function>,
    /// Getter methods (property_name -> function that returns the value)
    pub getters: Vec<(String, Function)>,
    /// Setter methods (property_name -> function that takes the value)
    pub setters: Vec<(String, Function)>,
    /// Static fields
    pub static_fields: Vec<ClassField>,
    /// Static methods
    pub static_methods: Vec<Function>,
    /// Whether this class is exported from the module
    pub is_exported: bool,
}

/// A class field
#[derive(Debug, Clone)]
pub struct ClassField {
    pub name: String,
    pub ty: Type,
    pub init: Option<Expr>,
    pub is_private: bool,
    pub is_readonly: bool,
}

/// A global variable
#[derive(Debug, Clone)]
pub struct Global {
    pub id: GlobalId,
    pub name: String,
    pub ty: Type,
    pub mutable: bool,
    pub init: Option<Expr>,
}

/// A decorator applied to a method or class
#[derive(Debug, Clone)]
pub struct Decorator {
    /// The decorator function name (e.g., "log" for @log)
    pub name: String,
    /// Arguments if this is a decorator factory call (e.g., @log("prefix") -> args = ["prefix"])
    pub args: Vec<Expr>,
}

/// A function definition
#[derive(Debug, Clone)]
pub struct Function {
    pub id: FuncId,
    pub name: String,
    /// Generic type parameters (e.g., T, K in function<T, K>)
    pub type_params: Vec<TypeParam>,
    pub params: Vec<Param>,
    pub return_type: Type,
    pub body: Vec<Stmt>,
    pub is_async: bool,
    pub is_exported: bool,
    /// Captured variables (for closures)
    pub captures: Vec<LocalId>,
    /// Decorators applied to this function/method
    pub decorators: Vec<Decorator>,
}

/// A function parameter
#[derive(Debug, Clone)]
pub struct Param {
    pub id: LocalId,
    pub name: String,
    pub ty: Type,
    pub default: Option<Expr>,
    /// True if this is a rest parameter (...args)
    pub is_rest: bool,
}

/// Statement in function body
#[derive(Debug, Clone)]
pub enum Stmt {
    /// Local variable declaration: let/const x = expr
    Let {
        id: LocalId,
        name: String,
        ty: Type,
        mutable: bool,
        init: Option<Expr>,
    },
    /// Expression statement
    Expr(Expr),
    /// Return statement
    Return(Option<Expr>),
    /// If statement
    If {
        condition: Expr,
        then_branch: Vec<Stmt>,
        else_branch: Option<Vec<Stmt>>,
    },
    /// While loop
    While {
        condition: Expr,
        body: Vec<Stmt>,
    },
    /// For loop (lowered from various JS for loops)
    For {
        init: Option<Box<Stmt>>,
        condition: Option<Expr>,
        update: Option<Expr>,
        body: Vec<Stmt>,
    },
    /// Break statement
    Break,
    /// Continue statement
    Continue,
    /// Throw statement
    Throw(Expr),
    /// Try-catch-finally
    Try {
        body: Vec<Stmt>,
        catch: Option<CatchClause>,
        finally: Option<Vec<Stmt>>,
    },
    /// Switch statement
    Switch {
        discriminant: Expr,
        cases: Vec<SwitchCase>,
    },
}

/// A case in a switch statement
#[derive(Debug, Clone)]
pub struct SwitchCase {
    /// Test expression (None for default case)
    pub test: Option<Expr>,
    /// Statements in this case (including fallthrough)
    pub body: Vec<Stmt>,
}

/// Catch clause in try statement
#[derive(Debug, Clone)]
pub struct CatchClause {
    pub param: Option<(LocalId, String)>,
    pub body: Vec<Stmt>,
}

/// Expression
#[derive(Debug, Clone)]
pub enum Expr {
    // Literals
    Undefined,
    Null,
    Bool(bool),
    Number(f64),
    Integer(i64), // Integer literal that fits in i64 (for optimization)
    BigInt(String), // Store as string to preserve precision
    String(String),

    // Variables
    LocalGet(LocalId),
    LocalSet(LocalId, Box<Expr>),
    GlobalGet(GlobalId),
    GlobalSet(GlobalId, Box<Expr>),

    // Update (++/--)
    Update {
        id: LocalId,
        op: UpdateOp,
        prefix: bool, // true for ++x, false for x++
    },

    // Operations
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Unary {
        op: UnaryOp,
        operand: Box<Expr>,
    },

    // Comparison
    Compare {
        op: CompareOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },

    // Logical
    Logical {
        op: LogicalOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },

    // Function call
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
        /// Explicit type arguments (e.g., identity<number>(x))
        type_args: Vec<Type>,
    },

    /// Function call with spread arguments (e.g., fn(a, ...arr, b))
    CallSpread {
        callee: Box<Expr>,
        args: Vec<CallArg>,
        type_args: Vec<Type>,
    },

    // Named function reference
    FuncRef(FuncId),

    // External function reference (imported from another module)
    // Includes type information for proper code generation
    ExternFuncRef {
        name: String,
        param_types: Vec<Type>,
        return_type: Type,
    },

    // Native module reference (e.g., mysql2, pg)
    // The string is the module name, the local name is tracked separately
    NativeModuleRef(String),

    // Native module method call (e.g., mysql.createConnection, connection.query)
    // module: the native module name (e.g., "mysql2")
    // class_name: optional class name for distinguishing object types (e.g., "Pool" vs "Connection")
    // object: optional object to call method on (None for static methods like createConnection)
    // method: the method name
    // args: call arguments
    NativeMethodCall {
        module: String,
        class_name: Option<String>,
        object: Option<Box<Expr>>,
        method: String,
        args: Vec<Expr>,
    },

    // Object/property access
    PropertyGet {
        object: Box<Expr>,
        property: String,
    },
    PropertySet {
        object: Box<Expr>,
        property: String,
        value: Box<Expr>,
    },
    // Property update (++/--)
    PropertyUpdate {
        object: Box<Expr>,
        property: String,
        op: BinaryOp,      // Add for ++, Sub for --
        prefix: bool,      // true for ++x, false for x++
    },

    // Array/index access
    IndexGet {
        object: Box<Expr>,
        index: Box<Expr>,
    },
    IndexSet {
        object: Box<Expr>,
        index: Box<Expr>,
        value: Box<Expr>,
    },

    // Object literal
    Object(Vec<(String, Expr)>),

    // Array literal
    Array(Vec<Expr>),

    // Array literal with spread elements
    // Each element is either a regular expression (Left) or a spread expression (Right)
    ArraySpread(Vec<ArrayElement>),

    // Conditional expression (ternary)
    Conditional {
        condition: Box<Expr>,
        then_expr: Box<Expr>,
        else_expr: Box<Expr>,
    },

    // Type operations
    TypeOf(Box<Expr>),
    InstanceOf {
        expr: Box<Expr>,
        ty: String,
    },
    /// The 'in' operator: checks if property exists in object
    /// e.g., "prop" in obj or key in obj
    In {
        property: Box<Expr>,
        object: Box<Expr>,
    },

    // Await expression (for async functions)
    Await(Box<Expr>),

    // New expression (class instantiation)
    New {
        class_name: String,
        args: Vec<Expr>,
        /// Explicit type arguments (e.g., new Box<number>(42))
        type_args: Vec<Type>,
    },

    /// Dynamic new expression (new with non-identifier callee)
    /// e.g., new (condition ? ClassA : ClassB)()
    /// or new someVariable()
    NewDynamic {
        /// The expression that evaluates to a constructor
        callee: Box<Expr>,
        /// Arguments to pass to the constructor
        args: Vec<Expr>,
    },

    // Class reference (for new expressions)
    ClassRef(String),

    // Enum member access (e.g., Color.Red)
    EnumMember {
        enum_name: String,
        member_name: String,
    },

    // Static field access (e.g., Counter.count)
    StaticFieldGet {
        class_name: String,
        field_name: String,
    },

    // Static field assignment (e.g., Counter.count = 5)
    StaticFieldSet {
        class_name: String,
        field_name: String,
        value: Box<Expr>,
    },

    // Static method call (e.g., Counter.increment())
    StaticMethodCall {
        class_name: String,
        method_name: String,
        args: Vec<Expr>,
    },

    // This expression
    This,

    // Super constructor call: super(args)
    SuperCall(Vec<Expr>),

    // Super method call: super.method(args)
    SuperMethodCall {
        method: String,
        args: Vec<Expr>,
    },

    // Environment variable access: process.env.VARNAME
    EnvGet(String),
    // Process uptime: process.uptime() -> number (seconds)
    ProcessUptime,
    // Process current working directory: process.cwd() -> string
    ProcessCwd,

    // File system operations
    FsReadFileSync(Box<Expr>),           // fs.readFileSync(path) -> string
    FsWriteFileSync(Box<Expr>, Box<Expr>), // fs.writeFileSync(path, content) -> void
    FsExistsSync(Box<Expr>),             // fs.existsSync(path) -> boolean
    FsMkdirSync(Box<Expr>),              // fs.mkdirSync(path) -> void
    FsUnlinkSync(Box<Expr>),             // fs.unlinkSync(path) -> void

    // Path operations
    PathJoin(Box<Expr>, Box<Expr>),      // path.join(a, b) -> string
    PathDirname(Box<Expr>),              // path.dirname(path) -> string
    PathBasename(Box<Expr>),             // path.basename(path) -> string
    PathExtname(Box<Expr>),              // path.extname(path) -> string
    PathResolve(Box<Expr>),              // path.resolve(path) -> string

    // JSON operations
    JsonParse(Box<Expr>),                // JSON.parse(string) -> value
    JsonStringify(Box<Expr>),            // JSON.stringify(value) -> string

    // Math operations
    MathFloor(Box<Expr>),                // Math.floor(x) -> number
    MathCeil(Box<Expr>),                 // Math.ceil(x) -> number
    MathRound(Box<Expr>),                // Math.round(x) -> number
    MathAbs(Box<Expr>),                  // Math.abs(x) -> number
    MathSqrt(Box<Expr>),                 // Math.sqrt(x) -> number
    MathPow(Box<Expr>, Box<Expr>),       // Math.pow(base, exp) -> number
    MathMin(Vec<Expr>),                  // Math.min(...values) -> number
    MathMax(Vec<Expr>),                  // Math.max(...values) -> number
    MathRandom,                          // Math.random() -> number

    // Crypto operations
    CryptoRandomBytes(Box<Expr>),        // crypto.randomBytes(size) -> string (hex)
    CryptoRandomUUID,                    // crypto.randomUUID() -> string
    CryptoSha256(Box<Expr>),             // crypto.sha256(data) -> string (hex)
    CryptoMd5(Box<Expr>),                // crypto.md5(data) -> string (hex)

    // OS operations
    OsPlatform,                          // os.platform() -> string ("darwin", "linux", "win32")
    OsArch,                              // os.arch() -> string ("x64", "arm64", etc.)
    OsHostname,                          // os.hostname() -> string
    OsHomedir,                           // os.homedir() -> string
    OsTmpdir,                            // os.tmpdir() -> string
    OsTotalmem,                          // os.totalmem() -> number (bytes)
    OsFreemem,                           // os.freemem() -> number (bytes)
    OsUptime,                            // os.uptime() -> number (seconds)
    OsType,                              // os.type() -> string ("Darwin", "Linux", "Windows_NT")
    OsRelease,                           // os.release() -> string
    OsCpus,                              // os.cpus() -> array of CPU info objects
    OsNetworkInterfaces,                 // os.networkInterfaces() -> object
    OsUserInfo,                          // os.userInfo() -> object
    OsEOL,                               // os.EOL -> string ("\n" or "\r\n")

    // Buffer operations
    BufferFrom {                         // Buffer.from(data, encoding?) -> Buffer
        data: Box<Expr>,
        encoding: Option<Box<Expr>>,
    },
    BufferAlloc {                        // Buffer.alloc(size, fill?) -> Buffer
        size: Box<Expr>,
        fill: Option<Box<Expr>>,
    },
    BufferAllocUnsafe(Box<Expr>),        // Buffer.allocUnsafe(size) -> Buffer
    BufferConcat(Box<Expr>),             // Buffer.concat(list) -> Buffer
    BufferIsBuffer(Box<Expr>),           // Buffer.isBuffer(obj) -> boolean
    BufferByteLength(Box<Expr>),         // Buffer.byteLength(string) -> number
    BufferToString {                     // buffer.toString(encoding?) -> string
        buffer: Box<Expr>,
        encoding: Option<Box<Expr>>,
    },
    BufferLength(Box<Expr>),             // buffer.length -> number
    BufferSlice {                        // buffer.slice(start?, end?) -> Buffer
        buffer: Box<Expr>,
        start: Option<Box<Expr>>,
        end: Option<Box<Expr>>,
    },
    BufferCopy {                         // buffer.copy(target, tStart?, sStart?, sEnd?) -> number
        source: Box<Expr>,
        target: Box<Expr>,
        target_start: Option<Box<Expr>>,
        source_start: Option<Box<Expr>>,
        source_end: Option<Box<Expr>>,
    },
    BufferWrite {                        // buffer.write(string, offset?, encoding?) -> number
        buffer: Box<Expr>,
        string: Box<Expr>,
        offset: Option<Box<Expr>>,
        encoding: Option<Box<Expr>>,
    },
    BufferEquals {                       // buffer.equals(other) -> boolean
        buffer: Box<Expr>,
        other: Box<Expr>,
    },
    BufferIndexGet {                     // buffer[i] -> number
        buffer: Box<Expr>,
        index: Box<Expr>,
    },
    BufferIndexSet {                     // buffer[i] = value
        buffer: Box<Expr>,
        index: Box<Expr>,
        value: Box<Expr>,
    },

    // Typed array operations
    Uint8ArrayNew(Option<Box<Expr>>),    // new Uint8Array() or new Uint8Array(length) or new Uint8Array(array)
    Uint8ArrayFrom(Box<Expr>),           // Uint8Array.from(arrayLike) -> Uint8Array
    Uint8ArrayLength(Box<Expr>),         // uint8array.length -> number
    Uint8ArrayGet {                      // uint8array[i] -> number
        array: Box<Expr>,
        index: Box<Expr>,
    },
    Uint8ArraySet {                      // uint8array[i] = value
        array: Box<Expr>,
        index: Box<Expr>,
        value: Box<Expr>,
    },

    // Child Process operations
    ChildProcessExecSync {               // execSync(cmd, opts?) -> Buffer | string
        command: Box<Expr>,
        options: Option<Box<Expr>>,
    },
    ChildProcessSpawnSync {              // spawnSync(cmd, args?, opts?) -> SpawnSyncResult
        command: Box<Expr>,
        args: Option<Box<Expr>>,
        options: Option<Box<Expr>>,
    },
    ChildProcessSpawn {                  // spawn(cmd, args?, opts?) -> ChildProcess
        command: Box<Expr>,
        args: Option<Box<Expr>>,
        options: Option<Box<Expr>>,
    },
    ChildProcessExec {                   // exec(cmd, opts?, callback?) -> ChildProcess
        command: Box<Expr>,
        options: Option<Box<Expr>>,
        callback: Option<Box<Expr>>,
    },

    // Net operations
    NetCreateServer {                    // net.createServer(options?, connectionListener?) -> Server
        options: Option<Box<Expr>>,
        connection_listener: Option<Box<Expr>>,
    },
    NetCreateConnection {                // net.createConnection(port, host?, connectListener?) -> Socket
        port: Box<Expr>,
        host: Option<Box<Expr>>,
        connect_listener: Option<Box<Expr>>,
    },
    NetConnect {                         // net.connect(port, host?, connectListener?) -> Socket
        port: Box<Expr>,
        host: Option<Box<Expr>>,
        connect_listener: Option<Box<Expr>>,
    },

    // Array methods
    ArrayPush { array_id: LocalId, value: Box<Expr> },    // arr.push(value) -> new length
    ArrayPop(LocalId),                                     // arr.pop() -> removed element
    ArrayShift(LocalId),                                   // arr.shift() -> removed element
    ArrayUnshift { array_id: LocalId, value: Box<Expr> }, // arr.unshift(value) -> new length
    ArrayIndexOf { array: Box<Expr>, value: Box<Expr> },  // arr.indexOf(value) -> index
    ArrayIncludes { array: Box<Expr>, value: Box<Expr> }, // arr.includes(value) -> boolean
    ArraySlice { array: Box<Expr>, start: Box<Expr>, end: Option<Box<Expr>> }, // arr.slice(start, end?) -> new array
    ArraySplice { array_id: LocalId, start: Box<Expr>, delete_count: Option<Box<Expr>>, items: Vec<Expr> }, // arr.splice(start, deleteCount?, ...items) -> deleted elements array

    // Array higher-order function methods
    ArrayForEach { array: Box<Expr>, callback: Box<Expr> },  // arr.forEach(fn) -> void
    ArrayMap { array: Box<Expr>, callback: Box<Expr> },      // arr.map(fn) -> new array
    ArrayFilter { array: Box<Expr>, callback: Box<Expr> },   // arr.filter(fn) -> new array
    ArrayFind { array: Box<Expr>, callback: Box<Expr> },     // arr.find(fn) -> element | undefined
    ArrayFindIndex { array: Box<Expr>, callback: Box<Expr> }, // arr.findIndex(fn) -> index | -1
    ArrayReduce { array: Box<Expr>, callback: Box<Expr>, initial: Option<Box<Expr>> }, // arr.reduce(fn, init?) -> value
    ArrayJoin { array: Box<Expr>, separator: Option<Box<Expr>> }, // arr.join(separator?) -> string

    // String methods
    StringSplit(Box<Expr>, Box<Expr>),  // string.split(delimiter) -> string[]

    // Map operations
    MapNew,                                                    // new Map() -> empty map
    MapSet { map_id: LocalId, key: Box<Expr>, value: Box<Expr> }, // map.set(key, value) -> map (updates local)
    MapGet { map: Box<Expr>, key: Box<Expr> },                 // map.get(key) -> value | undefined
    MapHas { map: Box<Expr>, key: Box<Expr> },                 // map.has(key) -> boolean
    MapDelete { map: Box<Expr>, key: Box<Expr> },              // map.delete(key) -> boolean
    MapSize(Box<Expr>),                                        // map.size -> number
    MapClear(Box<Expr>),                                       // map.clear() -> void

    // Set operations
    SetNew,                                                    // new Set() -> empty set
    SetAdd { set_id: LocalId, value: Box<Expr> },              // set.add(value) -> set (updates local)
    SetHas { set: Box<Expr>, value: Box<Expr> },               // set.has(value) -> boolean
    SetDelete { set: Box<Expr>, value: Box<Expr> },            // set.delete(value) -> boolean
    SetSize(Box<Expr>),                                        // set.size -> number
    SetClear(Box<Expr>),                                       // set.clear() -> void

    // Sequence expression (comma operator)
    Sequence(Vec<Expr>),

    // Date operations
    DateNow,                              // Date.now() -> number (timestamp in ms)
    DateNew(Option<Box<Expr>>),           // new Date() or new Date(timestamp) -> Date object
    DateGetTime(Box<Expr>),               // date.getTime() -> number
    DateToISOString(Box<Expr>),           // date.toISOString() -> string
    DateGetFullYear(Box<Expr>),           // date.getFullYear() -> number
    DateGetMonth(Box<Expr>),              // date.getMonth() -> number (0-11)
    DateGetDate(Box<Expr>),               // date.getDate() -> number (1-31)
    DateGetHours(Box<Expr>),              // date.getHours() -> number (0-23)
    DateGetMinutes(Box<Expr>),            // date.getMinutes() -> number (0-59)
    DateGetSeconds(Box<Expr>),            // date.getSeconds() -> number (0-59)
    DateGetMilliseconds(Box<Expr>),       // date.getMilliseconds() -> number (0-999)

    // Error operations
    ErrorNew(Option<Box<Expr>>),          // new Error() or new Error(message) -> Error object
    ErrorMessage(Box<Expr>),              // error.message -> string

    // URL operations
    /// new URL(url) or new URL(url, base) -> URL object (stored as pointer)
    UrlNew {
        url: Box<Expr>,
        base: Option<Box<Expr>>,
    },
    /// url.href -> string (full URL)
    UrlGetHref(Box<Expr>),
    /// url.pathname -> string (path portion)
    UrlGetPathname(Box<Expr>),
    /// url.protocol -> string (e.g., "https:")
    UrlGetProtocol(Box<Expr>),
    /// url.host -> string (hostname:port)
    UrlGetHost(Box<Expr>),
    /// url.hostname -> string (hostname without port)
    UrlGetHostname(Box<Expr>),
    /// url.port -> string (port number as string)
    UrlGetPort(Box<Expr>),
    /// url.search -> string (query string including ?)
    UrlGetSearch(Box<Expr>),
    /// url.hash -> string (fragment including #)
    UrlGetHash(Box<Expr>),
    /// url.origin -> string (protocol + host)
    UrlGetOrigin(Box<Expr>),
    /// url.searchParams -> URLSearchParams object
    UrlGetSearchParams(Box<Expr>),

    // URLSearchParams operations
    /// new URLSearchParams(init?)
    UrlSearchParamsNew(Option<Box<Expr>>),
    /// params.get(name) -> string | null
    UrlSearchParamsGet {
        params: Box<Expr>,
        name: Box<Expr>,
    },
    /// params.has(name) -> boolean
    UrlSearchParamsHas {
        params: Box<Expr>,
        name: Box<Expr>,
    },
    /// params.set(name, value)
    UrlSearchParamsSet {
        params: Box<Expr>,
        name: Box<Expr>,
        value: Box<Expr>,
    },
    /// params.append(name, value)
    UrlSearchParamsAppend {
        params: Box<Expr>,
        name: Box<Expr>,
        value: Box<Expr>,
    },
    /// params.delete(name)
    UrlSearchParamsDelete {
        params: Box<Expr>,
        name: Box<Expr>,
    },
    /// params.toString() -> string
    UrlSearchParamsToString(Box<Expr>),
    /// params.getAll(name) -> string[]
    UrlSearchParamsGetAll {
        params: Box<Expr>,
        name: Box<Expr>,
    },

    // Delete operator
    Delete(Box<Expr>),                    // delete obj.prop or delete obj["prop"] -> bool

    // Closure (inline function/arrow function)
    Closure {
        /// Unique ID for this closure's underlying function
        func_id: FuncId,
        /// Parameter definitions
        params: Vec<Param>,
        /// Return type
        return_type: Type,
        /// Function body
        body: Vec<Stmt>,
        /// Variables captured from enclosing scope
        captures: Vec<LocalId>,
        /// Captured variables that are modified (need boxing)
        mutable_captures: Vec<LocalId>,
        /// Whether this closure captures `this` from the enclosing scope (arrow function semantics)
        captures_this: bool,
        /// The enclosing class name if this closure captures `this` (for field access during codegen)
        enclosing_class: Option<String>,
    },

    // RegExp operations
    /// RegExp literal: /pattern/flags
    RegExp {
        pattern: String,
        flags: String,
    },
    /// regex.test(string) -> boolean
    RegExpTest {
        regex: Box<Expr>,
        string: Box<Expr>,
    },
    /// string.match(regex) -> string[] | null
    StringMatch {
        string: Box<Expr>,
        regex: Box<Expr>,
    },
    /// string.replace(regex, replacement) -> string
    StringReplace {
        string: Box<Expr>,
        pattern: Box<Expr>,
        replacement: Box<Expr>,
    },

    // Object operations
    /// Object.keys(obj) -> string[]
    /// Returns an array of the object's own enumerable property names
    ObjectKeys(Box<Expr>),

    // Global built-in functions
    /// parseInt(string, radix?) -> number
    /// Parses a string and returns an integer
    ParseInt {
        string: Box<Expr>,
        radix: Option<Box<Expr>>,
    },
    /// parseFloat(string) -> number
    /// Parses a string and returns a floating-point number
    ParseFloat(Box<Expr>),
    /// Number(value) -> number
    /// Type coercion to number
    NumberCoerce(Box<Expr>),
    /// String(value) -> string
    /// Type coercion to string
    StringCoerce(Box<Expr>),
    /// isNaN(value) -> boolean
    /// Check if value is NaN
    IsNaN(Box<Expr>),
    /// isFinite(value) -> boolean
    /// Check if value is finite
    IsFinite(Box<Expr>),

    // V8 JavaScript Runtime interop
    // These expressions are used for modules loaded via the V8 interpreter

    /// Load a JavaScript module via V8 runtime
    /// Returns a module handle (u64) for subsequent calls
    JsLoadModule {
        /// Path to the JavaScript module
        path: String,
    },

    /// Get an export from a V8-loaded module
    JsGetExport {
        /// Module handle from JsLoadModule
        module_handle: Box<Expr>,
        /// Name of the export to retrieve
        export_name: String,
    },

    /// Call a function from a V8-loaded module
    JsCallFunction {
        /// Module handle from JsLoadModule
        module_handle: Box<Expr>,
        /// Name of the function to call
        func_name: String,
        /// Arguments to pass to the function
        args: Vec<Expr>,
    },

    /// Call a method on a V8 JavaScript object
    JsCallMethod {
        /// The object to call the method on
        object: Box<Expr>,
        /// Name of the method to call
        method_name: String,
        /// Arguments to pass to the method
        args: Vec<Expr>,
    },

    /// Get a property from a V8 JavaScript object
    JsGetProperty {
        /// The object to get the property from
        object: Box<Expr>,
        /// Name of the property to get
        property_name: String,
    },

    /// Set a property on a V8 JavaScript object
    JsSetProperty {
        /// The object to set the property on
        object: Box<Expr>,
        /// Name of the property to set
        property_name: String,
        /// Value to set
        value: Box<Expr>,
    },

    /// Create a new instance of a V8 JavaScript class
    JsNew {
        /// Module handle from JsLoadModule
        module_handle: Box<Expr>,
        /// Name of the class to instantiate
        class_name: String,
        /// Arguments to pass to the constructor
        args: Vec<Expr>,
    },

    /// Create a new instance from a V8 JS handle to a constructor
    JsNewFromHandle {
        /// JS handle to the constructor function
        constructor: Box<Expr>,
        /// Arguments to pass to the constructor
        args: Vec<Expr>,
    },

    /// Create a V8 function that wraps a native callback
    JsCreateCallback {
        /// The closure expression to wrap
        closure: Box<Expr>,
        /// Number of parameters the callback expects
        param_count: usize,
    },

    /// import.meta.url - returns the URL of the current module
    /// The string is the file:// URL of the source file
    ImportMetaUrl(String),
}

/// Binary operators
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
    UShr,
}

/// Unary operators
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,
    Not,
    BitNot,
    Pos,
}

/// Comparison operators
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompareOp {
    Eq,    // ===
    Ne,    // !==
    Lt,    // <
    Le,    // <=
    Gt,    // >
    Ge,    // >=
}

/// Logical operators
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogicalOp {
    And, // &&
    Or,  // ||
    Coalesce, // ??
}

/// Update operators (++/--)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateOp {
    Increment, // ++
    Decrement, // --
}

/// Element in an array literal with spread support
#[derive(Debug, Clone)]
pub enum ArrayElement {
    /// Regular element: [1, 2, 3]
    Expr(Expr),
    /// Spread element: [...arr]
    Spread(Expr),
}

/// Argument in a function call with spread support
#[derive(Debug, Clone)]
pub enum CallArg {
    /// Regular argument: fn(x, y)
    Expr(Expr),
    /// Spread argument: fn(...arr)
    Spread(Expr),
}

impl Module {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            imports: Vec::new(),
            exports: Vec::new(),
            classes: Vec::new(),
            interfaces: Vec::new(),
            type_aliases: Vec::new(),
            enums: Vec::new(),
            globals: Vec::new(),
            functions: Vec::new(),
            init: Vec::new(),
            exported_native_instances: Vec::new(),
            exported_objects: Vec::new(),
            exported_functions: Vec::new(),
        }
    }
}
