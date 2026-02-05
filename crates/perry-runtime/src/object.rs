//! Object representation for Perry
//!
//! Objects are heap-allocated with a header containing:
//! - Class ID (for type checking and vtable lookup)
//! - Field count
//! - Keys array pointer (for Object.keys() support)
//! - Fields array (inline)

use crate::JSValue;
use crate::ArrayHeader;
use crate::arena::arena_alloc;
use std::alloc::{alloc, dealloc, Layout};
use std::ptr;
use std::collections::HashMap;
use std::sync::RwLock;

/// Global class registry mapping class_id -> parent_class_id for inheritance chain lookups
static CLASS_REGISTRY: RwLock<Option<HashMap<u32, u32>>> = RwLock::new(None);

/// Function pointer type for dispatching method calls on handle-based objects.
/// Handle-based objects use small integer IDs (1, 2, 3...) instead of real heap pointers.
/// This is registered by perry-stdlib to dispatch to Fastify, ioredis, etc.
type HandleMethodDispatchFn = unsafe extern "C" fn(
    handle: i64,
    method_name_ptr: *const u8,
    method_name_len: usize,
    args_ptr: *const f64,
    args_len: usize,
) -> f64;

static mut HANDLE_METHOD_DISPATCH: Option<HandleMethodDispatchFn> = None;

/// Register a function to handle method calls on handle-based objects
#[no_mangle]
pub unsafe extern "C" fn js_register_handle_method_dispatch(f: HandleMethodDispatchFn) {
    HANDLE_METHOD_DISPATCH = Some(f);
}

/// Register a class with its parent class ID in the global registry
fn register_class(class_id: u32, parent_class_id: u32) {
    let mut registry = CLASS_REGISTRY.write().unwrap();
    if registry.is_none() {
        *registry = Some(HashMap::new());
    }
    registry.as_mut().unwrap().insert(class_id, parent_class_id);
}

/// Look up parent class ID from the registry
fn get_parent_class_id(class_id: u32) -> Option<u32> {
    let registry = CLASS_REGISTRY.read().unwrap();
    registry.as_ref().and_then(|r| r.get(&class_id).copied())
}

/// Object header - precedes the fields in memory
#[repr(C)]
pub struct ObjectHeader {
    /// Type tag to distinguish from Error objects (must be first field!)
    /// Uses OBJECT_TYPE_REGULAR (1) for regular objects
    pub object_type: u32,
    /// Class ID for this object (used for instanceof, vtable lookup)
    pub class_id: u32,
    /// Parent class ID for inheritance chain (0 if no parent)
    pub parent_class_id: u32,
    /// Number of fields in this object
    pub field_count: u32,
    /// Pointer to array of key strings (for Object.keys() support)
    /// NULL for class instances (keys are defined by the class)
    pub keys_array: *mut ArrayHeader,
}

/// Calculate the layout for an object with N fields
fn object_layout(field_count: usize) -> Layout {
    let header_size = std::mem::size_of::<ObjectHeader>();
    let fields_size = field_count * std::mem::size_of::<JSValue>();
    let total_size = header_size + fields_size;
    Layout::from_size_align(total_size, 8).unwrap()
}

/// Allocate a new object with the given class ID and field count
/// Returns a pointer to the object header
#[no_mangle]
pub extern "C" fn js_object_alloc(class_id: u32, field_count: u32) -> *mut ObjectHeader {
    js_object_alloc_with_parent(class_id, 0, field_count)
}

/// Allocate a new object with class ID, parent class ID, and field count
/// The parent_class_id is used for instanceof inheritance checks
/// Returns a pointer to the object header
#[no_mangle]
pub extern "C" fn js_object_alloc_with_parent(class_id: u32, parent_class_id: u32, field_count: u32) -> *mut ObjectHeader {
    // Register this class's parent for inheritance lookups
    if parent_class_id != 0 {
        register_class(class_id, parent_class_id);
    }

    let layout = object_layout(field_count as usize);
    unsafe {
        let ptr = alloc(layout) as *mut ObjectHeader;
        if ptr.is_null() {
            panic!("Failed to allocate object");
        }

        // Initialize header
        (*ptr).object_type = crate::error::OBJECT_TYPE_REGULAR;
        (*ptr).class_id = class_id;
        (*ptr).parent_class_id = parent_class_id;
        (*ptr).field_count = field_count;
        (*ptr).keys_array = ptr::null_mut();

        // Initialize all fields to undefined
        let fields_ptr = (ptr as *mut u8).add(std::mem::size_of::<ObjectHeader>()) as *mut JSValue;
        for i in 0..field_count as usize {
            ptr::write(fields_ptr.add(i), JSValue::undefined());
        }

        ptr
    }
}

