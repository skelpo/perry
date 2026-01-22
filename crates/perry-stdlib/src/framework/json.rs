//! JSON handling
//!
//! Provides JSON.parse() and JSON.stringify() functionality.

use perry_runtime::{
    js_array_alloc, js_array_push, js_object_alloc, js_object_set_field,
    js_string_from_bytes, JSValue, StringHeader,
};

/// Helper to extract string from StringHeader pointer
unsafe fn string_from_header(ptr: *const StringHeader) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    let len = (*ptr).length as usize;
    let data_ptr = (ptr as *const u8).add(std::mem::size_of::<StringHeader>());
    let bytes = std::slice::from_raw_parts(data_ptr, len);
    Some(String::from_utf8_lossy(bytes).to_string())
}

/// Convert serde_json::Value to JSValue
unsafe fn json_value_to_jsvalue(value: &serde_json::Value) -> JSValue {
    match value {
        serde_json::Value::Null => JSValue::null(),
        serde_json::Value::Bool(b) => JSValue::bool(*b),
        serde_json::Value::Number(n) => {
            // Always use f64 for compatibility - the codegen treats all numbers as f64
            if let Some(f) = n.as_f64() {
                JSValue::number(f)
            } else if let Some(i) = n.as_i64() {
                JSValue::number(i as f64)
            } else {
                JSValue::number(0.0)
            }
        }
        serde_json::Value::String(s) => {
            let ptr = js_string_from_bytes(s.as_ptr(), s.len() as u32);
            JSValue::string_ptr(ptr)
        }
        serde_json::Value::Array(arr) => {
            let js_arr = js_array_alloc(arr.len() as u32);
            for item in arr {
                js_array_push(js_arr, json_value_to_jsvalue(item));
            }
            JSValue::object_ptr(js_arr as *mut u8)
        }
        serde_json::Value::Object(obj) => {
            let js_obj = js_object_alloc(0, obj.len() as u32);
            for (idx, (_key, value)) in obj.iter().enumerate() {
                js_object_set_field(js_obj, idx as u32, json_value_to_jsvalue(value));
            }
            JSValue::object_ptr(js_obj as *mut u8)
        }
    }
}

/// JSON.parse(text) -> any
///
/// Parse a JSON string into a JavaScript value.
#[no_mangle]
pub unsafe extern "C" fn js_json_parse(text_ptr: *const StringHeader) -> JSValue {
    let text = match string_from_header(text_ptr) {
        Some(t) => t,
        None => return JSValue::null(),
    };

    match serde_json::from_str::<serde_json::Value>(&text) {
        Ok(value) => json_value_to_jsvalue(&value),
        Err(_) => JSValue::null(), // Return null on parse error
    }
}

/// JSON.stringify(value) -> string
///
/// Convert a JavaScript value to a JSON string.
/// Note: This is a simplified version that only handles primitives and strings.
/// For complex objects, the TypeScript compiler should generate the serialization.
#[no_mangle]
pub unsafe extern "C" fn js_json_stringify_string(
    str_ptr: *const StringHeader,
) -> *mut StringHeader {
    let s = match string_from_header(str_ptr) {
        Some(s) => s,
        None => return std::ptr::null_mut(),
    };

    // Escape the string and wrap in quotes
    let escaped = serde_json::to_string(&s).unwrap_or_else(|_| "null".to_string());
    js_string_from_bytes(escaped.as_ptr(), escaped.len() as u32)
}

/// Stringify a number
#[no_mangle]
pub unsafe extern "C" fn js_json_stringify_number(value: f64) -> *mut StringHeader {
    let s = if value.is_nan() {
        "null".to_string()
    } else if value.is_infinite() {
        "null".to_string()
    } else if value.fract() == 0.0 && value.abs() < (i64::MAX as f64) {
        // Integer
        format!("{}", value as i64)
    } else {
        // Float
        format!("{}", value)
    };

    js_string_from_bytes(s.as_ptr(), s.len() as u32)
}

/// Stringify a boolean
#[no_mangle]
pub unsafe extern "C" fn js_json_stringify_bool(value: bool) -> *mut StringHeader {
    let s = if value { "true" } else { "false" };
    js_string_from_bytes(s.as_ptr(), s.len() as u32)
}

/// Stringify null
#[no_mangle]
pub unsafe extern "C" fn js_json_stringify_null() -> *mut StringHeader {
    let s = "null";
    js_string_from_bytes(s.as_ptr(), s.len() as u32)
}

