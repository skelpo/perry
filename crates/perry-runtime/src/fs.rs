//! File system module - provides file operations

use std::fs;
use std::path::Path;

use crate::string::{js_string_from_bytes, StringHeader};

/// Read a file synchronously and return its contents as a string
/// Returns null pointer on error
#[no_mangle]
pub extern "C" fn js_fs_read_file_sync(path_ptr: *const StringHeader) -> *mut StringHeader {
    unsafe {
        if path_ptr.is_null() {
            return std::ptr::null_mut();
        }

        let len = (*path_ptr).length as usize;
        let data_ptr = (path_ptr as *const u8).add(std::mem::size_of::<StringHeader>());
        let path_bytes = std::slice::from_raw_parts(data_ptr, len);

        let path_str = match std::str::from_utf8(path_bytes) {
            Ok(s) => s,
            Err(_) => return std::ptr::null_mut(),
        };

        match fs::read_to_string(path_str) {
            Ok(content) => {
                let bytes = content.as_bytes();
                js_string_from_bytes(bytes.as_ptr(), bytes.len() as u32)
            }
            Err(_) => std::ptr::null_mut(),
        }
    }
}

/// Write content to a file synchronously
/// Returns 1 on success, 0 on failure
#[no_mangle]
pub extern "C" fn js_fs_write_file_sync(
    path_ptr: *const StringHeader,
    content_ptr: *const StringHeader,
) -> i32 {
    unsafe {
        if path_ptr.is_null() || content_ptr.is_null() {
            return 0;
        }

        // Get path string
        let path_len = (*path_ptr).length as usize;
        let path_data = (path_ptr as *const u8).add(std::mem::size_of::<StringHeader>());
        let path_bytes = std::slice::from_raw_parts(path_data, path_len);
        let path_str = match std::str::from_utf8(path_bytes) {
            Ok(s) => s,
            Err(_) => return 0,
        };

        // Get content string
        let content_len = (*content_ptr).length as usize;
        let content_data = (content_ptr as *const u8).add(std::mem::size_of::<StringHeader>());
        let content_bytes = std::slice::from_raw_parts(content_data, content_len);

        match fs::write(path_str, content_bytes) {
            Ok(_) => 1,
            Err(_) => 0,
        }
    }
}

/// Check if a file or directory exists
/// Returns 1 if exists, 0 if not
#[no_mangle]
pub extern "C" fn js_fs_exists_sync(path_ptr: *const StringHeader) -> i32 {
    unsafe {
        if path_ptr.is_null() {
            return 0;
        }

        let len = (*path_ptr).length as usize;
        let data_ptr = (path_ptr as *const u8).add(std::mem::size_of::<StringHeader>());
        let path_bytes = std::slice::from_raw_parts(data_ptr, len);

        let path_str = match std::str::from_utf8(path_bytes) {
            Ok(s) => s,
            Err(_) => return 0,
        };

        if Path::new(path_str).exists() { 1 } else { 0 }
    }
}

/// Create a directory synchronously
/// Returns 1 on success, 0 on failure
#[no_mangle]
pub extern "C" fn js_fs_mkdir_sync(path_ptr: *const StringHeader) -> i32 {
    unsafe {
        if path_ptr.is_null() {
            return 0;
        }

        let len = (*path_ptr).length as usize;
        let data_ptr = (path_ptr as *const u8).add(std::mem::size_of::<StringHeader>());
        let path_bytes = std::slice::from_raw_parts(data_ptr, len);

        let path_str = match std::str::from_utf8(path_bytes) {
            Ok(s) => s,
            Err(_) => return 0,
        };

        match fs::create_dir_all(path_str) {
            Ok(_) => 1,
            Err(_) => 0,
        }
    }
}

/// Remove a file synchronously
/// Returns 1 on success, 0 on failure
#[no_mangle]
pub extern "C" fn js_fs_unlink_sync(path_ptr: *const StringHeader) -> i32 {
    unsafe {
        if path_ptr.is_null() {
            return 0;
        }

        let len = (*path_ptr).length as usize;
        let data_ptr = (path_ptr as *const u8).add(std::mem::size_of::<StringHeader>());
        let path_bytes = std::slice::from_raw_parts(data_ptr, len);

        let path_str = match std::str::from_utf8(path_bytes) {
            Ok(s) => s,
            Err(_) => return 0,
        };

        match fs::remove_file(path_str) {
            Ok(_) => 1,
            Err(_) => 0,
        }
    }
}