/// Fast object allocation using bump allocator - NO field initialization
/// This is significantly faster for hot paths where constructor immediately sets all fields
/// Returns a pointer to the object header with UNINITIALIZED fields
#[no_mangle]
pub extern "C" fn js_object_alloc_fast(class_id: u32, field_count: u32) -> *mut ObjectHeader {
    let header_size = std::mem::size_of::<ObjectHeader>();
    let fields_size = (field_count as usize) * std::mem::size_of::<JSValue>();
    let total_size = header_size + fields_size;

    let ptr = arena_alloc(total_size, 8) as *mut ObjectHeader;

    unsafe {
        // Initialize header only - fields left uninitialized for constructor to fill
        (*ptr).object_type = crate::error::OBJECT_TYPE_REGULAR;
        (*ptr).class_id = class_id;
        (*ptr).parent_class_id = 0;
        (*ptr).field_count = field_count;
        (*ptr).keys_array = ptr::null_mut();
    }

    ptr
}

/// Fast object allocation with parent class ID - NO field initialization
#[no_mangle]
pub extern "C" fn js_object_alloc_fast_with_parent(class_id: u32, parent_class_id: u32, field_count: u32) -> *mut ObjectHeader {
    // Only register class if it has a parent (one-time operation per class)
    if parent_class_id != 0 {
        register_class(class_id, parent_class_id);
    }

    let header_size = std::mem::size_of::<ObjectHeader>();
    let fields_size = (field_count as usize) * std::mem::size_of::<JSValue>();
    let total_size = header_size + fields_size;

    let ptr = arena_alloc(total_size, 8) as *mut ObjectHeader;

    unsafe {
        (*ptr).object_type = crate::error::OBJECT_TYPE_REGULAR;
        (*ptr).class_id = class_id;
        (*ptr).parent_class_id = parent_class_id;
        (*ptr).field_count = field_count;
        (*ptr).keys_array = ptr::null_mut();
    }

    ptr
}

/// Get a field from an object by index
#[no_mangle]
pub extern "C" fn js_object_get_field(obj: *const ObjectHeader, field_index: u32) -> JSValue {
    unsafe {
        let fields_ptr = (obj as *const u8).add(std::mem::size_of::<ObjectHeader>()) as *const JSValue;
        *fields_ptr.add(field_index as usize)
    }
}

/// Set a field on an object by index
#[no_mangle]
pub extern "C" fn js_object_set_field(obj: *mut ObjectHeader, field_index: u32, value: JSValue) {
    unsafe {
        let fields_ptr = (obj as *mut u8).add(std::mem::size_of::<ObjectHeader>()) as *mut JSValue;
        ptr::write(fields_ptr.add(field_index as usize), value);
    }
}

/// Get the class ID of an object
#[no_mangle]
pub extern "C" fn js_object_get_class_id(obj: *const ObjectHeader) -> u32 {
    unsafe { (*obj).class_id }
}

/// Free an object (for manual memory management / testing)
#[no_mangle]
pub extern "C" fn js_object_free(obj: *mut ObjectHeader) {
    unsafe {
        let field_count = (*obj).field_count as usize;
        let layout = object_layout(field_count);
        dealloc(obj as *mut u8, layout);
    }
}

/// Convert an object pointer to a JSValue
#[no_mangle]
pub extern "C" fn js_object_to_value(obj: *const ObjectHeader) -> JSValue {
    JSValue::pointer(obj as *const u8)
}

