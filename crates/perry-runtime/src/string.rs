//! String runtime support for Perry
//!
//! Strings are heap-allocated UTF-8 sequences with capacity for efficient appending.
//! Layout:
//!   - StringHeader at the start
//!   - Followed by `capacity` bytes of data (only `length` bytes are valid)

use std::alloc::{alloc, realloc, Layout};
use std::ptr;
use std::slice;
use std::str;

/// Header for heap-allocated strings
#[repr(C)]
pub struct StringHeader {
    /// Length in bytes (not chars - we store UTF-8)
    pub length: u32,
    /// Capacity (allocated space for data)
    pub capacity: u32,
}

/// Calculate the layout for a string with given capacity
fn string_layout(capacity: usize) -> Layout {
    let total_size = std::mem::size_of::<StringHeader>() + capacity;
    Layout::from_size_align(total_size, 8).unwrap()
}

/// Create a string from raw bytes
/// Returns a pointer to StringHeader
#[no_mangle]
pub extern "C" fn js_string_from_bytes(data: *const u8, len: u32) -> *mut StringHeader {
    js_string_from_bytes_with_capacity(data, len, len)
}

/// Create a string from raw bytes with extra capacity for future appending
#[no_mangle]
pub extern "C" fn js_string_from_bytes_with_capacity(data: *const u8, len: u32, capacity: u32) -> *mut StringHeader {
    let capacity = capacity.max(len); // Ensure capacity >= len
    let layout = string_layout(capacity as usize);

    unsafe {
        let ptr = alloc(layout) as *mut StringHeader;
        if ptr.is_null() {
            panic!("Failed to allocate string");
        }

        (*ptr).length = len;
        (*ptr).capacity = capacity;

        // Copy string data after header
        if len > 0 && !data.is_null() {
            let data_ptr = (ptr as *mut u8).add(std::mem::size_of::<StringHeader>());
            ptr::copy_nonoverlapping(data, data_ptr, len as usize);
        }

        ptr
    }
}

/// Append a string to another string in-place if possible
/// Returns the (possibly reallocated) string pointer
/// This is the key optimization for `str = str + x` patterns
#[no_mangle]
pub extern "C" fn js_string_append(dest: *mut StringHeader, src: *const StringHeader) -> *mut StringHeader {
    if dest.is_null() {
        // If dest is null, just duplicate src
        if src.is_null() {
            return js_string_from_bytes(ptr::null(), 0);
        }
        let src_len = unsafe { (*src).length };
        let src_data = string_data(src);
        return js_string_from_bytes(src_data, src_len);
    }

    if src.is_null() {
        return dest;
    }

    unsafe {
        let dest_len = (*dest).length;
        let dest_cap = (*dest).capacity;
        let src_len = (*src).length;

        if src_len == 0 {
            return dest;
        }

        let new_len = dest_len + src_len;

        // Check if we have enough capacity
        if new_len <= dest_cap {
            // Append in-place - no allocation needed!
            let dest_data = (dest as *mut u8).add(std::mem::size_of::<StringHeader>());
            let src_data = string_data(src);
            ptr::copy_nonoverlapping(src_data, dest_data.add(dest_len as usize), src_len as usize);
            (*dest).length = new_len;
            return dest;
        }

        // Need to reallocate - use 2x growth strategy
        let new_cap = (new_len * 2).max(32); // At least 32 bytes, or 2x needed
        let old_layout = string_layout(dest_cap as usize);
        let new_layout = string_layout(new_cap as usize);

        let new_ptr = realloc(dest as *mut u8, old_layout, new_layout.size()) as *mut StringHeader;
        if new_ptr.is_null() {
            panic!("Failed to reallocate string");
        }

        (*new_ptr).capacity = new_cap;

        // Copy src data
        let dest_data = (new_ptr as *mut u8).add(std::mem::size_of::<StringHeader>());
        let src_data = string_data(src);
        ptr::copy_nonoverlapping(src_data, dest_data.add(dest_len as usize), src_len as usize);
        (*new_ptr).length = new_len;

        new_ptr
    }
}

/// Create an empty string with initial capacity (for building strings)
#[no_mangle]
pub extern "C" fn js_string_builder_new(initial_capacity: u32) -> *mut StringHeader {
    js_string_from_bytes_with_capacity(ptr::null(), 0, initial_capacity.max(16))
}

