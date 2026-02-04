//! Closure runtime support for Perry
//!
//! A closure is a function pointer plus captured environment.
//! Layout:
//!   - ClosureHeader at the start
//!   - Followed by captured values (as f64 or i64 pointers)

use std::alloc::{alloc, Layout};

/// Header for heap-allocated closures
#[repr(C)]
pub struct ClosureHeader {
    /// Function pointer (the actual compiled function)
    pub func_ptr: *const u8,
    /// Number of captured values
    pub capture_count: u32,
    /// Reserved for future use (e.g., closure type tag)
    pub _reserved: u32,
}

/// Allocate a closure with space for captured values
/// Returns pointer to ClosureHeader
#[no_mangle]
pub extern "C" fn js_closure_alloc(func_ptr: *const u8, capture_count: u32) -> *mut ClosureHeader {
    let captures_size = (capture_count as usize) * 8; // Each capture is 8 bytes (f64 or i64)
    let total_size = std::mem::size_of::<ClosureHeader>() + captures_size;
    let layout = Layout::from_size_align(total_size, 8).unwrap();

    unsafe {
        let ptr = alloc(layout) as *mut ClosureHeader;
        if ptr.is_null() {
            panic!("Failed to allocate closure");
        }

        (*ptr).func_ptr = func_ptr;
        (*ptr).capture_count = capture_count;
        (*ptr)._reserved = 0;

        ptr
    }
}

/// Get the function pointer from a closure
#[no_mangle]
pub extern "C" fn js_closure_get_func(closure: *const ClosureHeader) -> *const u8 {
    unsafe { (*closure).func_ptr }
}

/// Get a captured value (as f64) by index
#[no_mangle]
pub extern "C" fn js_closure_get_capture_f64(closure: *const ClosureHeader, index: u32) -> f64 {
    unsafe {
        let captures_ptr = (closure as *const u8).add(std::mem::size_of::<ClosureHeader>()) as *const f64;
        *captures_ptr.add(index as usize)
    }
}

/// Set a captured value (as f64) by index
#[no_mangle]
pub extern "C" fn js_closure_set_capture_f64(closure: *mut ClosureHeader, index: u32, value: f64) {
    unsafe {
        let captures_ptr = (closure as *mut u8).add(std::mem::size_of::<ClosureHeader>()) as *mut f64;
        *captures_ptr.add(index as usize) = value;
    }
}

/// Get a captured value (as i64 pointer) by index
#[no_mangle]
pub extern "C" fn js_closure_get_capture_ptr(closure: *const ClosureHeader, index: u32) -> i64 {
    unsafe {
        let captures_ptr = (closure as *const u8).add(std::mem::size_of::<ClosureHeader>()) as *const i64;
        *captures_ptr.add(index as usize)
    }
}

/// Set a captured value (as i64 pointer) by index
#[no_mangle]
pub extern "C" fn js_closure_set_capture_ptr(closure: *mut ClosureHeader, index: u32, value: i64) {
    unsafe {
        let captures_ptr = (closure as *mut u8).add(std::mem::size_of::<ClosureHeader>()) as *mut i64;
        *captures_ptr.add(index as usize) = value;
    }
}

/// Call a closure with 0 arguments, returning f64
#[no_mangle]
pub extern "C" fn js_closure_call0(closure: *const ClosureHeader) -> f64 {
    unsafe {
        let func: extern "C" fn(*const ClosureHeader) -> f64 = std::mem::transmute((*closure).func_ptr);
        func(closure)
    }
}

/// Call a closure with 1 argument, returning f64
#[no_mangle]
pub extern "C" fn js_closure_call1(closure: *const ClosureHeader, arg0: f64) -> f64 {
    unsafe {
        let func: extern "C" fn(*const ClosureHeader, f64) -> f64 = std::mem::transmute((*closure).func_ptr);
        func(closure, arg0)
    }
}

/// Call a closure with 2 arguments, returning f64
#[no_mangle]
pub extern "C" fn js_closure_call2(closure: *const ClosureHeader, arg0: f64, arg1: f64) -> f64 {
    unsafe {
        let func: extern "C" fn(*const ClosureHeader, f64, f64) -> f64 = std::mem::transmute((*closure).func_ptr);
        func(closure, arg0, arg1)
    }
}

/// Call a closure with 3 arguments, returning f64
#[no_mangle]
pub extern "C" fn js_closure_call3(closure: *const ClosureHeader, arg0: f64, arg1: f64, arg2: f64) -> f64 {
    unsafe {
        let func: extern "C" fn(*const ClosureHeader, f64, f64, f64) -> f64 = std::mem::transmute((*closure).func_ptr);
        func(closure, arg0, arg1, arg2)
    }
}