/// Extract an object pointer from a JSValue
#[no_mangle]
pub extern "C" fn js_value_to_object(value: JSValue) -> *mut ObjectHeader {
    value.as_pointer::<ObjectHeader>() as *mut ObjectHeader
}

/// Get a field as f64 (returns raw JSValue bits as f64)
/// This preserves NaN-boxing for strings and other pointer types
#[no_mangle]
pub extern "C" fn js_object_get_field_f64(obj: *const ObjectHeader, field_index: u32) -> f64 {
    let value = js_object_get_field(obj, field_index);
    f64::from_bits(value.bits())
}

/// Set a field from f64 (interprets raw bits as JSValue)
/// This preserves NaN-boxing for strings and other pointer types
#[no_mangle]
pub extern "C" fn js_object_set_field_f64(obj: *mut ObjectHeader, field_index: u32, value: f64) {
    js_object_set_field(obj, field_index, JSValue::from_bits(value.to_bits()));
}

/// Set a field by index with a raw f64 value (for dynamic object creation)
/// This is a convenience wrapper that takes field_index as u32 and value as f64
#[no_mangle]
pub extern "C" fn js_object_set_field_by_index(obj: *mut ObjectHeader, _key: *const crate::string::StringHeader, field_index: u32, value: f64) {
    js_object_set_field(obj, field_index, JSValue::from_bits(value.to_bits()));
}

/// Set the keys array for an object (used for Object.keys() support)
/// The keys_array should be an array of string pointers
#[no_mangle]
pub extern "C" fn js_object_set_keys(obj: *mut ObjectHeader, keys_array: *mut ArrayHeader) {
    unsafe {
        (*obj).keys_array = keys_array;
    }
}

/// Get the keys of an object as an array of strings
/// Returns the stored keys array, or an empty array if no keys were stored
#[no_mangle]
pub extern "C" fn js_object_keys(obj: *const ObjectHeader) -> *mut ArrayHeader {
    unsafe {
        let keys = (*obj).keys_array;
        if keys.is_null() {
            // Return an empty array if no keys are stored
            crate::array::js_array_alloc(0)
        } else {
            keys
        }
    }
}

/// Get the values of an object as an array
/// Returns an array of the object's field values
#[no_mangle]
pub extern "C" fn js_object_values(obj: *const ObjectHeader) -> *mut ArrayHeader {
    unsafe {
        let field_count = (*obj).field_count as usize;
        let result = crate::array::js_array_alloc(field_count as u32);

        for i in 0..field_count {
            let value = js_object_get_field(obj as *mut ObjectHeader, i as u32);
            // Store the raw f64 bits (which may be NaN-boxed)
            crate::array::js_array_push_f64(result, f64::from_bits(value.bits()));
        }

        result
    }
}

/// Get the entries of an object as an array of [key, value] pairs
/// Returns an array where each element is a 2-element array [key, value]
#[no_mangle]
pub extern "C" fn js_object_entries(obj: *const ObjectHeader) -> *mut ArrayHeader {
    unsafe {
        let keys = (*obj).keys_array;
        let field_count = (*obj).field_count as usize;
        let result = crate::array::js_array_alloc(field_count as u32);

        for i in 0..field_count {
            // Create a pair array [key, value]
            let pair = crate::array::js_array_alloc(2);

            // Get the key (from keys array if available)
            if !keys.is_null() && (i as u32) < crate::array::js_array_length(keys) {
                let key = crate::array::js_array_get_f64(keys, i as u32);
                crate::array::js_array_push_f64(pair, key);
            } else {
                // No key available, use empty string
                crate::array::js_array_push_f64(pair, 0.0);
            }

            // Get the value
            let value = js_object_get_field(obj as *mut ObjectHeader, i as u32);
            crate::array::js_array_push_f64(pair, f64::from_bits(value.bits()));

            // Push the pair to result (NaN-box the array pointer)
            let pair_boxed = crate::value::js_nanbox_pointer(pair as i64);
            crate::array::js_array_push_f64(result, pair_boxed);
        }

        result
    }
}

