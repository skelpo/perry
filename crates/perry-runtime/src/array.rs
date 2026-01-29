//! Array representation for Perry
//!
//! Arrays are heap-allocated with a header containing:
//! - Length
//! - Capacity
//! - Elements array (inline)

use std::alloc::{alloc, realloc, Layout};
use std::ptr;

/// Array header - precedes the elements in memory
#[repr(C)]
pub struct ArrayHeader {
    /// Number of elements in the array
    pub length: u32,
    /// Capacity (allocated space for elements)
    pub capacity: u32,
}

/// Calculate the layout for an array with N elements capacity
fn array_layout(capacity: usize) -> Layout {
    let header_size = std::mem::size_of::<ArrayHeader>();
    let elements_size = capacity * std::mem::size_of::<f64>(); // Using f64 for now
    let total_size = header_size + elements_size;
    Layout::from_size_align(total_size, 8).unwrap()
}

/// Minimum initial capacity for arrays to reduce reallocations
const MIN_ARRAY_CAPACITY: u32 = 16;

/// Allocate a new array with the given initial capacity
#[no_mangle]
pub extern "C" fn js_array_alloc(capacity: u32) -> *mut ArrayHeader {
    // Use at least MIN_ARRAY_CAPACITY to reduce reallocations for growing arrays
    let actual_capacity = capacity.max(MIN_ARRAY_CAPACITY);
    let layout = array_layout(actual_capacity as usize);
    unsafe {
        let ptr = alloc(layout) as *mut ArrayHeader;
        if ptr.is_null() {
            panic!("Failed to allocate array");
        }

        // Initialize header
        (*ptr).length = 0;
        (*ptr).capacity = actual_capacity;

        // Don't initialize elements - they'll be written before read
        // This saves significant time for large arrays

        ptr
    }
}

/// Allocate and initialize an array from a list of f64 values
#[no_mangle]
pub extern "C" fn js_array_from_f64(elements: *const f64, count: u32) -> *mut ArrayHeader {
    let arr = js_array_alloc(count);
    unsafe {
        (*arr).length = count;
        let arr_elements = (arr as *mut u8).add(std::mem::size_of::<ArrayHeader>()) as *mut f64;
        ptr::copy_nonoverlapping(elements, arr_elements, count as usize);
    }
    arr
}

/// Get the length of an array
#[no_mangle]
pub extern "C" fn js_array_length(arr: *const ArrayHeader) -> u32 {
    unsafe { (*arr).length }
}

/// Get an element from an array by index (returns f64)
#[no_mangle]
pub extern "C" fn js_array_get_f64(arr: *const ArrayHeader, index: u32) -> f64 {
    unsafe {
        let length = (*arr).length;
        if index >= length {
            return f64::NAN; // Out of bounds returns NaN (like undefined coerced to number)
        }
        let elements_ptr = (arr as *const u8).add(std::mem::size_of::<ArrayHeader>()) as *const f64;
        *elements_ptr.add(index as usize)
    }
}

/// Set an element in an array by index
/// Note: This does NOT extend the array if index >= length
#[no_mangle]
pub extern "C" fn js_array_set_f64(arr: *mut ArrayHeader, index: u32, value: f64) {
    unsafe {
        let length = (*arr).length;
        if index >= length {
            return;
        }
        let elements_ptr = (arr as *mut u8).add(std::mem::size_of::<ArrayHeader>()) as *mut f64;
        ptr::write(elements_ptr.add(index as usize), value);
    }
}

/// Set an element in an array by index, extending the array if needed
/// Returns the (possibly reallocated) array pointer
/// This mimics JavaScript's arr[i] = value behavior
#[no_mangle]
pub extern "C" fn js_array_set_f64_extend(arr: *mut ArrayHeader, index: u32, value: f64) -> *mut ArrayHeader {
    unsafe {
        let length = (*arr).length;

        // If index is within bounds, just set it
        if index < length {
            let elements_ptr = (arr as *mut u8).add(std::mem::size_of::<ArrayHeader>()) as *mut f64;
            ptr::write(elements_ptr.add(index as usize), value);
            return arr;
        }

        // Need to extend the array
        let new_length = index + 1;
        let arr = if new_length > (*arr).capacity {
            js_array_grow(arr, new_length)
        } else {
            arr
        };

        // Fill any gap with 0.0 (undefined coerced to number)
        let elements_ptr = (arr as *mut u8).add(std::mem::size_of::<ArrayHeader>()) as *mut f64;
        for i in length..index {
            ptr::write(elements_ptr.add(i as usize), 0.0);
        }

        // Set the value
        ptr::write(elements_ptr.add(index as usize), value);
        (*arr).length = new_length;

        arr
    }
}