/// Internal helper: Create a StringHeader from a Rust &str
#[inline]
fn js_string_from_str(s: &str) -> *mut StringHeader {
    js_string_from_bytes(s.as_ptr(), s.len() as u32)
}

/// Get string length (in bytes for now, chars would need UTF-8 counting)
#[no_mangle]
pub extern "C" fn js_string_length(s: *const StringHeader) -> u32 {
    if s.is_null() {
        return 0;
    }
    unsafe { (*s).length }
}

/// Get the data pointer for a string
fn string_data(s: *const StringHeader) -> *const u8 {
    unsafe {
        (s as *const u8).add(std::mem::size_of::<StringHeader>())
    }
}

/// Get string as a Rust &str (for internal use)
fn string_as_str<'a>(s: *const StringHeader) -> &'a str {
    unsafe {
        let len = (*s).length as usize;
        let data = string_data(s);
        let bytes = slice::from_raw_parts(data, len);
        str::from_utf8_unchecked(bytes)
    }
}

/// Concatenate two strings
#[no_mangle]
pub extern "C" fn js_string_concat(a: *const StringHeader, b: *const StringHeader) -> *mut StringHeader {
    let len_a = if a.is_null() { 0 } else { unsafe { (*a).length } };
    let len_b = if b.is_null() { 0 } else { unsafe { (*b).length } };
    let total_len = len_a + len_b;

    let total_size = std::mem::size_of::<StringHeader>() + total_len as usize;
    let layout = Layout::from_size_align(total_size, 8).unwrap();

    unsafe {
        let ptr = alloc(layout) as *mut StringHeader;
        if ptr.is_null() {
            panic!("Failed to allocate string");
        }

        (*ptr).length = total_len;
        (*ptr).capacity = total_len;

        let data_ptr = (ptr as *mut u8).add(std::mem::size_of::<StringHeader>());

        if !a.is_null() && len_a > 0 {
            ptr::copy_nonoverlapping(string_data(a), data_ptr, len_a as usize);
        }
        if !b.is_null() && len_b > 0 {
            ptr::copy_nonoverlapping(string_data(b), data_ptr.add(len_a as usize), len_b as usize);
        }

        ptr
    }
}

/// Convert a number (f64) to a string
/// Returns a new string representing the number
#[no_mangle]
pub extern "C" fn js_number_to_string(value: f64) -> *mut StringHeader {
    // Format the number as a string
    let s = if value.fract() == 0.0 && value.abs() < 1e15 {
        // Integer-like, format without decimal
        format!("{}", value as i64)
    } else {
        // Float, format with appropriate precision
        format!("{}", value)
    };

    let bytes = s.as_bytes();
    js_string_from_bytes(bytes.as_ptr(), bytes.len() as u32)
}

/// Get a slice of a string (byte-based for now)
/// Returns a new string from start to end (exclusive)
#[no_mangle]
pub extern "C" fn js_string_slice(s: *const StringHeader, start: i32, end: i32) -> *mut StringHeader {
    if s.is_null() {
        return js_string_from_bytes(ptr::null(), 0);
    }

    let len = unsafe { (*s).length } as i32;

    // Handle negative indices (from end)
    let start = if start < 0 { (len + start).max(0) } else { start.min(len) };
    let end = if end < 0 { (len + end).max(0) } else { end.min(len) };

    if start >= end {
        return js_string_from_bytes(ptr::null(), 0);
    }

    let slice_len = (end - start) as u32;
    unsafe {
        let src = string_data(s).add(start as usize);
        js_string_from_bytes(src, slice_len)
    }
}

/// Get a substring (similar to slice but different behavior)
/// - Negative indices are treated as 0
/// - If start > end, arguments are swapped
/// Returns a new string from start to end (exclusive)
#[no_mangle]
pub extern "C" fn js_string_substring(s: *const StringHeader, start: i32, end: i32) -> *mut StringHeader {
    if s.is_null() {
        return js_string_from_bytes(ptr::null(), 0);
    }

    let len = unsafe { (*s).length } as i32;

    // Treat negative indices as 0
    let mut start = start.max(0).min(len);
    let mut end = end.max(0).min(len);

    // Swap if start > end
    if start > end {
        std::mem::swap(&mut start, &mut end);
    }

    if start >= end {
        return js_string_from_bytes(ptr::null(), 0);
    }

    let slice_len = (end - start) as u32;
    unsafe {
        let src = string_data(s).add(start as usize);
        js_string_from_bytes(src, slice_len)
    }
}