/// NaN-boxing constants (same as in value.rs)
const TAG_NULL: u64 = 0x7FFC_0000_0000_0002;
const TAG_FALSE: u64 = 0x7FFC_0000_0000_0003;
const TAG_TRUE: u64 = 0x7FFC_0000_0000_0004;
const POINTER_TAG: u64 = 0x7FFD_0000_0000_0000;
const STRING_TAG: u64 = 0x7FFF_0000_0000_0000;
const POINTER_MASK: u64 = 0x0000_FFFF_FFFF_FFFF;

/// Check if a f64 value might be a raw bitcast pointer (not NaN-boxed).
/// Raw pointers, when bitcast to f64, appear as subnormal positive numbers
/// because heap addresses typically only use the lower 48 bits.
fn is_raw_pointer(bits: u64) -> bool {
    // Check if it could be a raw pointer:
    // - Not a special NaN value
    // - Not negative
    // - Exponent is 0 (subnormal or zero)
    // - Mantissa is non-zero (non-zero pointer)
    let exponent = (bits >> 52) & 0x7FF;
    let mantissa = bits & 0x000F_FFFF_FFFF_FFFF;
    let sign = bits >> 63;
    exponent == 0 && mantissa != 0 && sign == 0
}

/// Get pointer from raw bitcast value
fn raw_pointer_value(bits: u64) -> *const u8 {
    bits as *const u8
}