/// Grow the array to at least the given capacity
/// Returns a new pointer (the old one may be invalid after this)
#[no_mangle]
pub extern "C" fn js_array_grow(arr: *mut ArrayHeader, min_capacity: u32) -> *mut ArrayHeader {
    unsafe {
        let old_capacity = (*arr).capacity;
        if min_capacity <= old_capacity {
            return arr;
        }

        // Double the capacity, or use min_capacity if larger
        let new_capacity = std::cmp::max(old_capacity * 2, min_capacity);
        let old_layout = array_layout(old_capacity as usize);
        let new_layout = array_layout(new_capacity as usize);

        let new_ptr = realloc(arr as *mut u8, old_layout, new_layout.size()) as *mut ArrayHeader;
        if new_ptr.is_null() {
            panic!("Failed to grow array");
        }

        (*new_ptr).capacity = new_capacity;

        // Don't initialize new elements - they'll be written before read

        new_ptr
    }
}

/// Push an element to the end of an array, growing if needed
/// Returns a pointer to the (possibly reallocated) array
#[no_mangle]
pub extern "C" fn js_array_push_f64(arr: *mut ArrayHeader, value: f64) -> *mut ArrayHeader {
    unsafe {
        let length = (*arr).length;
        let capacity = (*arr).capacity;

        let arr = if length >= capacity {
            js_array_grow(arr, length + 1)
        } else {
            arr
        };

        let elements_ptr = (arr as *mut u8).add(std::mem::size_of::<ArrayHeader>()) as *mut f64;
        ptr::write(elements_ptr.add(length as usize), value);
        (*arr).length = length + 1;
        arr
    }
}

/// Pop an element from the end of an array
/// Returns the removed element (or NaN if empty)
#[no_mangle]
pub extern "C" fn js_array_pop_f64(arr: *mut ArrayHeader) -> f64 {
    unsafe {
        let length = (*arr).length;
        if length == 0 {
            return f64::NAN; // undefined coerced to number
        }

        let new_length = length - 1;
        let elements_ptr = (arr as *mut u8).add(std::mem::size_of::<ArrayHeader>()) as *mut f64;
        let value = *elements_ptr.add(new_length as usize);
        (*arr).length = new_length;
        value
    }
}

/// Shift an element from the beginning of an array
/// Returns the removed element (or NaN if empty)
#[no_mangle]
pub extern "C" fn js_array_shift_f64(arr: *mut ArrayHeader) -> f64 {
    unsafe {
        let length = (*arr).length;
        if length == 0 {
            return f64::NAN;
        }

        let elements_ptr = (arr as *mut u8).add(std::mem::size_of::<ArrayHeader>()) as *mut f64;
        let value = *elements_ptr;

        // Shift all elements down
        ptr::copy(elements_ptr.add(1), elements_ptr, (length - 1) as usize);
        (*arr).length = length - 1;
        value
    }
}

/// Unshift an element to the beginning of an array, growing if needed
/// Returns a pointer to the (possibly reallocated) array
#[no_mangle]
pub extern "C" fn js_array_unshift_f64(arr: *mut ArrayHeader, value: f64) -> *mut ArrayHeader {
    unsafe {
        let length = (*arr).length;
        let capacity = (*arr).capacity;

        let arr = if length >= capacity {
            js_array_grow(arr, length + 1)
        } else {
            arr
        };

        let elements_ptr = (arr as *mut u8).add(std::mem::size_of::<ArrayHeader>()) as *mut f64;

        // Shift all elements up
        ptr::copy(elements_ptr, elements_ptr.add(1), length as usize);
        // Write new element at beginning
        ptr::write(elements_ptr, value);
        (*arr).length = length + 1;
        arr
    }
}

/// Find the index of an element in an array
/// Returns -1 if not found
#[no_mangle]
pub extern "C" fn js_array_indexOf_f64(arr: *const ArrayHeader, value: f64) -> i32 {
    unsafe {
        let length = (*arr).length;
        let elements_ptr = (arr as *const u8).add(std::mem::size_of::<ArrayHeader>()) as *const f64;

        for i in 0..length as usize {
            if *elements_ptr.add(i) == value {
                return i as i32;
            }
        }
        -1
    }
}