/// Trim whitespace from both ends of a string
#[no_mangle]
pub extern "C" fn js_string_trim(s: *const StringHeader) -> *mut StringHeader {
    if s.is_null() {
        return js_string_from_bytes(ptr::null(), 0);
    }

    let str_data = string_as_str(s);
    let trimmed = str_data.trim();
    js_string_from_str(trimmed)
}

/// Convert string to lowercase
#[no_mangle]
pub extern "C" fn js_string_to_lower_case(s: *const StringHeader) -> *mut StringHeader {
    if s.is_null() {
        return js_string_from_bytes(ptr::null(), 0);
    }

    let str_data = string_as_str(s);
    let lower = str_data.to_lowercase();
    js_string_from_str(&lower)
}

/// Convert string to uppercase
#[no_mangle]
pub extern "C" fn js_string_to_upper_case(s: *const StringHeader) -> *mut StringHeader {
    if s.is_null() {
        return js_string_from_bytes(ptr::null(), 0);
    }

    let str_data = string_as_str(s);
    let upper = str_data.to_uppercase();
    js_string_from_str(&upper)
}

/// Find index of substring (-1 if not found)
#[no_mangle]
pub extern "C" fn js_string_index_of(haystack: *const StringHeader, needle: *const StringHeader) -> i32 {
    js_string_index_of_from(haystack, needle, 0)
}

/// Find index of substring starting from a given position (-1 if not found)
#[no_mangle]
pub extern "C" fn js_string_index_of_from(haystack: *const StringHeader, needle: *const StringHeader, from_index: i32) -> i32 {
    if haystack.is_null() || needle.is_null() {
        return -1;
    }

    let h = string_as_str(haystack);
    let n = string_as_str(needle);

    // Handle negative or out-of-bounds start index
    let start = if from_index < 0 { 0 } else { from_index as usize };
    if start >= h.len() {
        return -1;
    }

    // Search from the start position
    match h[start..].find(n) {
        Some(pos) => (start + pos) as i32,
        None => -1,
    }
}

/// Compare two strings for equality
#[no_mangle]
pub extern "C" fn js_string_equals(a: *const StringHeader, b: *const StringHeader) -> bool {
    if a.is_null() && b.is_null() {
        return true;
    }
    if a.is_null() || b.is_null() {
        return false;
    }

    let len_a = unsafe { (*a).length };
    let len_b = unsafe { (*b).length };

    if len_a != len_b {
        return false;
    }

    unsafe {
        let data_a = string_data(a);
        let data_b = string_data(b);

        for i in 0..len_a as usize {
            if *data_a.add(i) != *data_b.add(i) {
                return false;
            }
        }
    }

    true
}

/// Get character at index (returns char code, -1 if out of bounds)
#[no_mangle]
pub extern "C" fn js_string_char_code_at(s: *const StringHeader, index: u32) -> i32 {
    if s.is_null() {
        return -1;
    }

    let len = unsafe { (*s).length };
    if index >= len {
        return -1;
    }

    unsafe {
        let data = string_data(s);
        *data.add(index as usize) as i32
    }
}

/// Print a string to stdout
#[no_mangle]
pub extern "C" fn js_string_print(s: *const StringHeader) {
    if s.is_null() {
        println!("");
        return;
    }

    let str_data = string_as_str(s);
    println!("{}", str_data);
}

/// Print a string to stderr (console.error)
#[no_mangle]
pub extern "C" fn js_string_error(s: *const StringHeader) {
    if s.is_null() {
        eprintln!("");
        return;
    }

    let str_data = string_as_str(s);
    eprintln!("{}", str_data);
}

/// Print a string to stderr (console.warn)
#[no_mangle]
pub extern "C" fn js_string_warn(s: *const StringHeader) {
    if s.is_null() {
        eprintln!("");
        return;
    }

    let str_data = string_as_str(s);
    eprintln!("{}", str_data);
}

use crate::array::ArrayHeader;

