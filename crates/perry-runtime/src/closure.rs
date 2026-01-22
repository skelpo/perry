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