/// Check if a property exists in an object by its string key name
/// Returns 1.0 if the property exists, 0.0 otherwise
/// This implements the JavaScript 'in' operator: "key" in obj
#[no_mangle]
pub extern "C" fn js_object_has_property(obj: f64, key: f64) -> f64 {
    let obj_val = JSValue::from_bits(obj.to_bits());
    let key_val = JSValue::from_bits(key.to_bits());

    // The object must be a pointer (object, array, etc.)
    if !obj_val.is_pointer() {
        return 0.0;
    }

    let obj_ptr = obj_val.as_pointer::<ObjectHeader>();
    if obj_ptr.is_null() {
        return 0.0;
    }

    // The key should be a string
    if !key_val.is_string() {
        // If key is a number, convert to string for lookup
        // For now, we only support string keys
        return 0.0;
    }

    let key_str = key_val.as_pointer::<crate::StringHeader>();

    unsafe {
        let keys = (*obj_ptr).keys_array;
        if keys.is_null() {
            return 0.0;
        }

        // Search through the keys array for a match
        let key_count = crate::array::js_array_length(keys) as usize;
        for i in 0..key_count {
            let stored_key_val = crate::array::js_array_get(keys, i as u32);
            // Keys are stored as string pointers (NaN-boxed)
            if stored_key_val.is_string() {
                let stored_key = stored_key_val.as_pointer::<crate::StringHeader>();
                if crate::string::js_string_equals(key_str, stored_key) {
                    // Found the property
                    return 1.0;
                }
            }
        }

        // Key not found
        0.0
    }
}

/// Get a field by its string key name
/// Returns the field value or undefined if the key is not found
#[no_mangle]
pub extern "C" fn js_object_get_field_by_name(obj: *const ObjectHeader, key: *const crate::StringHeader) -> JSValue {
    if obj.is_null() {
        return JSValue::undefined();
    }
    unsafe {
        let keys = (*obj).keys_array;
        if keys.is_null() {
            return JSValue::undefined();
        }

        // Search through the keys array for a match
        let key_count = crate::array::js_array_length(keys) as usize;
        for i in 0..key_count {
            let key_val = crate::array::js_array_get(keys, i as u32);
            // Keys are stored as string pointers (NaN-boxed)
            if key_val.is_string() {
                let stored_key = key_val.as_pointer::<crate::StringHeader>();
                if crate::string::js_string_equals(key, stored_key) {
                    // Found it - return the field at this index
                    return js_object_get_field(obj, i as u32);
                }
            }
        }

        // Key not found
        JSValue::undefined()
    }
}

/// Get a field by its string key name, returned as f64 (raw JSValue bits)
/// This preserves the NaN-boxing for strings and other pointer types
#[no_mangle]
pub extern "C" fn js_object_get_field_by_name_f64(obj: *const ObjectHeader, key: *const crate::StringHeader) -> f64 {
    let value = js_object_get_field_by_name(obj, key);
    f64::from_bits(value.bits())
}

/// Set a field value by its string key name (dynamic property access)
/// This searches the keys array for a match and sets the corresponding value.
/// If the key doesn't exist, it adds it to the object.
#[no_mangle]
pub extern "C" fn js_object_set_field_by_name(obj: *mut ObjectHeader, key: *const crate::StringHeader, value: f64) {
    if obj.is_null() {
        return;
    }
    unsafe {
        let keys = (*obj).keys_array;

        // If no keys array exists, create one
        if keys.is_null() {
            // Create a new keys array with the key
            let new_keys = crate::array::js_array_alloc(4);
            crate::array::js_array_push(new_keys, JSValue::string_ptr(key as *mut _));
            (*obj).keys_array = new_keys;

            // Reallocate fields to hold at least one value
            // Note: We assume the object has enough field slots pre-allocated
            js_object_set_field(obj, 0, JSValue::from_bits(value.to_bits()));
            return;
        }

        // Search through the keys array for a match
        let key_count = crate::array::js_array_length(keys) as usize;
        for i in 0..key_count {
            let key_val = crate::array::js_array_get(keys, i as u32);
            // Keys are stored as string pointers (NaN-boxed)
            if key_val.is_string() {
                let stored_key = key_val.as_pointer::<crate::StringHeader>();
                if crate::string::js_string_equals(key, stored_key) {
                    // Found it - update the field
                    js_object_set_field(obj, i as u32, JSValue::from_bits(value.to_bits()));
                    return;
                }
            }
        }

        // Key not found - add it to the object
        // First, add the key to the keys array
        crate::array::js_array_push(keys, JSValue::string_ptr(key as *mut _));

        // Set the field at the new index
        let new_index = key_count as u32;
        js_object_set_field(obj, new_index, JSValue::from_bits(value.to_bits()));
    }
}