/// Check if an array includes a value
/// Returns 1 if found, 0 if not
#[no_mangle]
pub extern "C" fn js_array_includes_f64(arr: *const ArrayHeader, value: f64) -> i32 {
    if js_array_indexOf_f64(arr, value) >= 0 { 1 } else { 0 }
}

/// Check if an array includes a value using deep equality comparison.
/// This handles NaN-boxed strings by comparing string contents.
/// Returns 1 if found, 0 if not.
#[no_mangle]
pub extern "C" fn js_array_includes_jsvalue(arr: *const ArrayHeader, value: f64) -> i32 {
    unsafe {
        let length = (*arr).length;
        let elements_ptr = (arr as *const u8).add(std::mem::size_of::<ArrayHeader>()) as *const f64;

        for i in 0..length as usize {
            let element = *elements_ptr.add(i);
            if crate::value::js_jsvalue_equals(element, value) == 1 {
                return 1;
            }
        }
        0
    }
}

/// Splice an array - removes elements and optionally inserts new ones
/// start: starting index (can be negative for from-end)
/// delete_count: number of elements to delete
/// items: pointer to elements to insert (can be null if no items)
/// items_count: number of elements to insert
/// Returns a new array containing the deleted elements
/// ALSO modifies arr in place, returns the modified array pointer (may have reallocated)
/// The return is packed: lower 48 bits = deleted array ptr, we return via out param
#[no_mangle]
pub extern "C" fn js_array_splice(
    arr: *mut ArrayHeader,
    start: i32,
    delete_count: i32,
    items: *const f64,
    items_count: u32,
    out_arr: *mut *mut ArrayHeader,
) -> *mut ArrayHeader {
    unsafe {
        let len = (*arr).length as i32;

        // Normalize start index
        let start_idx = if start < 0 {
            (len + start).max(0) as u32
        } else {
            (start as u32).min(len as u32)
        };

        // Normalize delete count
        let actual_delete = if delete_count < 0 {
            0
        } else {
            (delete_count as u32).min(len as u32 - start_idx)
        };

        // Create array of deleted elements
        let deleted = js_array_alloc(actual_delete);
        (*deleted).length = actual_delete;

        let elements_ptr = (arr as *mut u8).add(std::mem::size_of::<ArrayHeader>()) as *mut f64;
        let deleted_elements = (deleted as *mut u8).add(std::mem::size_of::<ArrayHeader>()) as *mut f64;

        // Copy deleted elements to return array
        for i in 0..actual_delete as usize {
            ptr::write(deleted_elements.add(i), *elements_ptr.add(start_idx as usize + i));
        }

        // Calculate new length
        let new_len = (len as u32 - actual_delete + items_count) as u32;

        // Grow array if needed
        let arr = if new_len > (*arr).capacity {
            js_array_grow(arr, new_len)
        } else {
            arr
        };
        let elements_ptr = (arr as *mut u8).add(std::mem::size_of::<ArrayHeader>()) as *mut f64;

        // Shift elements after the splice point
        let tail_start = start_idx + actual_delete;
        let tail_len = len as u32 - tail_start;

        if items_count != actual_delete && tail_len > 0 {
            // Need to shift the tail
            let src = elements_ptr.add(tail_start as usize);
            let dst = elements_ptr.add((start_idx + items_count) as usize);
            ptr::copy(src, dst, tail_len as usize);
        }

        // Insert new items
        if items_count > 0 && !items.is_null() {
            for i in 0..items_count as usize {
                ptr::write(elements_ptr.add(start_idx as usize + i), *items.add(i));
            }
        }

        (*arr).length = new_len;

        // Return modified array via out param
        *out_arr = arr;

        deleted
    }
}