/// Generic JSON.stringify that handles any JSValue
/// Takes a f64 (NaN-boxed JSValue) and returns a string pointer
#[no_mangle]
pub unsafe extern "C" fn js_json_stringify(value: f64) -> *mut StringHeader {
    let bits: u64 = value.to_bits();

    // Check special values
    if bits == TAG_NULL {
        return js_json_stringify_null();
    }
    if bits == TAG_TRUE {
        return js_json_stringify_bool(true);
    }
    if bits == TAG_FALSE {
        return js_json_stringify_bool(false);
    }

    // Check if it's a pointer (array, object, or string)
    // Handle both NaN-boxed pointers and raw bitcast pointers (from variables)
    let is_nanboxed_ptr = (bits & 0xFFFF_0000_0000_0000) == POINTER_TAG;
    let is_raw_ptr = is_raw_pointer(bits);

    if is_nanboxed_ptr || is_raw_ptr {
        let ptr = if is_nanboxed_ptr {
            (bits & POINTER_MASK) as *const u8
        } else {
            raw_pointer_value(bits)
        };

        // First try to interpret as array (most common case)
        // This avoids false positive object detection when array elements look like keys_array
        let arr = ptr as *const perry_runtime::ArrayHeader;
        let arr_len = (*arr).length;
        let arr_cap = (*arr).capacity;

        // Check if this looks like a valid array header
        // Arrays have: length <= capacity, reasonable values, capacity > 0
        let looks_like_array = arr_len <= arr_cap && arr_cap > 0 && arr_cap < 10000;

        // For arrays, also check that the "keys_array" offset (which is first element)
        // doesn't look like a valid keys array pointer
        // A real keys array would have string pointers, not object pointers
        let obj = ptr as *const perry_runtime::ObjectHeader;
        let has_valid_keys = if !obj.is_null() && !(*obj).keys_array.is_null() {
            let keys_arr = (*obj).keys_array;
            let keys_len = (*keys_arr).length;
            let keys_cap = (*keys_arr).capacity;
            let field_count = (*obj).field_count;

            // Valid keys array should have:
            // - length <= capacity
            // - length > 0 (objects have at least one key if keys_array is set)
            // - field_count == keys_len (number of fields equals number of keys)
            keys_len <= keys_cap && keys_len > 0 && keys_cap < 1000 && field_count == keys_len
        } else {
            false
        };

        // If it looks like both array and object, prefer array if keys validation fails
        // If it has valid keys that match field_count, it's definitely an object
        if has_valid_keys {
            // This looks like an object (has valid keys)
            let num_fields = (*obj).field_count;
            let mut result = String::from("{");

            // Get the keys array for field names
            let keys_arr = (*obj).keys_array;
            let keys_len = (*keys_arr).length;
            let keys_elements = (keys_arr as *const u8)
                .add(std::mem::size_of::<perry_runtime::ArrayHeader>()) as *const f64;

            for f in 0..num_fields {
                if f > 0 {
                    result.push(',');
                }

                // Get field name from keys array
                if (f as u32) < keys_len {
                    let key_f64 = *keys_elements.add(f as usize);
                    let key_bits = key_f64.to_bits();
                    // Keys are NaN-boxed strings (STRING_TAG = 0x7FFF)
                    let key_tag = key_bits & 0xFFFF_0000_0000_0000;
                    let key_ptr = if key_tag == STRING_TAG || key_tag == POINTER_TAG {
                        (key_bits & POINTER_MASK) as *const StringHeader
                    } else {
                        key_bits as *const StringHeader
                    };
                    if let Some(key_str) = string_from_header(key_ptr) {
                        result.push('"');
                        result.push_str(&key_str);
                        result.push_str("\":");
                    } else {
                        result.push_str(&format!("\"field{}\":", f));
                    }
                } else {
                    result.push_str(&format!("\"field{}\":", f));
                }

                // Get field value
                let fields_ptr = (ptr as *const u8)
                    .add(std::mem::size_of::<perry_runtime::ObjectHeader>()) as *const f64;
                let field_val = *fields_ptr.add(f as usize);
                let field_bits = field_val.to_bits();

                // Stringify the field value
                let field_tag = field_bits & 0xFFFF_0000_0000_0000;
                if field_bits == TAG_NULL {
                    result.push_str("null");
                } else if field_bits == TAG_TRUE {
                    result.push_str("true");
                } else if field_bits == TAG_FALSE {
                    result.push_str("false");
                } else if field_tag == STRING_TAG || field_tag == POINTER_TAG || is_raw_pointer(field_bits) {
                    // String or object field (could be NaN-boxed or raw)
                    let str_ptr = if field_tag == STRING_TAG || field_tag == POINTER_TAG {
                        (field_bits & POINTER_MASK) as *const StringHeader
                    } else {
                        field_bits as *const StringHeader
                    };
                    if let Some(s) = string_from_header(str_ptr) {
                        let escaped = serde_json::to_string(&s).unwrap_or_else(|_| "null".to_string());
                        result.push_str(&escaped);
                    } else {
                        result.push_str("null");
                    }
                } else {
                    // Number
                    if field_val.is_nan() {
                        result.push_str("null");
                    } else if field_val.fract() == 0.0 && field_val.abs() < (i64::MAX as f64) {
                        result.push_str(&format!("{}", field_val as i64));
                    } else {
                        result.push_str(&format!("{}", field_val));
                    }
                }
            }
            result.push('}');
            return js_string_from_bytes(result.as_ptr(), result.len() as u32);
        }

        // Try to interpret as array and stringify
        let arr = ptr as *const perry_runtime::ArrayHeader;
        if !arr.is_null() {
            let len = (*arr).length;
            let elements = (ptr as *const u8).add(std::mem::size_of::<perry_runtime::ArrayHeader>()) as *const f64;

            let mut result = String::from("[");
            for i in 0..len {
                if i > 0 {
                    result.push(',');
                }
                let elem = *elements.add(i as usize);
                let elem_bits = elem.to_bits();

                // Recursively stringify each element
                // Check for NaN-boxed pointer (object or string) OR raw bitcast pointer
                let elem_tag = elem_bits & 0xFFFF_0000_0000_0000;
                let is_nanboxed_ptr = elem_tag == POINTER_TAG || elem_tag == STRING_TAG;
                let is_raw_ptr = is_raw_pointer(elem_bits);

                if is_nanboxed_ptr || is_raw_ptr {
                    // It's a pointer - could be an object, string, or nested array
                    let elem_ptr = if is_nanboxed_ptr {
                        (elem_bits & POINTER_MASK) as *const u8
                    } else {
                        raw_pointer_value(elem_bits)
                    };

                    // Try to interpret as an object (simplified - assume it has known fields)
                    let obj = elem_ptr as *const perry_runtime::ObjectHeader;
                    if !obj.is_null() {
                        let num_fields = (*obj).field_count;
                        result.push('{');
                        for f in 0..num_fields {
                            if f > 0 {
                                result.push(',');
                            }
                            // Get field value
                            let fields_ptr = (elem_ptr as *const u8)
                                .add(std::mem::size_of::<perry_runtime::ObjectHeader>()) as *const f64;
                            let field_val = *fields_ptr.add(f as usize);

                            // We need field names - for now just use index
                            result.push_str(&format!("\"field{}\":", f));

                            // Stringify the field value
                            let field_bits = field_val.to_bits();
                            if field_bits == TAG_NULL {
                                result.push_str("null");
                            } else if field_bits == TAG_TRUE {
                                result.push_str("true");
                            } else if field_bits == TAG_FALSE {
                                result.push_str("false");
                            } else {
                                let field_tag = field_bits & 0xFFFF_0000_0000_0000;
                                if field_tag == STRING_TAG || field_tag == POINTER_TAG || is_raw_pointer(field_bits) {
                                    // String or object field (could be NaN-boxed or raw)
                                    let str_ptr = if field_tag == STRING_TAG || field_tag == POINTER_TAG {
                                        (field_bits & POINTER_MASK) as *const StringHeader
                                    } else {
                                        field_bits as *const StringHeader
                                    };
                                    if let Some(s) = string_from_header(str_ptr) {
                                        let escaped = serde_json::to_string(&s).unwrap_or_else(|_| "null".to_string());
                                        result.push_str(&escaped);
                                    } else {
                                        result.push_str("null");
                                    }
                                } else {
                                    // Number
                                    if field_val.is_nan() {
                                        result.push_str("null");
                                    } else if field_val.fract() == 0.0 && field_val.abs() < (i64::MAX as f64) {
                                        result.push_str(&format!("{}", field_val as i64));
                                    } else {
                                        result.push_str(&format!("{}", field_val));
                                    }
                                }
                            }
                        }
                        result.push('}');
                    }
                } else {
                    // It's a number
                    if elem.is_nan() {
                        result.push_str("null");
                    } else if elem.fract() == 0.0 && elem.abs() < (i64::MAX as f64) {
                        result.push_str(&format!("{}", elem as i64));
                    } else {
                        result.push_str(&format!("{}", elem));
                    }
                }
            }
            result.push(']');

            return js_string_from_bytes(result.as_ptr(), result.len() as u32);
        }

        // Try as string
        let str_ptr = ptr as *const StringHeader;
        if let Some(s) = string_from_header(str_ptr) {
            let escaped = serde_json::to_string(&s).unwrap_or_else(|_| "null".to_string());
            return js_string_from_bytes(escaped.as_ptr(), escaped.len() as u32);
        }
    }

    // It's a regular number
    js_json_stringify_number(value)
}

