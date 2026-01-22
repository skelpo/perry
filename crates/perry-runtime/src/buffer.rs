//! Buffer module - provides binary data handling similar to Node.js Buffer

use std::alloc::{alloc, Layout};
use std::ptr;

use crate::string::{js_string_from_bytes, StringHeader};
use crate::array::ArrayHeader;

/// Buffer header - similar to StringHeader but specifically for binary data
#[repr(C)]
pub struct BufferHeader {
    /// Length in bytes
    pub length: u32,
    /// Capacity (allocated space)
    pub capacity: u32,
}

/// Calculate the layout for a buffer with given capacity
fn buffer_layout(capacity: usize) -> Layout {
    let total_size = std::mem::size_of::<BufferHeader>() + capacity;
    Layout::from_size_align(total_size, 8).unwrap()
}

/// Allocate a buffer with the given capacity
fn buffer_alloc(capacity: u32) -> *mut BufferHeader {
    let layout = buffer_layout(capacity as usize);
    unsafe {
        let ptr = alloc(layout) as *mut BufferHeader;
        if ptr.is_null() {
            panic!("Failed to allocate buffer");
        }
        (*ptr).length = 0;
        (*ptr).capacity = capacity;
        ptr
    }
}

/// Get the data pointer for a buffer
fn buffer_data(buf: *const BufferHeader) -> *const u8 {
    unsafe {
        (buf as *const u8).add(std::mem::size_of::<BufferHeader>())
    }
}

/// Get the mutable data pointer for a buffer
fn buffer_data_mut(buf: *mut BufferHeader) -> *mut u8 {
    unsafe {
        (buf as *mut u8).add(std::mem::size_of::<BufferHeader>())
    }
}

/// Create a Buffer from a string
/// encoding: 0 = utf8 (default), 1 = hex, 2 = base64
#[no_mangle]
pub extern "C" fn js_buffer_from_string(str_ptr: *const StringHeader, encoding: i32) -> *mut BufferHeader {
    if str_ptr.is_null() {
        return buffer_alloc(0);
    }

    unsafe {
        let len = (*str_ptr).length as usize;
        let data_ptr = (str_ptr as *const u8).add(std::mem::size_of::<StringHeader>());
        let str_bytes = std::slice::from_raw_parts(data_ptr, len);

        match encoding {
            1 => {
                // Hex encoding
                let decoded = decode_hex(str_bytes);
                let buf = buffer_alloc(decoded.len() as u32);
                (*buf).length = decoded.len() as u32;
                ptr::copy_nonoverlapping(decoded.as_ptr(), buffer_data_mut(buf), decoded.len());
                buf
            }
            2 => {
                // Base64 encoding
                let decoded = decode_base64(str_bytes);
                let buf = buffer_alloc(decoded.len() as u32);
                (*buf).length = decoded.len() as u32;
                ptr::copy_nonoverlapping(decoded.as_ptr(), buffer_data_mut(buf), decoded.len());
                buf
            }
            _ => {
                // UTF-8 (default)
                let buf = buffer_alloc(len as u32);
                (*buf).length = len as u32;
                ptr::copy_nonoverlapping(str_bytes.as_ptr(), buffer_data_mut(buf), len);
                buf
            }
        }
    }
}

/// Create a Buffer from an array of numbers
#[no_mangle]
pub extern "C" fn js_buffer_from_array(arr_ptr: *const ArrayHeader) -> *mut BufferHeader {
    if arr_ptr.is_null() {
        return buffer_alloc(0);
    }

    unsafe {
        let len = (*arr_ptr).length as usize;
        let buf = buffer_alloc(len as u32);
        (*buf).length = len as u32;

        let arr_data = (arr_ptr as *const u8).add(std::mem::size_of::<ArrayHeader>()) as *const f64;
        let buf_data = buffer_data_mut(buf);

        for i in 0..len {
            let val = *arr_data.add(i);
            *buf_data.add(i) = (val as u32 & 0xFF) as u8;
        }

        buf
    }
}

/// Allocate a zero-filled buffer
#[no_mangle]
pub extern "C" fn js_buffer_alloc(size: i32, fill: i32) -> *mut BufferHeader {
    let size = size.max(0) as u32;
    let buf = buffer_alloc(size);
    unsafe {
        (*buf).length = size;
        let data = buffer_data_mut(buf);
        ptr::write_bytes(data, fill as u8, size as usize);
    }
    buf
}