/// Delete a field from an object by its string key name
/// Returns 1 if the field was deleted (or didn't exist), 0 otherwise
/// Note: In strict mode, this would return 0 for non-configurable properties,
/// but we don't track configurability, so we always return 1.
#[no_mangle]
pub extern "C" fn js_object_delete_field(obj: *mut ObjectHeader, key: *const crate::StringHeader) -> i32 {
    unsafe {
        let keys = (*obj).keys_array;
        if keys.is_null() {
            // No keys array means no fields to delete, but delete "succeeds" vacuously
            return 1;
        }

        // Search through the keys array for a match
        let key_count = crate::array::js_array_length(keys) as usize;
        for i in 0..key_count {
            let key_val = crate::array::js_array_get(keys, i as u32);
            // Keys are stored as string pointers (NaN-boxed)
            if key_val.is_string() {
                let stored_key = key_val.as_pointer::<crate::StringHeader>();
                if crate::string::js_string_equals(key, stored_key) {
                    // Found it - set the field to undefined
                    js_object_set_field(obj, i as u32, JSValue::undefined());
                    return 1;
                }
            }
        }

        // Key not found - delete still "succeeds" for non-existent properties
        1
    }
}

/// Delete a field from an object using a dynamic key (could be string or number index)
/// For arrays, this sets the element to undefined
/// Returns 1 if successful, 0 otherwise
#[no_mangle]
pub extern "C" fn js_object_delete_dynamic(obj: *mut ObjectHeader, key: f64) -> i32 {
    let key_val = JSValue::from_bits(key.to_bits());

    // If the key is a string, use js_object_delete_field
    if key_val.is_string() {
        let key_str = key_val.as_pointer::<crate::StringHeader>();
        return js_object_delete_field(obj, key_str);
    }

    // If the key is a number, treat as array index
    if key_val.is_number() {
        let index = key_val.as_number() as usize;
        // Try to treat it as an array and set the element to undefined
        // This is a simplified implementation - real JS delete on arrays
        // creates a hole (sparse array), but we just set to undefined
        let arr = obj as *mut crate::array::ArrayHeader;
        let len = crate::array::js_array_length(arr) as usize;
        if index < len {
            crate::array::js_array_set(arr, index as u32, JSValue::undefined());
            return 1;
        }
    }

    // For other types, delete succeeds vacuously
    1
}

/// Check if a value is an instance of a class with the given class_id
/// Walks the inheritance chain to check parent classes
/// Returns 1.0 for true, 0.0 for false
#[no_mangle]
pub extern "C" fn js_instanceof(value: f64, class_id: u32) -> f64 {
    let jsval = crate::JSValue::from_bits(value.to_bits());

    // Only objects (pointers) can be instances of classes
    if !jsval.is_pointer() {
        return 0.0;
    }

    // Get the object pointer
    let obj_ptr = jsval.as_pointer::<ObjectHeader>();
    if obj_ptr.is_null() {
        return 0.0;
    }

    unsafe {
        // Check if the object's class_id matches directly
        let obj_class_id = (*obj_ptr).class_id;
        if obj_class_id == class_id {
            return 1.0;
        }

        // Walk up the inheritance chain using the class registry
        let mut current_class = obj_class_id;
        while let Some(parent_id) = get_parent_class_id(current_class) {
            if parent_id == 0 {
                break;
            }
            if parent_id == class_id {
                return 1.0;
            }
            current_class = parent_id;
        }

        0.0
    }
}

