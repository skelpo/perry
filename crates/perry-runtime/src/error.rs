//! Error object implementation for Perry
//!
//! Provides the built-in Error class and its subclasses.

use crate::string::{js_string_from_bytes, StringHeader};
use std::alloc::{alloc, Layout};

/// Object type tag for runtime type discrimination
pub const OBJECT_TYPE_REGULAR: u32 = 1;
pub const OBJECT_TYPE_ERROR: u32 = 2;

/// Error object header
#[repr(C)]
pub struct ErrorHeader {
    /// Type tag to distinguish from regular objects (must be first field!)
    pub object_type: u32,
    /// Padding for alignment
    pub _padding: u32,
    /// Error message as a string pointer
    pub message: *mut StringHeader,
    /// Error name (e.g., "Error", "TypeError")
    pub name: *mut StringHeader,
    /// Stack trace (simplified - just a string for now)
    pub stack: *mut StringHeader,
}

/// Create a new Error with no message
#[no_mangle]
pub extern "C" fn js_error_new() -> *mut ErrorHeader {
    unsafe {
        let layout = Layout::new::<ErrorHeader>();
        let ptr = alloc(layout) as *mut ErrorHeader;
        if ptr.is_null() {
            panic!("Failed to allocate Error");
        }

        // Set type tag to identify as Error object
        (*ptr).object_type = OBJECT_TYPE_ERROR;
        (*ptr)._padding = 0;

        // Create empty message
        let empty_msg = js_string_from_bytes(b"".as_ptr(), 0);
        let error_name = js_string_from_bytes(b"Error".as_ptr(), 5);
        let stack = js_string_from_bytes(b"".as_ptr(), 0);

        (*ptr).message = empty_msg;
        (*ptr).name = error_name;
        (*ptr).stack = stack;

        ptr
    }
}

/// Create a new Error with a message
#[no_mangle]
pub extern "C" fn js_error_new_with_message(message: *mut StringHeader) -> *mut ErrorHeader {
    unsafe {
        let layout = Layout::new::<ErrorHeader>();
        let ptr = alloc(layout) as *mut ErrorHeader;
        if ptr.is_null() {
            panic!("Failed to allocate Error");
        }

        // Set type tag to identify as Error object
        (*ptr).object_type = OBJECT_TYPE_ERROR;
        (*ptr)._padding = 0;

        let error_name = js_string_from_bytes(b"Error".as_ptr(), 5);
        let stack = js_string_from_bytes(b"".as_ptr(), 0);

        (*ptr).message = message;
        (*ptr).name = error_name;
        (*ptr).stack = stack;

        ptr
    }
}

/// Get the message property of an Error
#[no_mangle]
pub extern "C" fn js_error_get_message(error: *mut ErrorHeader) -> *mut StringHeader {
    unsafe {
        if error.is_null() {
            return js_string_from_bytes(b"".as_ptr(), 0);
        }
        (*error).message
    }
}

/// Get the name property of an Error
#[no_mangle]
pub extern "C" fn js_error_get_name(error: *mut ErrorHeader) -> *mut StringHeader {
    unsafe {
        if error.is_null() {
            return js_string_from_bytes(b"Error".as_ptr(), 5);
        }
        (*error).name
    }
}

/// Get the stack property of an Error
#[no_mangle]
pub extern "C" fn js_error_get_stack(error: *mut ErrorHeader) -> *mut StringHeader {
    unsafe {
        if error.is_null() {
            return js_string_from_bytes(b"".as_ptr(), 0);
        }
        (*error).stack
    }
}