/// Allocate an uninitialized buffer
#[no_mangle]
pub extern "C" fn js_buffer_alloc_unsafe(size: i32) -> *mut BufferHeader {
    let size = size.max(0) as u32;
    let buf = buffer_alloc(size);
    unsafe {
        (*buf).length = size;
    }
    buf
}

/// Concatenate multiple buffers
#[no_mangle]
pub extern "C" fn js_buffer_concat(arr_ptr: *const ArrayHeader) -> *mut BufferHeader {
    if arr_ptr.is_null() {
        return buffer_alloc(0);
    }

    unsafe {
        let len = (*arr_ptr).length as usize;
        let arr_data = (arr_ptr as *const u8).add(std::mem::size_of::<ArrayHeader>()) as *const f64;

        // Calculate total size
        let mut total_size: usize = 0;
        for i in 0..len {
            let buf_ptr = (*arr_data.add(i)).to_bits() as *const BufferHeader;
            if !buf_ptr.is_null() {
                total_size += (*buf_ptr).length as usize;
            }
        }

        // Allocate result buffer
        let result = buffer_alloc(total_size as u32);
        (*result).length = total_size as u32;

        // Copy data
        let mut offset: usize = 0;
        for i in 0..len {
            let buf_ptr = (*arr_data.add(i)).to_bits() as *const BufferHeader;
            if !buf_ptr.is_null() {
                let buf_len = (*buf_ptr).length as usize;
                let src_data = buffer_data(buf_ptr);
                let dst_data = buffer_data_mut(result).add(offset);
                ptr::copy_nonoverlapping(src_data, dst_data, buf_len);
                offset += buf_len;
            }
        }

        result
    }
}

/// Check if an object is a Buffer (by checking the header)
#[no_mangle]
pub extern "C" fn js_buffer_is_buffer(ptr: i64) -> i32 {
    // For now, we can't really distinguish between Buffer and other objects
    // In a real implementation, we'd have type tags
    if ptr == 0 {
        return 0;
    }
    // Return 1 if it looks like a valid buffer pointer
    1
}

/// Get the byte length of a string (when encoded to UTF-8)
#[no_mangle]
pub extern "C" fn js_buffer_byte_length(str_ptr: *const StringHeader) -> i32 {
    if str_ptr.is_null() {
        return 0;
    }
    unsafe {
        (*str_ptr).length as i32
    }
}

/// Convert a buffer to a string
/// encoding: 0 = utf8 (default), 1 = hex, 2 = base64
#[no_mangle]
pub extern "C" fn js_buffer_to_string(buf_ptr: *const BufferHeader, encoding: i32) -> *mut StringHeader {
    if buf_ptr.is_null() {
        return js_string_from_bytes(ptr::null(), 0);
    }

    unsafe {
        let len = (*buf_ptr).length as usize;
        let data = buffer_data(buf_ptr);
        let bytes = std::slice::from_raw_parts(data, len);

        match encoding {
            1 => {
                // Hex encoding
                let hex = encode_hex(bytes);
                js_string_from_bytes(hex.as_ptr(), hex.len() as u32)
            }
            2 => {
                // Base64 encoding
                let b64 = encode_base64(bytes);
                js_string_from_bytes(b64.as_ptr(), b64.len() as u32)
            }
            _ => {
                // UTF-8 (default)
                js_string_from_bytes(data, len as u32)
            }
        }
    }
}

/// Get the length of a buffer
#[no_mangle]
pub extern "C" fn js_buffer_length(buf_ptr: *const BufferHeader) -> i32 {
    if buf_ptr.is_null() {
        return 0;
    }
    unsafe { (*buf_ptr).length as i32 }
}

/// Get a byte at the specified index
#[no_mangle]
pub extern "C" fn js_buffer_get(buf_ptr: *const BufferHeader, index: i32) -> i32 {
    if buf_ptr.is_null() || index < 0 {
        return 0;
    }
    unsafe {
        if index as u32 >= (*buf_ptr).length {
            return 0;
        }
        let data = buffer_data(buf_ptr);
        *data.add(index as usize) as i32
    }
}

/// Set a byte at the specified index
#[no_mangle]
pub extern "C" fn js_buffer_set(buf_ptr: *mut BufferHeader, index: i32, value: i32) {
    if buf_ptr.is_null() || index < 0 {
        return;
    }
    unsafe {
        if index as u32 >= (*buf_ptr).length {
            return;
        }
        let data = buffer_data_mut(buf_ptr);
        *data.add(index as usize) = (value & 0xFF) as u8;
    }
}