/// Call a method on an object with dynamic dispatch
/// This is used for runtime method calls when the method cannot be resolved statically.
/// object: NaN-boxed f64 containing an object pointer
/// method_name_ptr: pointer to the method name string (raw bytes, not StringHeader)
/// method_name_len: length of the method name
/// args_ptr: pointer to array of f64 arguments
/// args_len: number of arguments
/// Returns the result as f64
///
/// NOTE: This function is named js_native_call_method to avoid symbol collision
/// with js_call_method in perry-jsruntime which handles V8 JavaScript values.
#[no_mangle]
pub unsafe extern "C" fn js_native_call_method(
    object: f64,
    method_name_ptr: *const i8,
    method_name_len: usize,
    args_ptr: *const f64,
    args_len: usize,
) -> f64 {
    // Get the method name
    let method_name = if method_name_ptr.is_null() || method_name_len == 0 {
        ""
    } else {
        let bytes = std::slice::from_raw_parts(method_name_ptr as *const u8, method_name_len);
        std::str::from_utf8(bytes).unwrap_or("")
    };

    let jsval = JSValue::from_bits(object.to_bits());

    // Check if this is a handle-based object (small integer, not a real heap pointer)
    // Handles are used by Fastify, ioredis, and other native modules that store
    // objects in a registry and use integer IDs to reference them.
    if jsval.is_pointer() {
        let raw_ptr = jsval.as_pointer::<u8>() as usize;
        if raw_ptr > 0 && raw_ptr < 0x100000 {
            // This is a handle, not a real memory pointer - dispatch to stdlib
            if let Some(dispatch) = HANDLE_METHOD_DISPATCH {
                return dispatch(
                    raw_ptr as i64,
                    method_name.as_ptr(),
                    method_name.len(),
                    args_ptr,
                    args_len,
                );
            }
            // No dispatcher registered, return undefined
            return f64::from_bits(0x7FF8_0000_0000_0001);
        }
    }

    // Handle common method calls
    match method_name {
        // Function.prototype.bind - returns the same function for native closures
        // This is a simplification - real bind() creates a new function with bound 'this'
        "bind" => {
            // For native closures, we return the function as-is
            // The 'this' binding is handled at the call site
            return object;
        }

        // Common string methods on string values
        "toString" => {
            if jsval.is_string() {
                return object;
            } else if jsval.is_number() {
                let n = jsval.as_number();
                let s = if n.fract() == 0.0 && n.abs() < (i64::MAX as f64) {
                    (n as i64).to_string()
                } else {
                    n.to_string()
                };
                let str_ptr = crate::string::js_string_from_bytes(s.as_ptr(), s.len() as u32);
                return JSValue::string_ptr(str_ptr).bits() as f64;
            } else if jsval.is_bool() {
                let s = if jsval.as_bool() { "true" } else { "false" };
                let str_ptr = crate::string::js_string_from_bytes(s.as_ptr(), s.len() as u32);
                return JSValue::string_ptr(str_ptr).bits() as f64;
            } else if jsval.is_undefined() {
                let s = "undefined";
                let str_ptr = crate::string::js_string_from_bytes(s.as_ptr(), s.len() as u32);
                return JSValue::string_ptr(str_ptr).bits() as f64;
            } else if jsval.is_null() {
                let s = "null";
                let str_ptr = crate::string::js_string_from_bytes(s.as_ptr(), s.len() as u32);
                return JSValue::string_ptr(str_ptr).bits() as f64;
            }
        }

        // Array methods - delegate to array runtime
        "push" if jsval.is_pointer() => {
            let arr = jsval.as_pointer::<crate::array::ArrayHeader>() as *mut crate::array::ArrayHeader;
            if args_len > 0 && !args_ptr.is_null() {
                let val = *args_ptr;
                crate::array::js_array_push_f64(arr, val);
            }
            return crate::array::js_array_length(arr) as f64;
        }
        "pop" if jsval.is_pointer() => {
            let arr = jsval.as_pointer::<crate::array::ArrayHeader>() as *mut crate::array::ArrayHeader;
            return crate::array::js_array_pop_f64(arr);
        }
        "length" if jsval.is_pointer() => {
            let arr = jsval.as_pointer::<crate::array::ArrayHeader>();
            return crate::array::js_array_length(arr) as f64;
        }

        _ => {}
    }

    // If it's an object with a method stored as a closure in a field,
    // try to find and call it
    if jsval.is_pointer() {
        let obj = jsval.as_pointer::<ObjectHeader>();
        let keys = (*obj).keys_array;

        if !keys.is_null() {
            // Search for the method in the object's fields
            let key_count = crate::array::js_array_length(keys) as usize;
            let method_key = crate::string::js_string_from_bytes(
                method_name.as_ptr(),
                method_name.len() as u32,
            );

            for i in 0..key_count {
                let key_val = crate::array::js_array_get(keys, i as u32);
                if key_val.is_string() {
                    let stored_key = key_val.as_pointer::<crate::StringHeader>();
                    if crate::string::js_string_equals(method_key, stored_key) {
                        // Found the method - get it and call it if it's a closure
                        let field_val = js_object_get_field(obj as *mut _, i as u32);
                        if field_val.is_pointer() {
                            // Assume it's a closure and call it
                            let closure = field_val.as_pointer::<crate::closure::ClosureHeader>();
                            return crate::closure::js_native_call_value(
                                f64::from_bits(field_val.bits()),
                                args_ptr,
                                args_len,
                            );
                        }
                    }
                }
            }
        }
    }

    // Method not found - return undefined
    JSValue::undefined().bits() as f64
}