/// Check if a string is valid JSON
#[no_mangle]
pub unsafe extern "C" fn js_json_is_valid(text_ptr: *const StringHeader) -> bool {
    let text = match string_from_header(text_ptr) {
        Some(t) => t,
        None => return false,
    };

    serde_json::from_str::<serde_json::Value>(&text).is_ok()
}

/// Get a value from parsed JSON by key (for object access)
/// The json_ptr should be the result of js_json_parse
#[no_mangle]
pub unsafe extern "C" fn js_json_get_string(
    json_ptr: *const StringHeader,
    key_ptr: *const StringHeader,
) -> *mut StringHeader {
    let json_str = match string_from_header(json_ptr) {
        Some(j) => j,
        None => return std::ptr::null_mut(),
    };

    let key = match string_from_header(key_ptr) {
        Some(k) => k,
        None => return std::ptr::null_mut(),
    };

    match serde_json::from_str::<serde_json::Value>(&json_str) {
        Ok(serde_json::Value::Object(obj)) => {
            if let Some(serde_json::Value::String(s)) = obj.get(&key) {
                return js_string_from_bytes(s.as_ptr(), s.len() as u32);
            }
        }
        _ => {}
    }

    std::ptr::null_mut()
}

/// Get a number from parsed JSON by key
#[no_mangle]
pub unsafe extern "C" fn js_json_get_number(
    json_ptr: *const StringHeader,
    key_ptr: *const StringHeader,
) -> f64 {
    let json_str = match string_from_header(json_ptr) {
        Some(j) => j,
        None => return f64::NAN,
    };

    let key = match string_from_header(key_ptr) {
        Some(k) => k,
        None => return f64::NAN,
    };

    match serde_json::from_str::<serde_json::Value>(&json_str) {
        Ok(serde_json::Value::Object(obj)) => {
            if let Some(serde_json::Value::Number(n)) = obj.get(&key) {
                return n.as_f64().unwrap_or(f64::NAN);
            }
        }
        _ => {}
    }

    f64::NAN
}

/// Get a boolean from parsed JSON by key
#[no_mangle]
pub unsafe extern "C" fn js_json_get_bool(
    json_ptr: *const StringHeader,
    key_ptr: *const StringHeader,
) -> bool {
    let json_str = match string_from_header(json_ptr) {
        Some(j) => j,
        None => return false,
    };

    let key = match string_from_header(key_ptr) {
        Some(k) => k,
        None => return false,
    };

    match serde_json::from_str::<serde_json::Value>(&json_str) {
        Ok(serde_json::Value::Object(obj)) => {
            if let Some(serde_json::Value::Bool(b)) = obj.get(&key) {
                return *b;
            }
        }
        _ => {}
    }

    false
}
