//! Set representation for Perry
//!
//! Sets are heap-allocated with a stable header pointer.
//! The elements array is separately allocated and can be reallocated
//! without changing the SetHeader address.

use std::alloc::{alloc, realloc, Layout};
use std::ptr;
use crate::string::StringHeader;

/// Set header - stable address, elements allocated separately
#[repr(C)]
pub struct SetHeader {
    /// Number of elements in the set
    pub size: u32,
    /// Capacity (allocated space for elements)
    pub capacity: u32,
    /// Pointer to elements array (separately allocated)
    pub elements: *mut f64,
}

/// Each set element is 8 bytes (f64/JSValue)
const ELEMENT_SIZE: usize = 8;

/// Calculate the layout for an elements array with N elements capacity
fn elements_layout(capacity: usize) -> Layout {
    let elements_size = capacity * ELEMENT_SIZE;
    Layout::from_size_align(elements_size.max(8), 8).unwrap()
}

/// Get pointer to elements array
unsafe fn elements_ptr(set: *const SetHeader) -> *const f64 {
    (*set).elements as *const f64
}

/// Get mutable pointer to elements array
unsafe fn elements_ptr_mut(set: *mut SetHeader) -> *mut f64 {
    (*set).elements
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

/// Grow the elements array if needed (header stays at same address)
unsafe fn ensure_capacity(set: *mut SetHeader) {
    let size = (*set).size;
    let capacity = (*set).capacity;

    if size < capacity {
        return;
    }

    // Double the capacity
    let new_capacity = capacity * 2;
    let old_layout = elements_layout(capacity as usize);
    let new_layout = elements_layout(new_capacity as usize);

    let new_elements = realloc((*set).elements as *mut u8, old_layout, new_layout.size()) as *mut f64;
    if new_elements.is_null() {
        panic!("Failed to grow set elements");
    }

    (*set).elements = new_elements;
    (*set).capacity = new_capacity;
}

/// Allocate a new empty set with the given initial capacity
#[no_mangle]
pub extern "C" fn js_set_alloc(capacity: u32) -> *mut SetHeader {
    let cap = if capacity == 0 { 4 } else { capacity };
    let header_layout = Layout::new::<SetHeader>();
    let elem_layout = elements_layout(cap as usize);
    unsafe {
        let ptr = alloc(header_layout) as *mut SetHeader;
        if ptr.is_null() {
            panic!("Failed to allocate set header");
        }
        let elements = alloc(elem_layout) as *mut f64;
        if elements.is_null() {
            panic!("Failed to allocate set elements");
        }

        // Initialize header
        (*ptr).size = 0;
        (*ptr).capacity = cap;
        (*ptr).elements = elements;

        ptr
    }
}

/// Get the number of elements in the set
#[no_mangle]
pub extern "C" fn js_set_size(set: *const SetHeader) -> u32 {
    unsafe { (*set).size }
}

/// Add a value to the set
/// Returns the set pointer (always the same, stable address)
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
        ensure_capacity(set);
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