/// Special class ID for native module namespace objects
/// This is used to identify objects that represent native module namespaces
pub const NATIVE_MODULE_CLASS_ID: u32 = 0xFFFFFFFE;

/// Create a native module namespace object
/// This is used for `import * as X from 'module'` patterns
/// The returned object identifies itself as an object (typeof returns "object")
/// and stores the module name for debugging purposes
///
/// module_name_ptr: pointer to the module name string bytes
/// module_name_len: length of the module name
/// Returns the object as a NaN-boxed f64
#[no_mangle]
pub extern "C" fn js_create_native_module_namespace(module_name_ptr: *const u8, module_name_len: usize) -> f64 {
    // Create an object with one field to store the module name
    let obj = js_object_alloc(NATIVE_MODULE_CLASS_ID, 1);

    unsafe {
        // Create a string from the module name
        let module_name = crate::string::js_string_from_bytes(module_name_ptr, module_name_len as u32);

        // Store the module name in the first field
        js_object_set_field(obj, 0, JSValue::string_ptr(module_name));

        // Create a keys array with one key: "__module__"
        let keys_array = crate::array::js_array_alloc(1);
        let key_bytes = b"__module__";
        let key_str = crate::string::js_string_from_bytes(key_bytes.as_ptr(), key_bytes.len() as u32);
        crate::array::js_array_push(keys_array, JSValue::string_ptr(key_str));
        js_object_set_keys(obj, keys_array);
    }

    // Return as NaN-boxed pointer
    crate::value::js_nanbox_pointer(obj as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_object_alloc_and_fields() {
        let obj = js_object_alloc(1, 3);

        // Check header
        assert_eq!(js_object_get_class_id(obj), 1);

        // Fields should be undefined initially
        let f0 = js_object_get_field(obj, 0);
        assert!(f0.is_undefined());

        // Set and get a field
        js_object_set_field(obj, 0, JSValue::number(42.0));
        let f0 = js_object_get_field(obj, 0);
        assert!(f0.is_number());
        assert_eq!(f0.as_number(), 42.0);

        // Set another field
        js_object_set_field(obj, 2, JSValue::bool(true));
        let f2 = js_object_get_field(obj, 2);
        assert!(f2.is_bool());
        assert!(f2.as_bool());

        // Clean up
        js_object_free(obj);
    }

    #[test]
    fn test_object_to_value_roundtrip() {
        let obj = js_object_alloc(5, 2);
        js_object_set_field(obj, 0, JSValue::number(123.0));

        let value = js_object_to_value(obj);
        assert!(value.is_pointer());

        let obj2 = js_value_to_object(value);
        assert_eq!(js_object_get_class_id(obj2), 5);

        let f0 = js_object_get_field(obj2, 0);
        assert_eq!(f0.as_number(), 123.0);

        js_object_free(obj);
    }
}
