//! Path module - provides path manipulation utilities

use std::path::Path;

use crate::string::{js_string_from_bytes, StringHeader};

/// Helper to extract string from StringHeader pointer
unsafe fn string_from_header(ptr: *const StringHeader) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    let len = (*ptr).length as usize;
    let data_ptr = (ptr as *const u8).add(std::mem::size_of::<StringHeader>());
    let bytes = std::slice::from_raw_parts(data_ptr, len);
    std::str::from_utf8(bytes).ok().map(|s| s.to_string())
}

/// Helper to create a JS string from a Rust string
fn string_to_js(s: &str) -> *mut StringHeader {
    let bytes = s.as_bytes();
    js_string_from_bytes(bytes.as_ptr(), bytes.len() as u32)
}

/// Join two path segments
#[no_mangle]
pub extern "C" fn js_path_join(a_ptr: *const StringHeader, b_ptr: *const StringHeader) -> *mut StringHeader {
    unsafe {
        let a = string_from_header(a_ptr).unwrap_or_default();
        let b = string_from_header(b_ptr).unwrap_or_default();

        let joined = Path::new(&a).join(&b);
        let result = joined.to_string_lossy();
        string_to_js(&result)
    }
}

/// Get directory name from path
#[no_mangle]
pub extern "C" fn js_path_dirname(path_ptr: *const StringHeader) -> *mut StringHeader {
    unsafe {
        let path_str = match string_from_header(path_ptr) {
            Some(s) => s,
            None => return string_to_js(""),
        };

        let path = Path::new(&path_str);
        match path.parent() {
            Some(parent) => string_to_js(&parent.to_string_lossy()),
            None => string_to_js(""),
        }
    }
}

/// Get base name (file name) from path
#[no_mangle]
pub extern "C" fn js_path_basename(path_ptr: *const StringHeader) -> *mut StringHeader {
    unsafe {
        let path_str = match string_from_header(path_ptr) {
            Some(s) => s,
            None => return string_to_js(""),
        };

        let path = Path::new(&path_str);
        match path.file_name() {
            Some(name) => string_to_js(&name.to_string_lossy()),
            None => string_to_js(""),
        }
    }
}

/// Get file extension from path (including the dot)
#[no_mangle]
pub extern "C" fn js_path_extname(path_ptr: *const StringHeader) -> *mut StringHeader {
    unsafe {
        let path_str = match string_from_header(path_ptr) {
            Some(s) => s,
            None => return string_to_js(""),
        };

        let path = Path::new(&path_str);
        match path.extension() {
            Some(ext) => {
                let mut result = String::from(".");
                result.push_str(&ext.to_string_lossy());
                string_to_js(&result)
            }
            None => string_to_js(""),
        }
    }
}

/// Resolve path to absolute path
#[no_mangle]
pub extern "C" fn js_path_resolve(path_ptr: *const StringHeader) -> *mut StringHeader {
    unsafe {
        let path_str = match string_from_header(path_ptr) {
            Some(s) => s,
            None => return string_to_js(""),
        };

        match std::fs::canonicalize(&path_str) {
            Ok(abs_path) => string_to_js(&abs_path.to_string_lossy()),
            Err(_) => {
                // If canonicalize fails (file doesn't exist), try to construct absolute path
                if Path::new(&path_str).is_absolute() {
                    string_to_js(&path_str)
                } else {
                    match std::env::current_dir() {
                        Ok(cwd) => {
                            let joined = cwd.join(&path_str);
                            string_to_js(&joined.to_string_lossy())
                        }
                        Err(_) => string_to_js(&path_str),
                    }
                }
            }
        }
    }
}