/// Slice an array, returning a new array with elements from start to end (exclusive)
/// Handles negative indices (from end of array)
/// If end is i32::MAX, slices to end of array
#[no_mangle]
pub extern "C" fn js_array_slice(arr: *const ArrayHeader, start: i32, end: i32) -> *mut ArrayHeader {
    unsafe {
        let len = (*arr).length as i32;

        // Normalize start index
        let start_idx = if start < 0 {
            (len + start).max(0) as u32
        } else {
            (start as u32).min(len as u32)
        };

        // Normalize end index
        let end_idx = if end == i32::MAX {
            len as u32
        } else if end < 0 {
            (len + end).max(0) as u32
        } else {
            (end as u32).min(len as u32)
        };

        // Calculate slice length
        let slice_len = if end_idx > start_idx { end_idx - start_idx } else { 0 };

        // Allocate new array
        let result = js_array_alloc(slice_len);
        (*result).length = slice_len;

        // Copy elements
        let src_elements = (arr as *const u8).add(std::mem::size_of::<ArrayHeader>()) as *const f64;
        let dst_elements = (result as *mut u8).add(std::mem::size_of::<ArrayHeader>()) as *mut f64;

        for i in 0..slice_len as usize {
            ptr::write(dst_elements.add(i), ptr::read(src_elements.add(start_idx as usize + i)));
        }

        result
    }
}

// ============================================================================
// JSValue-based array functions (for stdlib convenience)
// These store JSValue bits as f64 for uniform storage
// ============================================================================

use crate::value::JSValue;

/// Set an element using JSValue
#[no_mangle]
pub extern "C" fn js_array_set(arr: *mut ArrayHeader, index: u32, value: JSValue) {
    // Convert JSValue bits to f64 for storage
    let bits_as_f64 = f64::from_bits(value.bits());
    js_array_set_f64(arr, index, bits_as_f64);
}

/// Get an element as JSValue
#[no_mangle]
pub extern "C" fn js_array_get(arr: *const ArrayHeader, index: u32) -> JSValue {
    let bits_as_f64 = js_array_get_f64(arr, index);
    JSValue::from_bits(bits_as_f64.to_bits())
}

/// Push a JSValue to the array
#[no_mangle]
pub extern "C" fn js_array_push(arr: *mut ArrayHeader, value: JSValue) -> *mut ArrayHeader {
    let bits_as_f64 = f64::from_bits(value.bits());
    js_array_push_f64(arr, bits_as_f64)
}

/// Allocate and initialize an array from a list of JSValue (stored as u64 bits)
/// This is used for mixed-type arrays where elements can be numbers, strings, objects, etc.
#[no_mangle]
pub extern "C" fn js_array_from_jsvalue(elements: *const u64, count: u32) -> *mut ArrayHeader {
    let arr = js_array_alloc(count);
    unsafe {
        (*arr).length = count;
        let arr_elements = (arr as *mut u8).add(std::mem::size_of::<ArrayHeader>()) as *mut f64;
        // Each u64 contains NaN-boxed JSValue bits, store as f64 bits
        for i in 0..count as usize {
            let bits = *elements.add(i);
            ptr::write(arr_elements.add(i), f64::from_bits(bits));
        }
    }
    arr
}

/// Get an element from a mixed-type array (returns raw u64 bits for JSValue)
#[no_mangle]
pub extern "C" fn js_array_get_jsvalue(arr: *const ArrayHeader, index: u32) -> u64 {
    let bits_as_f64 = js_array_get_f64(arr, index);
    bits_as_f64.to_bits()
}

/// Set an element in a mixed-type array (value is raw u64 bits for JSValue)
#[no_mangle]
pub extern "C" fn js_array_set_jsvalue(arr: *mut ArrayHeader, index: u32, value: u64) {
    let bits_as_f64 = f64::from_bits(value);
    js_array_set_f64(arr, index, bits_as_f64);
}

/// Push a JSValue (as u64 bits) to a mixed-type array
#[no_mangle]
pub extern "C" fn js_array_push_jsvalue(arr: *mut ArrayHeader, value: u64) -> *mut ArrayHeader {
    let bits_as_f64 = f64::from_bits(value);
    js_array_push_f64(arr, bits_as_f64)
}

/// Append all elements from source array to destination array
/// Returns the (possibly reallocated) destination array pointer
#[no_mangle]
pub extern "C" fn js_array_concat(dest: *mut ArrayHeader, src: *const ArrayHeader) -> *mut ArrayHeader {
    unsafe {
        let src_len = (*src).length;
        if src_len == 0 {
            return dest;
        }

        let src_elements = (src as *const u8).add(std::mem::size_of::<ArrayHeader>()) as *const f64;
        let mut result = dest;

        for i in 0..src_len as usize {
            let element = *src_elements.add(i);
            result = js_array_push_f64(result, element);
        }

        result
    }
}

// ============================================================================
// Array higher-order function methods
// These use closure pointers to call the callback function
// ============================================================================

