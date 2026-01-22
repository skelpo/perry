//! Process module - provides access to environment and process information

use crate::string::{js_string_from_bytes, StringHeader};

/// Exit the process with the given exit code
/// process.exit(code?: number) -> never
#[no_mangle]
pub extern "C" fn js_process_exit(code: f64) {
    let exit_code = if code.is_nan() || code.is_infinite() {
        1 // Default to 1 for invalid codes
    } else {
        code as i32
    };
    std::process::exit(exit_code);
}

/// Get an environment variable by name (takes JS string pointer)
/// Returns a string pointer, or null (0) if not found
#[no_mangle]
pub extern "C" fn js_getenv(name_ptr: *const StringHeader) -> *mut StringHeader {
    unsafe {
        if name_ptr.is_null() {
            return std::ptr::null_mut();
        }

        let len = (*name_ptr).length as usize;
        let data_ptr = (name_ptr as *const u8).add(std::mem::size_of::<StringHeader>());

        // Convert to Rust string
        let name_bytes = std::slice::from_raw_parts(data_ptr, len);
        let name = match std::str::from_utf8(name_bytes) {
            Ok(s) => s,
            Err(_) => return std::ptr::null_mut(),
        };

        match std::env::var(name) {
            Ok(value) => {
                // Create a JS string from the value
                let bytes = value.as_bytes();
                js_string_from_bytes(bytes.as_ptr(), bytes.len() as u32)
            }
            Err(_) => std::ptr::null_mut(), // Not found, return null
        }
    }
}