/// Call a closure with 4 arguments, returning f64
#[no_mangle]
pub extern "C" fn js_closure_call4(closure: *const ClosureHeader, arg0: f64, arg1: f64, arg2: f64, arg3: f64) -> f64 {
    unsafe {
        let func: extern "C" fn(*const ClosureHeader, f64, f64, f64, f64) -> f64 = std::mem::transmute((*closure).func_ptr);
        func(closure, arg0, arg1, arg2, arg3)
    }
}

/// Call a closure with 5 arguments, returning f64
#[no_mangle]
pub extern "C" fn js_closure_call5(closure: *const ClosureHeader, arg0: f64, arg1: f64, arg2: f64, arg3: f64, arg4: f64) -> f64 {
    unsafe {
        let func: extern "C" fn(*const ClosureHeader, f64, f64, f64, f64, f64) -> f64 = std::mem::transmute((*closure).func_ptr);
        func(closure, arg0, arg1, arg2, arg3, arg4)
    }
}

/// Call a closure with 6 arguments, returning f64
#[no_mangle]
pub extern "C" fn js_closure_call6(closure: *const ClosureHeader, arg0: f64, arg1: f64, arg2: f64, arg3: f64, arg4: f64, arg5: f64) -> f64 {
    unsafe {
        let func: extern "C" fn(*const ClosureHeader, f64, f64, f64, f64, f64, f64) -> f64 = std::mem::transmute((*closure).func_ptr);
        func(closure, arg0, arg1, arg2, arg3, arg4, arg5)
    }
}

/// Call a closure with 7 arguments, returning f64
#[no_mangle]
pub extern "C" fn js_closure_call7(closure: *const ClosureHeader, arg0: f64, arg1: f64, arg2: f64, arg3: f64, arg4: f64, arg5: f64, arg6: f64) -> f64 {
    unsafe {
        let func: extern "C" fn(*const ClosureHeader, f64, f64, f64, f64, f64, f64, f64) -> f64 = std::mem::transmute((*closure).func_ptr);
        func(closure, arg0, arg1, arg2, arg3, arg4, arg5, arg6)
    }
}

/// Call a closure with 8 arguments, returning f64
#[no_mangle]
pub extern "C" fn js_closure_call8(closure: *const ClosureHeader, arg0: f64, arg1: f64, arg2: f64, arg3: f64, arg4: f64, arg5: f64, arg6: f64, arg7: f64) -> f64 {
    unsafe {
        let func: extern "C" fn(*const ClosureHeader, f64, f64, f64, f64, f64, f64, f64, f64) -> f64 = std::mem::transmute((*closure).func_ptr);
        func(closure, arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7)
    }
}