use crate::closure::{ClosureHeader, js_closure_call1, js_closure_call2};

/// forEach - call callback(element, index) for each element
/// Returns nothing (void)
#[no_mangle]
pub extern "C" fn js_array_forEach(arr: *const ArrayHeader, callback: *const ClosureHeader) {
    unsafe {
        let length = (*arr).length;
        let elements_ptr = (arr as *const u8).add(std::mem::size_of::<ArrayHeader>()) as *const f64;

        for i in 0..length as usize {
            let element = *elements_ptr.add(i);
            // Call callback(element) - we pass just the element for simplicity
            // Full JS forEach also passes index and array, but we start simple
            js_closure_call1(callback, element);
        }
    }
}

/// map - create new array by calling callback(element) on each element
/// Returns pointer to new array
#[no_mangle]
pub extern "C" fn js_array_map(arr: *const ArrayHeader, callback: *const ClosureHeader) -> *mut ArrayHeader {
    unsafe {
        let length = (*arr).length;
        let elements_ptr = (arr as *const u8).add(std::mem::size_of::<ArrayHeader>()) as *const f64;

        // Allocate result array with same capacity
        let result = js_array_alloc(length);
        let result_elements = (result as *mut u8).add(std::mem::size_of::<ArrayHeader>()) as *mut f64;

        for i in 0..length as usize {
            let element = *elements_ptr.add(i);
            let mapped = js_closure_call1(callback, element);
            ptr::write(result_elements.add(i), mapped);
        }
        (*result).length = length;

        result
    }
}

/// filter - create new array with elements where callback(element) returns truthy
/// Returns pointer to new array
#[no_mangle]
pub extern "C" fn js_array_filter(arr: *const ArrayHeader, callback: *const ClosureHeader) -> *mut ArrayHeader {
    unsafe {
        let length = (*arr).length;
        let elements_ptr = (arr as *const u8).add(std::mem::size_of::<ArrayHeader>()) as *const f64;

        // Allocate result array with same capacity (might be smaller)
        let result = js_array_alloc(length);
        let mut result_len = 0u32;

        for i in 0..length as usize {
            let element = *elements_ptr.add(i);
            let keep = js_closure_call1(callback, element);
            // Truthy check: non-zero value
            if keep != 0.0 {
                let result = js_array_push_f64(result, element);
                result_len += 1;
            }
        }

        result
    }
}

/// find - find first element that matches callback(element) => true
/// Returns the element as f64, or f64::NAN (undefined) if not found
#[no_mangle]
pub extern "C" fn js_array_find(arr: *const ArrayHeader, callback: *const ClosureHeader) -> f64 {
    unsafe {
        let length = (*arr).length;
        let elements_ptr = (arr as *const u8).add(std::mem::size_of::<ArrayHeader>()) as *const f64;

        for i in 0..length as usize {
            let element = *elements_ptr.add(i);
            let result = js_closure_call1(callback, element);
            // Truthy check: non-zero value
            if result != 0.0 {
                return element;
            }
        }

        // Not found - return undefined (NaN)
        f64::NAN
    }
}

/// findIndex - find index of first element that matches callback(element) => true
/// Returns the index as i32, or -1 if not found
#[no_mangle]
pub extern "C" fn js_array_findIndex(arr: *const ArrayHeader, callback: *const ClosureHeader) -> i32 {
    unsafe {
        let length = (*arr).length;
        let elements_ptr = (arr as *const u8).add(std::mem::size_of::<ArrayHeader>()) as *const f64;

        for i in 0..length as usize {
            let element = *elements_ptr.add(i);
            let result = js_closure_call1(callback, element);
            // Truthy check: non-zero value
            if result != 0.0 {
                return i as i32;
            }
        }

        // Not found
        -1
    }
}

/// reduce - accumulate values using callback(accumulator, element)
/// initial_ptr is pointer to f64 initial value (null if not provided)
/// Returns the final accumulated value
#[no_mangle]
pub extern "C" fn js_array_reduce(
    arr: *const ArrayHeader,
    callback: *const ClosureHeader,
    has_initial: i32,
    initial: f64,
) -> f64 {
    unsafe {
        let length = (*arr).length;
        let elements_ptr = (arr as *const u8).add(std::mem::size_of::<ArrayHeader>()) as *const f64;

        if length == 0 {
            if has_initial != 0 {
                return initial;
            } else {
                // TypeError in JS, but we return NaN for simplicity
                return f64::NAN;
            }
        }

        let (mut accumulator, start_idx) = if has_initial != 0 {
            (initial, 0)
        } else {
            // Use first element as initial
            (*elements_ptr, 1)
        };

        for i in start_idx..length as usize {
            let element = *elements_ptr.add(i);
            accumulator = js_closure_call2(callback, accumulator, element);
        }

        accumulator
    }
}