/// Create a slice of a buffer (returns a new buffer)
#[no_mangle]
pub extern "C" fn js_buffer_slice(buf_ptr: *const BufferHeader, start: i32, end: i32) -> *mut BufferHeader {
    if buf_ptr.is_null() {
        return buffer_alloc(0);
    }

    unsafe {
        let len = (*buf_ptr).length as i32;

        // Handle negative indices
        let start = if start < 0 { (len + start).max(0) } else { start.min(len) };
        let end = if end < 0 { (len + end).max(0) } else { end.min(len) };

        if start >= end {
            return buffer_alloc(0);
        }

        let slice_len = (end - start) as u32;
        let result = buffer_alloc(slice_len);
        (*result).length = slice_len;

        let src_data = buffer_data(buf_ptr).add(start as usize);
        let dst_data = buffer_data_mut(result);
        ptr::copy_nonoverlapping(src_data, dst_data, slice_len as usize);

        result
    }
}

/// Copy data from source buffer to target buffer
/// Returns the number of bytes copied
#[no_mangle]
pub extern "C" fn js_buffer_copy(
    src_ptr: *const BufferHeader,
    dst_ptr: *mut BufferHeader,
    target_start: i32,
    source_start: i32,
    source_end: i32,
) -> i32 {
    if src_ptr.is_null() || dst_ptr.is_null() {
        return 0;
    }

    unsafe {
        let src_len = (*src_ptr).length as i32;
        let dst_len = (*dst_ptr).length as i32;

        let target_start = target_start.max(0).min(dst_len);
        let source_start = source_start.max(0).min(src_len);
        let source_end = if source_end < 0 { src_len } else { source_end.min(src_len) };

        if source_start >= source_end {
            return 0;
        }

        let copy_len = (source_end - source_start).min(dst_len - target_start);
        if copy_len <= 0 {
            return 0;
        }

        let src_data = buffer_data(src_ptr).add(source_start as usize);
        let dst_data = buffer_data_mut(dst_ptr).add(target_start as usize);
        ptr::copy_nonoverlapping(src_data, dst_data, copy_len as usize);

        copy_len
    }
}

/// Write a string to a buffer
/// Returns the number of bytes written
#[no_mangle]
pub extern "C" fn js_buffer_write(
    buf_ptr: *mut BufferHeader,
    str_ptr: *const StringHeader,
    offset: i32,
    encoding: i32,
) -> i32 {
    if buf_ptr.is_null() || str_ptr.is_null() {
        return 0;
    }

    unsafe {
        let buf_len = (*buf_ptr).length as i32;
        let offset = offset.max(0).min(buf_len);

        let str_len = (*str_ptr).length as usize;
        let str_data = (str_ptr as *const u8).add(std::mem::size_of::<StringHeader>());
        let str_bytes = std::slice::from_raw_parts(str_data, str_len);

        let bytes_to_write = match encoding {
            1 => decode_hex(str_bytes),
            2 => decode_base64(str_bytes),
            _ => str_bytes.to_vec(),
        };

        let available = (buf_len - offset) as usize;
        let write_len = bytes_to_write.len().min(available);

        let dst_data = buffer_data_mut(buf_ptr).add(offset as usize);
        ptr::copy_nonoverlapping(bytes_to_write.as_ptr(), dst_data, write_len);

        write_len as i32
    }
}

/// Compare two buffers for equality
#[no_mangle]
pub extern "C" fn js_buffer_equals(buf1_ptr: *const BufferHeader, buf2_ptr: *const BufferHeader) -> i32 {
    if buf1_ptr.is_null() && buf2_ptr.is_null() {
        return 1;
    }
    if buf1_ptr.is_null() || buf2_ptr.is_null() {
        return 0;
    }

    unsafe {
        let len1 = (*buf1_ptr).length;
        let len2 = (*buf2_ptr).length;

        if len1 != len2 {
            return 0;
        }

        let data1 = buffer_data(buf1_ptr);
        let data2 = buffer_data(buf2_ptr);

        for i in 0..len1 as usize {
            if *data1.add(i) != *data2.add(i) {
                return 0;
            }
        }

        1
    }
}

// Helper functions for encoding/decoding

fn decode_hex(input: &[u8]) -> Vec<u8> {
    let mut result = Vec::with_capacity(input.len() / 2);
    let mut i = 0;
    while i + 1 < input.len() {
        let high = hex_char_to_value(input[i]);
        let low = hex_char_to_value(input[i + 1]);
        if high < 16 && low < 16 {
            result.push((high << 4) | low);
        }
        i += 2;
    }
    result
}