/// Split a string by a delimiter
/// Returns an array of string pointers (stored as f64 bit patterns)
#[no_mangle]
pub extern "C" fn js_string_split(s: *const StringHeader, delimiter: *const StringHeader) -> *mut ArrayHeader {
    use std::alloc::{alloc, Layout};

    if s.is_null() {
        // Return empty array
        return crate::array::js_array_alloc(0);
    }

    let str_data = string_as_str(s);
    let delim = if delimiter.is_null() {
        ""
    } else {
        string_as_str(delimiter)
    };

    // Split the string
    let parts: Vec<&str> = if delim.is_empty() {
        // Empty delimiter: split into characters
        str_data.chars().map(|c| {
            // We need to create substrings for each char
            // This is tricky with &str, so we'll handle it differently
            ""  // Placeholder, we'll handle below
        }).collect()
    } else {
        str_data.split(delim).collect()
    };

    // Handle empty delimiter specially - split into characters
    let parts: Vec<*mut StringHeader> = if delim.is_empty() {
        str_data.chars().map(|c| {
            let mut buf = [0u8; 4];
            let char_str = c.encode_utf8(&mut buf);
            js_string_from_bytes(char_str.as_ptr(), char_str.len() as u32)
        }).collect()
    } else {
        parts.iter().map(|part| {
            js_string_from_bytes(part.as_ptr(), part.len() as u32)
        }).collect()
    };

    // Allocate array to hold string pointers
    // We store pointers as f64 (bitcast) since arrays currently use f64 storage
    let arr = crate::array::js_array_alloc(parts.len() as u32);
    unsafe {
        (*arr).length = parts.len() as u32;
        let elements_ptr = (arr as *mut u8).add(std::mem::size_of::<ArrayHeader>()) as *mut f64;
        for (i, ptr) in parts.iter().enumerate() {
            // Store the pointer as f64 bits
            let ptr_as_u64 = *ptr as u64;
            let ptr_as_f64 = f64::from_bits(ptr_as_u64);
            std::ptr::write(elements_ptr.add(i), ptr_as_f64);
        }
    }

    arr
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_string_create() {
        let data = b"hello";
        let s = js_string_from_bytes(data.as_ptr(), data.len() as u32);
        assert_eq!(js_string_length(s), 5);
    }

    #[test]
    fn test_string_concat() {
        let a = js_string_from_bytes(b"hello".as_ptr(), 5);
        let b = js_string_from_bytes(b" world".as_ptr(), 6);
        let c = js_string_concat(a, b);
        assert_eq!(js_string_length(c), 11);
        assert_eq!(string_as_str(c), "hello world");
    }

    #[test]
    fn test_string_slice() {
        let s = js_string_from_bytes(b"hello world".as_ptr(), 11);
        let slice = js_string_slice(s, 0, 5);
        assert_eq!(string_as_str(slice), "hello");

        let slice2 = js_string_slice(s, 6, 11);
        assert_eq!(string_as_str(slice2), "world");
    }

    #[test]
    fn test_string_index_of() {
        let s = js_string_from_bytes(b"hello world".as_ptr(), 11);
        let needle = js_string_from_bytes(b"world".as_ptr(), 5);
        assert_eq!(js_string_index_of(s, needle), 6);

        let not_found = js_string_from_bytes(b"xyz".as_ptr(), 3);
        assert_eq!(js_string_index_of(s, not_found), -1);
    }

    #[test]
    fn test_string_split() {
        use crate::array::{js_array_length, js_array_get_f64};

        let s = js_string_from_bytes(b"a,b,c".as_ptr(), 5);
        let delim = js_string_from_bytes(b",".as_ptr(), 1);
        let arr = js_string_split(s, delim);

        assert_eq!(js_array_length(arr), 3);

        // Get the string pointers from the array and verify their contents
        unsafe {
            let ptr0 = js_array_get_f64(arr, 0).to_bits() as *const StringHeader;
            let ptr1 = js_array_get_f64(arr, 1).to_bits() as *const StringHeader;
            let ptr2 = js_array_get_f64(arr, 2).to_bits() as *const StringHeader;

            assert_eq!(string_as_str(ptr0), "a");
            assert_eq!(string_as_str(ptr1), "b");
            assert_eq!(string_as_str(ptr2), "c");
        }
    }
}