/// join - Join array elements into a string with a separator
/// Returns pointer to new StringHeader
#[no_mangle]
pub extern "C" fn js_array_join(arr: *const ArrayHeader, separator: *const crate::string::StringHeader) -> *mut crate::string::StringHeader {
    use crate::string::{StringHeader, js_string_from_bytes};
    use crate::value::JSValue;

    unsafe {
        let length = (*arr).length;

        // Empty array returns empty string
        if length == 0 {
            return js_string_from_bytes(ptr::null(), 0);
        }

        let elements_ptr = (arr as *const u8).add(std::mem::size_of::<ArrayHeader>()) as *const f64;

        // Get separator string
        let sep_str = if separator.is_null() {
            ","
        } else {
            let sep_len = (*separator).length as usize;
            let sep_data = (separator as *const u8).add(std::mem::size_of::<StringHeader>());
            std::str::from_utf8_unchecked(std::slice::from_raw_parts(sep_data, sep_len))
        };

        // Build result string
        let mut result = String::new();
        for i in 0..length as usize {
            if i > 0 {
                result.push_str(sep_str);
            }
            let element_bits = (*elements_ptr.add(i)).to_bits();
            let jsvalue = JSValue::from_bits(element_bits);

            // Convert element to string based on its type
            if jsvalue.is_string() {
                let str_ptr = jsvalue.as_pointer() as *const StringHeader;
                let str_len = (*str_ptr).length as usize;
                let str_data = (str_ptr as *const u8).add(std::mem::size_of::<StringHeader>());
                let s = std::str::from_utf8_unchecked(std::slice::from_raw_parts(str_data, str_len));
                result.push_str(s);
            } else if jsvalue.is_number() {
                let n = jsvalue.as_number();
                if n.fract() == 0.0 && n.abs() < 1e15 {
                    result.push_str(&format!("{}", n as i64));
                } else {
                    result.push_str(&format!("{}", n));
                }
            } else if jsvalue.is_null() {
                // null stringifies to empty string in join
            } else if jsvalue.is_undefined() {
                // undefined stringifies to empty string in join
            } else if jsvalue.is_bool() {
                result.push_str(if jsvalue.as_bool() { "true" } else { "false" });
            } else {
                // For objects/arrays, just use placeholder
                result.push_str("[object Object]");
            }
        }

        // Create result string
        js_string_from_bytes(result.as_ptr(), result.len() as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_array_alloc_and_access() {
        let arr = js_array_alloc(5);

        // Initially empty
        assert_eq!(js_array_length(arr), 0);

        // Push some values
        js_array_push_f64(arr, 1.0);
        js_array_push_f64(arr, 2.0);
        js_array_push_f64(arr, 3.0);

        assert_eq!(js_array_length(arr), 3);
        assert_eq!(js_array_get_f64(arr, 0), 1.0);
        assert_eq!(js_array_get_f64(arr, 1), 2.0);
        assert_eq!(js_array_get_f64(arr, 2), 3.0);

        // Out of bounds
        assert!(js_array_get_f64(arr, 5).is_nan());
    }

    #[test]
    fn test_array_from_f64() {
        let values = [10.0, 20.0, 30.0, 40.0, 50.0];
        let arr = js_array_from_f64(values.as_ptr(), 5);

        assert_eq!(js_array_length(arr), 5);
        assert_eq!(js_array_get_f64(arr, 0), 10.0);
        assert_eq!(js_array_get_f64(arr, 2), 30.0);
        assert_eq!(js_array_get_f64(arr, 4), 50.0);
    }

    #[test]
    fn test_array_set() {
        let arr = js_array_alloc(3);
        js_array_push_f64(arr, 1.0);
        js_array_push_f64(arr, 2.0);
        js_array_push_f64(arr, 3.0);

        js_array_set_f64(arr, 1, 99.0);
        assert_eq!(js_array_get_f64(arr, 1), 99.0);
    }
}
