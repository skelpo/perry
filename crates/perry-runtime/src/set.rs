//! Set representation for Perry
//!
//! Sets are heap-allocated with a header containing:
//! - Size (number of elements)
//! - Capacity
//! - Elements array (inline) - each element is f64/JSValue

use std::alloc::{alloc, realloc, Layout};
use std::ptr;
use crate::string::StringHeader;

/// Set header - precedes the elements in memory
#[repr(C)]
pub struct SetHeader {
    /// Number of elements in the set
    pub size: u32,
    /// Capacity (allocated space for elements)
    pub capacity: u32,
}

/// Each set element is 8 bytes (f64/JSValue)
const ELEMENT_SIZE: usize = 8;

/// Calculate the layout for a set with N elements capacity
fn set_layout(capacity: usize) -> Layout {
    let header_size = std::mem::size_of::<SetHeader>();
    let elements_size = capacity * ELEMENT_SIZE;
    let total_size = header_size + elements_size;
    Layout::from_size_align(total_size, 8).unwrap()
}

/// Get pointer to elements array
unsafe fn elements_ptr(set: *const SetHeader) -> *const f64 {
    (set as *const u8).add(std::mem::size_of::<SetHeader>()) as *const f64
}

/// Get mutable pointer to elements array
unsafe fn elements_ptr_mut(set: *mut SetHeader) -> *mut f64 {
    (set as *mut u8).add(std::mem::size_of::<SetHeader>()) as *mut f64
}

/// Check if a value looks like a heap pointer (raw pointer stored in f64)
fn looks_like_pointer(val: f64) -> bool {
    let bits = val.to_bits();
    let upper_16 = bits >> 48;
    let lower_48 = bits & 0x0000_FFFF_FFFF_FFFF;
    upper_16 == 0 && lower_48 > 0x10000
}

/// Extract pointer from raw f64
fn as_raw_pointer(val: f64) -> *const u8 {
    val.to_bits() as *const u8
}

/// Compare two strings by content
unsafe fn strings_equal(a: *const StringHeader, b: *const StringHeader) -> bool {
    if a.is_null() || b.is_null() {
        return a == b;
    }
    let len_a = (*a).length;
    let len_b = (*b).length;
    if len_a != len_b {
        return false;
    }
    let data_a = (a as *const u8).add(std::mem::size_of::<StringHeader>());
    let data_b = (b as *const u8).add(std::mem::size_of::<StringHeader>());
    for i in 0..len_a as usize {
        if *data_a.add(i) != *data_b.add(i) {
            return false;
        }
    }
    true
}

/// Check if two JSValues are equal (for set element comparison)
fn jsvalue_eq(a: f64, b: f64) -> bool {
    let a_bits = a.to_bits();
    let b_bits = b.to_bits();

    // Fast path: identical bit patterns
    if a_bits == b_bits {
        return true;
    }

    // Both look like raw pointers - they might be strings with same content
    if looks_like_pointer(a) && looks_like_pointer(b) {
        let ptr_a = as_raw_pointer(a) as *const StringHeader;
        let ptr_b = as_raw_pointer(b) as *const StringHeader;
        unsafe { strings_equal(ptr_a, ptr_b) }
    } else {
        false
    }
}

/// Find the index of a value in the set, or -1 if not found
unsafe fn find_value_index(set: *const SetHeader, value: f64) -> i32 {
    let size = (*set).size;
    let elements = elements_ptr(set);

    for i in 0..size {
        let element = ptr::read(elements.add(i as usize));
        if jsvalue_eq(element, value) {
            return i as i32;
        }
    }

    -1
}

/// Grow the set if needed
unsafe fn ensure_capacity(set: *mut SetHeader) -> *mut SetHeader {
    let size = (*set).size;
    let capacity = (*set).capacity;

    if size < capacity {
        return set;
    }

    // Double the capacity
    let new_capacity = capacity * 2;
    let old_layout = set_layout(capacity as usize);
    let new_layout = set_layout(new_capacity as usize);

    let new_ptr = realloc(set as *mut u8, old_layout, new_layout.size()) as *mut SetHeader;
    if new_ptr.is_null() {
        panic!("Failed to grow set");
    }

    (*new_ptr).capacity = new_capacity;
    new_ptr
}

/// Allocate a new empty set with the given initial capacity
#[no_mangle]
pub extern "C" fn js_set_alloc(capacity: u32) -> *mut SetHeader {
    let cap = if capacity == 0 { 4 } else { capacity };
    let layout = set_layout(cap as usize);
    unsafe {
        let ptr = alloc(layout) as *mut SetHeader;
        if ptr.is_null() {
            panic!("Failed to allocate set");
        }

        // Initialize header
        (*ptr).size = 0;
        (*ptr).capacity = cap;

        ptr
    }
}

/// Get the number of elements in the set
#[no_mangle]
pub extern "C" fn js_set_size(set: *const SetHeader) -> u32 {
    unsafe { (*set).size }
}

/// Add a value to the set
/// Returns the (possibly reallocated) set pointer
#[no_mangle]
pub extern "C" fn js_set_add(set: *mut SetHeader, value: f64) -> *mut SetHeader {
    unsafe {
        // Check if value already exists
        let idx = find_value_index(set, value);

        if idx >= 0 {
            // Value already exists, nothing to do
            return set;
        }

        // Value doesn't exist, need to add it
        let set = ensure_capacity(set);
        let size = (*set).size;
        let elements = elements_ptr_mut(set);

        // Write the value
        ptr::write(elements.add(size as usize), value);

        (*set).size = size + 1;
        set
    }
}

/// Check if the set has a value
/// Returns 1 if found, 0 if not found
#[no_mangle]
pub extern "C" fn js_set_has(set: *const SetHeader, value: f64) -> i32 {
    unsafe {
        if find_value_index(set, value) >= 0 { 1 } else { 0 }
    }
}

/// Delete a value from the set
/// Returns 1 if deleted, 0 if value not found
#[no_mangle]
pub extern "C" fn js_set_delete(set: *mut SetHeader, value: f64) -> i32 {
    unsafe {
        let idx = find_value_index(set, value);

        if idx < 0 {
            return 0;
        }

        let size = (*set).size;
        let elements = elements_ptr_mut(set);

        // If not the last element, swap with the last element
        if (idx as u32) < size - 1 {
            let last_value = ptr::read(elements.add((size - 1) as usize));
            ptr::write(elements.add(idx as usize), last_value);
        }

        (*set).size = size - 1;
        1
    }
}

/// Clear all elements from the set
#[no_mangle]
pub extern "C" fn js_set_clear(set: *mut SetHeader) {
    unsafe {
        (*set).size = 0;
    }
}
