//! Exception handling runtime for Perry
//!
//! Uses setjmp/longjmp for exception unwinding.
//! The key insight is that setjmp must be called directly from the generated code,
//! not from inside a Rust function (because the stack frame would be invalid when longjmp returns).

// Platform-specific jmp_buf size (in i32 units)
// macOS ARM64: _JBLEN = 48 (48 * 4 = 192 bytes)
// macOS x86_64: _JBLEN = 37 (37 * 4 = 148 bytes, but aligned to 156)
// Linux x86_64: __jmp_buf is 8 * i64 = 64 bytes
// We use a conservative size that works for all
const JMP_BUF_SIZE: usize = 64; // 64 * i32 = 256 bytes, enough for any platform

// jmp_buf must be properly aligned
#[repr(C, align(16))]
#[derive(Copy, Clone)]
struct JmpBuf {
    data: [i32; JMP_BUF_SIZE],
}

impl JmpBuf {
    const fn new() -> Self {
        JmpBuf { data: [0; JMP_BUF_SIZE] }
    }

    fn as_mut_ptr(&mut self) -> *mut i32 {
        self.data.as_mut_ptr()
    }
}

extern "C" {
    fn longjmp(env: *mut i32, val: i32) -> !;
}

// Maximum nesting depth for try blocks
const MAX_TRY_DEPTH: usize = 32;

// Static storage for exception handling
static mut JUMP_BUFFERS: [JmpBuf; MAX_TRY_DEPTH] = [JmpBuf::new(); MAX_TRY_DEPTH];
static mut TRY_DEPTH: usize = 0;
static mut CURRENT_EXCEPTION: f64 = 0.0;
static mut HAS_EXCEPTION: bool = false;
static mut IN_FINALLY: bool = false;

/// Push a new try block and return a pointer to its jmp_buf.
/// The generated code must call setjmp() directly with this pointer.
#[no_mangle]
pub extern "C" fn js_try_push() -> *mut i32 {
    unsafe {
        if TRY_DEPTH >= MAX_TRY_DEPTH {
            panic!("Try block nesting too deep");
        }
        let depth = TRY_DEPTH;
        TRY_DEPTH += 1;
        JUMP_BUFFERS[depth].as_mut_ptr()
    }
}

/// End a try block (just decrements depth, does NOT clear exception)
/// The exception is cleared explicitly by js_clear_exception() in catch blocks
#[no_mangle]
pub extern "C" fn js_try_end() {
    unsafe {
        if TRY_DEPTH > 0 {
            TRY_DEPTH -= 1;
        }
    }
}

/// Throw an exception with the given value
#[no_mangle]
pub extern "C" fn js_throw(value: f64) -> ! {
    unsafe {
        CURRENT_EXCEPTION = value;
        HAS_EXCEPTION = true;

        if IN_FINALLY {
            panic!("Cannot throw during finally block");
        }

        if TRY_DEPTH == 0 {
            panic!("Uncaught exception: {}", value);
        }

        // Jump to the most recent try block
        let depth = TRY_DEPTH - 1;
        longjmp(JUMP_BUFFERS[depth].as_mut_ptr(), 1)
    }
}

/// Get the current exception value
#[no_mangle]
pub extern "C" fn js_get_exception() -> f64 {
    unsafe { CURRENT_EXCEPTION }
}

/// Check if there's an active exception
#[no_mangle]
pub extern "C" fn js_has_exception() -> i32 {
    unsafe { if HAS_EXCEPTION { 1 } else { 0 } }
}

/// Clear the current exception
#[no_mangle]
pub extern "C" fn js_clear_exception() {
    unsafe {
        HAS_EXCEPTION = false;
        CURRENT_EXCEPTION = 0.0;
    }
}

/// Mark entering a finally block
#[no_mangle]
pub extern "C" fn js_enter_finally() {
    unsafe { IN_FINALLY = true; }
}

/// Mark leaving a finally block
#[no_mangle]
pub extern "C" fn js_leave_finally() {
    unsafe { IN_FINALLY = false; }
}
