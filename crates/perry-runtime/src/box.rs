//! Box runtime for mutable captured variables
//!
//! When a closure captures a variable that is modified (either in the closure
//! or in the outer scope), we need to store it in a heap-allocated "box" so
//! both scopes share the same storage location.

use std::alloc::{alloc, Layout};

/// A box is simply a heap-allocated f64
#[repr(C)]
pub struct Box {
    pub value: f64,
}

/// Allocate a new box with an initial value
#[no_mangle]
pub extern "C" fn js_box_alloc(initial_value: f64) -> *mut Box {
    unsafe {
        let layout = Layout::new::<Box>();
        let ptr = alloc(layout) as *mut Box;
        if ptr.is_null() {
            panic!("Failed to allocate box");
        }
        (*ptr).value = initial_value;
        ptr
    }
}

/// Get the value from a box
#[no_mangle]
pub extern "C" fn js_box_get(ptr: *mut Box) -> f64 {
    unsafe {
        if ptr.is_null() {
            panic!("Null box pointer");
        }
        (*ptr).value
    }
}

/// Set the value in a box
#[no_mangle]
pub extern "C" fn js_box_set(ptr: *mut Box, value: f64) {
    unsafe {
        if ptr.is_null() {
            panic!("Null box pointer");
        }
        (*ptr).value = value;
    }
}