/// Call a JavaScript function value with variable arguments
/// This is the native implementation for dynamic function dispatch.
/// func_value: NaN-boxed f64 containing a closure pointer
/// args_ptr: pointer to array of f64 arguments
/// args_len: number of arguments
/// Returns the result as f64
///
/// NOTE: This function is named js_native_call_value to avoid symbol collision
/// with js_call_value in perry-jsruntime which handles V8 JavaScript values.
#[no_mangle]
pub unsafe extern "C" fn js_native_call_value(
    func_value: f64,
    args_ptr: *const f64,
    args_len: usize,
) -> f64 {
    use crate::value::JSValue;

    let jsval = JSValue::from_bits(func_value.to_bits());

    // Get the closure pointer from the value
    // For native compilation, function values are stored as NaN-boxed pointers
    let closure: *const ClosureHeader = if jsval.is_pointer() {
        jsval.as_pointer()
    } else {
        // Try treating the value directly as a pointer (for i64 representation)
        func_value.to_bits() as *const ClosureHeader
    };

    if closure.is_null() {
        // Return undefined for null/invalid closures
        return f64::from_bits(JSValue::undefined().bits());
    }

    // Call with the appropriate arity
    match args_len {
        0 => js_closure_call0(closure),
        1 => {
            let arg0 = if args_ptr.is_null() { 0.0 } else { *args_ptr };
            js_closure_call1(closure, arg0)
        }
        2 => {
            let arg0 = if args_ptr.is_null() { 0.0 } else { *args_ptr };
            let arg1 = if args_ptr.is_null() { 0.0 } else { *args_ptr.add(1) };
            js_closure_call2(closure, arg0, arg1)
        }
        3 => {
            let arg0 = if args_ptr.is_null() { 0.0 } else { *args_ptr };
            let arg1 = if args_ptr.is_null() { 0.0 } else { *args_ptr.add(1) };
            let arg2 = if args_ptr.is_null() { 0.0 } else { *args_ptr.add(2) };
            js_closure_call3(closure, arg0, arg1, arg2)
        }
        4 => {
            let arg0 = if args_ptr.is_null() { 0.0 } else { *args_ptr };
            let arg1 = if args_ptr.is_null() { 0.0 } else { *args_ptr.add(1) };
            let arg2 = if args_ptr.is_null() { 0.0 } else { *args_ptr.add(2) };
            let arg3 = if args_ptr.is_null() { 0.0 } else { *args_ptr.add(3) };
            js_closure_call4(closure, arg0, arg1, arg2, arg3)
        }
        5 => {
            let arg0 = if args_ptr.is_null() { 0.0 } else { *args_ptr };
            let arg1 = if args_ptr.is_null() { 0.0 } else { *args_ptr.add(1) };
            let arg2 = if args_ptr.is_null() { 0.0 } else { *args_ptr.add(2) };
            let arg3 = if args_ptr.is_null() { 0.0 } else { *args_ptr.add(3) };
            let arg4 = if args_ptr.is_null() { 0.0 } else { *args_ptr.add(4) };
            js_closure_call5(closure, arg0, arg1, arg2, arg3, arg4)
        }
        6 => {
            let arg0 = if args_ptr.is_null() { 0.0 } else { *args_ptr };
            let arg1 = if args_ptr.is_null() { 0.0 } else { *args_ptr.add(1) };
            let arg2 = if args_ptr.is_null() { 0.0 } else { *args_ptr.add(2) };
            let arg3 = if args_ptr.is_null() { 0.0 } else { *args_ptr.add(3) };
            let arg4 = if args_ptr.is_null() { 0.0 } else { *args_ptr.add(4) };
            let arg5 = if args_ptr.is_null() { 0.0 } else { *args_ptr.add(5) };
            js_closure_call6(closure, arg0, arg1, arg2, arg3, arg4, arg5)
        }
        7 => {
            let arg0 = if args_ptr.is_null() { 0.0 } else { *args_ptr };
            let arg1 = if args_ptr.is_null() { 0.0 } else { *args_ptr.add(1) };
            let arg2 = if args_ptr.is_null() { 0.0 } else { *args_ptr.add(2) };
            let arg3 = if args_ptr.is_null() { 0.0 } else { *args_ptr.add(3) };
            let arg4 = if args_ptr.is_null() { 0.0 } else { *args_ptr.add(4) };
            let arg5 = if args_ptr.is_null() { 0.0 } else { *args_ptr.add(5) };
            let arg6 = if args_ptr.is_null() { 0.0 } else { *args_ptr.add(6) };
            js_closure_call7(closure, arg0, arg1, arg2, arg3, arg4, arg5, arg6)
        }
        8 => {
            let arg0 = if args_ptr.is_null() { 0.0 } else { *args_ptr };
            let arg1 = if args_ptr.is_null() { 0.0 } else { *args_ptr.add(1) };
            let arg2 = if args_ptr.is_null() { 0.0 } else { *args_ptr.add(2) };
            let arg3 = if args_ptr.is_null() { 0.0 } else { *args_ptr.add(3) };
            let arg4 = if args_ptr.is_null() { 0.0 } else { *args_ptr.add(4) };
            let arg5 = if args_ptr.is_null() { 0.0 } else { *args_ptr.add(5) };
            let arg6 = if args_ptr.is_null() { 0.0 } else { *args_ptr.add(6) };
            let arg7 = if args_ptr.is_null() { 0.0 } else { *args_ptr.add(7) };
            js_closure_call8(closure, arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7)
        }
        _ => {
            // For more than 8 arguments, we'd need a more generic approach
            // For now, just call with as many as we can handle
            eprintln!("Warning: js_native_call_value called with {} args, only supporting up to 8", args_len);
            let arg0 = if args_ptr.is_null() { 0.0 } else { *args_ptr };
            let arg1 = if args_ptr.is_null() { 0.0 } else { *args_ptr.add(1) };
            let arg2 = if args_ptr.is_null() { 0.0 } else { *args_ptr.add(2) };
            let arg3 = if args_ptr.is_null() { 0.0 } else { *args_ptr.add(3) };
            let arg4 = if args_ptr.is_null() { 0.0 } else { *args_ptr.add(4) };
            let arg5 = if args_ptr.is_null() { 0.0 } else { *args_ptr.add(5) };
            let arg6 = if args_ptr.is_null() { 0.0 } else { *args_ptr.add(6) };
            let arg7 = if args_ptr.is_null() { 0.0 } else { *args_ptr.add(7) };
            js_closure_call8(closure, arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    extern "C" fn test_closure_func(closure: *const ClosureHeader) -> f64 {
        unsafe {
            let captured = js_closure_get_capture_f64(closure, 0);
            captured * 2.0
        }
    }

    #[test]
    fn test_closure_basic() {
        let closure = js_closure_alloc(test_closure_func as *const u8, 1);
        js_closure_set_capture_f64(closure, 0, 21.0);
        let result = js_closure_call0(closure);
        assert_eq!(result, 42.0);
    }
}