fn hex_char_to_value(c: u8) -> u8 {
    match c {
        b'0'..=b'9' => c - b'0',
        b'a'..=b'f' => c - b'a' + 10,
        b'A'..=b'F' => c - b'A' + 10,
        _ => 16, // Invalid
    }
}

fn encode_hex(input: &[u8]) -> Vec<u8> {
    const HEX_CHARS: &[u8] = b"0123456789abcdef";
    let mut result = Vec::with_capacity(input.len() * 2);
    for &byte in input {
        result.push(HEX_CHARS[(byte >> 4) as usize]);
        result.push(HEX_CHARS[(byte & 0xF) as usize]);
    }
    result
}

fn decode_base64(input: &[u8]) -> Vec<u8> {
    // Simple base64 decoder
    const DECODE_TABLE: [u8; 256] = {
        let mut table = [64u8; 256];
        let mut i = 0u8;
        while i < 26 {
            table[(b'A' + i) as usize] = i;
            table[(b'a' + i) as usize] = i + 26;
            i += 1;
        }
        let mut i = 0u8;
        while i < 10 {
            table[(b'0' + i) as usize] = i + 52;
            i += 1;
        }
        table[b'+' as usize] = 62;
        table[b'/' as usize] = 63;
        table
    };

    let mut result = Vec::with_capacity(input.len() * 3 / 4);
    let mut buf: u32 = 0;
    let mut buf_bits: u32 = 0;

    for &byte in input {
        if byte == b'=' {
            break;
        }
        let val = DECODE_TABLE[byte as usize];
        if val == 64 {
            continue; // Skip invalid characters
        }
        buf = (buf << 6) | val as u32;
        buf_bits += 6;

        if buf_bits >= 8 {
            buf_bits -= 8;
            result.push((buf >> buf_bits) as u8);
            buf &= (1 << buf_bits) - 1;
        }
    }

    result
}

fn encode_base64(input: &[u8]) -> Vec<u8> {
    const ENCODE_TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut result = Vec::with_capacity((input.len() + 2) / 3 * 4);
    let mut i = 0;

    while i + 2 < input.len() {
        let n = ((input[i] as u32) << 16) | ((input[i + 1] as u32) << 8) | (input[i + 2] as u32);
        result.push(ENCODE_TABLE[(n >> 18) as usize]);
        result.push(ENCODE_TABLE[((n >> 12) & 0x3F) as usize]);
        result.push(ENCODE_TABLE[((n >> 6) & 0x3F) as usize]);
        result.push(ENCODE_TABLE[(n & 0x3F) as usize]);
        i += 3;
    }

    if i + 1 < input.len() {
        let n = ((input[i] as u32) << 16) | ((input[i + 1] as u32) << 8);
        result.push(ENCODE_TABLE[(n >> 18) as usize]);
        result.push(ENCODE_TABLE[((n >> 12) & 0x3F) as usize]);
        result.push(ENCODE_TABLE[((n >> 6) & 0x3F) as usize]);
        result.push(b'=');
    } else if i < input.len() {
        let n = (input[i] as u32) << 16;
        result.push(ENCODE_TABLE[(n >> 18) as usize]);
        result.push(ENCODE_TABLE[((n >> 12) & 0x3F) as usize]);
        result.push(b'=');
        result.push(b'=');
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_alloc() {
        let buf = js_buffer_alloc(10, 0);
        assert_eq!(js_buffer_length(buf), 10);
        for i in 0..10 {
            assert_eq!(js_buffer_get(buf, i), 0);
        }
    }

    #[test]
    fn test_buffer_alloc_with_fill() {
        let buf = js_buffer_alloc(5, 0x42);
        assert_eq!(js_buffer_length(buf), 5);
        for i in 0..5 {
            assert_eq!(js_buffer_get(buf, i), 0x42);
        }
    }

    #[test]
    fn test_buffer_get_set() {
        let buf = js_buffer_alloc(5, 0);
        js_buffer_set(buf, 2, 0x42);
        assert_eq!(js_buffer_get(buf, 2), 0x42);
    }

    #[test]
    fn test_hex_encode_decode() {
        let original = b"Hello";
        let encoded = encode_hex(original);
        let decoded = decode_hex(&encoded);
        assert_eq!(decoded, original);
    }

    #[test]
    fn test_base64_encode_decode() {
        let original = b"Hello, World!";
        let encoded = encode_base64(original);
        let decoded = decode_base64(&encoded);
        assert_eq!(decoded, original);
    }
}
