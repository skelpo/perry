//! Map representation for Perry
//!
//! Maps are heap-allocated with a stable header pointer.
//! The entries array is separately allocated and can be reallocated
//! without changing the MapHeader address.

use std::alloc::{alloc, dealloc, realloc, Layout};
use std::ptr;
use crate::string::StringHeader;

/// Map header - stable address, entries allocated separately
#[repr(C)]
pub struct MapHeader {
    /// Number of key-value pairs in the map
    pub size: u32,
    /// Capacity (allocated space for entries)
    pub capacity: u32,
    /// Pointer to entries array (separately allocated)
    pub entries: *mut f64,
}

/// Each map entry is 16 bytes (key + value, both as f64/JSValue)
const ENTRY_SIZE: usize = 16;

/// Calculate the layout for an entries array with N entries capacity
fn entries_layout(capacity: usize) -> Layout {
    let entries_size = capacity * ENTRY_SIZE;
    Layout::from_size_align(entries_size.max(8), 8).unwrap()
}

/// Get pointer to entries array
unsafe fn entries_ptr(map: *const MapHeader) -> *const f64 {
    (*map).entries as *const f64
}

/// Get mutable pointer to entries array
unsafe fn entries_ptr_mut(map: *mut MapHeader) -> *mut f64 {
    (*map).entries
}

/// Check if a value looks like a heap pointer (raw pointer stored in f64)
/// On most systems, heap pointers have small upper bits (0x0000 or close to it)
fn looks_like_pointer(val: f64) -> bool {
    let bits = val.to_bits();
    // Heap pointers on modern systems typically have upper 16 bits as 0x0000
    // and lower 48 bits as the actual address. Addresses above 0x100000000 are typical.
    let upper_16 = bits >> 48;
    let lower_48 = bits & 0x0000_FFFF_FFFF_FFFF;
    // Check if upper bits are 0 (user-space pointer) and lower bits look like a valid address
    upper_16 == 0 && lower_48 > 0x10000
}

/// Extract pointer from raw f64 (for non-NaN-boxed pointers)
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
    // Compare content byte by byte
    let data_a = (a as *const u8).add(std::mem::size_of::<StringHeader>());
    let data_b = (b as *const u8).add(std::mem::size_of::<StringHeader>());
    for i in 0..len_a as usize {
        if *data_a.add(i) != *data_b.add(i) {
            return false;
        }
    }
    true
}

/// Check if two JSValues are equal (for map key comparison)
/// This handles both NaN-boxed values and raw pointers, including string content comparison
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
        // Try string comparison - if both are strings, compare content
        // Note: We can't easily distinguish string pointers from other pointers,
        // so we assume map keys that are pointers are strings
        unsafe { strings_equal(ptr_a, ptr_b) }
    } else {
        false
    }
}

/// Allocate a new empty map with the given initial capacity
#[no_mangle]
pub extern "C" fn js_map_alloc(capacity: u32) -> *mut MapHeader {
    let cap = if capacity == 0 { 4 } else { capacity };
    let header_layout = Layout::new::<MapHeader>();
    let ent_layout = entries_layout(cap as usize);
    unsafe {
        let ptr = alloc(header_layout) as *mut MapHeader;
        if ptr.is_null() {
            panic!("Failed to allocate map header");
        }
        let entries = alloc(ent_layout) as *mut f64;
        if entries.is_null() {
            panic!("Failed to allocate map entries");
        }

        // Initialize header
        (*ptr).size = 0;
        (*ptr).capacity = cap;
        (*ptr).entries = entries;

        ptr
    }
}

/// Get the number of entries in the map
#[no_mangle]
pub extern "C" fn js_map_size(map: *const MapHeader) -> u32 {
    unsafe { (*map).size }
}

/// Find the index of a key in the map, or -1 if not found
unsafe fn find_key_index(map: *const MapHeader, key: f64) -> i32 {
    let size = (*map).size;
    let entries = entries_ptr(map);

    for i in 0..size {
        let entry_key = ptr::read(entries.add((i as usize) * 2));
        if jsvalue_eq(entry_key, key) {
            return i as i32;
        }
    }

    -1
}

/// Grow the entries array if needed (header stays at same address)
unsafe fn ensure_capacity(map: *mut MapHeader) {
    let size = (*map).size;
    let capacity = (*map).capacity;

    if size < capacity {
        return;
    }

    // Double the capacity
    let new_capacity = capacity * 2;
    let old_layout = entries_layout(capacity as usize);
    let new_layout = entries_layout(new_capacity as usize);

    let new_entries = realloc((*map).entries as *mut u8, old_layout, new_layout.size()) as *mut f64;
    if new_entries.is_null() {
        panic!("Failed to grow map entries");
    }

    (*map).entries = new_entries;
    (*map).capacity = new_capacity;
}

/// Set a key-value pair in the map
/// The map pointer is stable (never reallocated)
#[no_mangle]
pub extern "C" fn js_map_set(map: *mut MapHeader, key: f64, value: f64) -> *mut MapHeader {
    unsafe {
        // Check if key already exists
        let idx = find_key_index(map, key);

        if idx >= 0 {
            // Update existing value
            let entries = entries_ptr_mut(map);
            ptr::write(entries.add((idx as usize) * 2 + 1), value);
            return map;
        }

        // Key doesn't exist, need to add new entry
        ensure_capacity(map);
        let size = (*map).size;
        let entries = entries_ptr_mut(map);

        // Write key and value
        ptr::write(entries.add((size as usize) * 2), key);
        ptr::write(entries.add((size as usize) * 2 + 1), value);

        (*map).size = size + 1;
        map
    }
}

/// Get a value from the map by key
/// Returns the value, or NaN (representing undefined) if not found
#[no_mangle]
pub extern "C" fn js_map_get(map: *const MapHeader, key: f64) -> f64 {
    unsafe {
        let idx = find_key_index(map, key);

        if idx >= 0 {
            let entries = entries_ptr(map);
            return ptr::read(entries.add((idx as usize) * 2 + 1));
        }

        // Return undefined (represented as a specific NaN pattern)
        // Using 0x7FF8_0000_0000_0001 as undefined marker
        f64::from_bits(0x7FF8_0000_0000_0001)
    }
}

/// Check if the map has a key
/// Returns 1 if found, 0 if not found
#[no_mangle]
pub extern "C" fn js_map_has(map: *const MapHeader, key: f64) -> i32 {
    unsafe {
        if find_key_index(map, key) >= 0 { 1 } else { 0 }
    }
}

/// Delete a key from the map
/// Returns 1 if deleted, 0 if key not found
#[no_mangle]
pub extern "C" fn js_map_delete(map: *mut MapHeader, key: f64) -> i32 {
    unsafe {
        let idx = find_key_index(map, key);

        if idx < 0 {
            return 0;
        }

        let size = (*map).size;
        let entries = entries_ptr_mut(map);

        // If not the last element, swap with the last element
        if (idx as u32) < size - 1 {
            let last_key = ptr::read(entries.add(((size - 1) as usize) * 2));
            let last_value = ptr::read(entries.add(((size - 1) as usize) * 2 + 1));
            ptr::write(entries.add((idx as usize) * 2), last_key);
            ptr::write(entries.add((idx as usize) * 2 + 1), last_value);
        }

        (*map).size = size - 1;
        1
    }
}

/// Clear all entries from the map
#[no_mangle]
pub extern "C" fn js_map_clear(map: *mut MapHeader) {
    unsafe {
        (*map).size = 0;
    }
}
