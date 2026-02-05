//! JSON handling
//!
//! Provides JSON.parse() and JSON.stringify() functionality.

use perry_runtime::{
    js_array_alloc, js_array_push, js_object_alloc, js_object_set_field,
    js_object_set_keys, js_string_from_bytes, JSValue, StringHeader,
};
use std::fmt::Write as FmtWrite;

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
            let keys_arr = js_array_alloc(obj.len() as u32);
            for (idx, (key, value)) in obj.iter().enumerate() {
                // Add key to keys array
                let key_ptr = js_string_from_bytes(key.as_ptr(), key.len() as u32);
                js_array_push(keys_arr, JSValue::string_ptr(key_ptr));
                // Set field value
                js_object_set_field(js_obj, idx as u32, json_value_to_jsvalue(value));
            }
            js_object_set_keys(js_obj, keys_arr);
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

/// Type hint constants for js_json_stringify
const TYPE_UNKNOWN: u32 = 0;
const TYPE_OBJECT: u32 = 1;
const TYPE_ARRAY: u32 = 2;

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

/// Extract a pointer from a NaN-boxed or raw value
unsafe fn extract_pointer(bits: u64) -> Option<*const u8> {
    let is_nanboxed_ptr = (bits & 0xFFFF_0000_0000_0000) == POINTER_TAG;
    let is_raw_ptr = is_raw_pointer(bits);
    if is_nanboxed_ptr {
        Some((bits & POINTER_MASK) as *const u8)
    } else if is_raw_ptr {
        Some(bits as *const u8)
    } else {
        None
    }
}

/// Check if a pointer looks like an object (has valid keys array)
unsafe fn is_object_pointer(ptr: *const u8) -> bool {
    let obj = ptr as *const perry_runtime::ObjectHeader;
    let potential_keys_ptr = (*obj).keys_array as u64;
    let top_16_bits = potential_keys_ptr >> 48;
    let is_likely_heap_pointer = top_16_bits == 0 || top_16_bits == 1;
    let looks_like_valid_pointer = is_likely_heap_pointer
        && potential_keys_ptr > 0x10000
        && (potential_keys_ptr & 0x7) == 0;

    if looks_like_valid_pointer {
        let keys_arr = (*obj).keys_array;
        let keys_len = (*keys_arr).length;
        let keys_cap = (*keys_arr).capacity;
        let field_count = (*obj).field_count;
        keys_len <= keys_cap && keys_len > 0 && keys_cap < 1000 && field_count == keys_len && field_count < 1000
    } else {
        false
    }
}

/// Write a number to buffer
unsafe fn write_number(buf: &mut String, value: f64) {
    if value.is_nan() {
        buf.push_str("null");
    } else if value.is_infinite() {
        buf.push_str("null");
    } else if value.fract() == 0.0 && value.abs() < (i64::MAX as f64) {
        let _ = write!(buf, "{}", value as i64);
    } else {
        let _ = write!(buf, "{}", value);
    }
}

/// Write a JSON-escaped string to buffer
unsafe fn write_escaped_string(buf: &mut String, s: &str) {
    buf.push('"');
    for ch in s.chars() {
        match ch {
            '"' => buf.push_str("\\\""),
            '\\' => buf.push_str("\\\\"),
            '\n' => buf.push_str("\\n"),
            '\r' => buf.push_str("\\r"),
            '\t' => buf.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(buf, "\\u{:04x}", c as u32);
            }
            c => buf.push(c),
        }
    }
    buf.push('"');
}

/// Internal stringify that writes directly into a shared buffer.
/// type_hint: 0=unknown, 1=object, 2=array
unsafe fn stringify_value(value: f64, type_hint: u32, buf: &mut String) {
    let bits: u64 = value.to_bits();

    // Check special values
    if bits == TAG_NULL {
        buf.push_str("null");
        return;
    }
    if bits == TAG_TRUE {
        buf.push_str("true");
        return;
    }
    if bits == TAG_FALSE {
        buf.push_str("false");
        return;
    }

    // Check for string (STRING_TAG)
    let tag = bits & 0xFFFF_0000_0000_0000;
    if tag == STRING_TAG {
        let str_ptr = (bits & POINTER_MASK) as *const StringHeader;
        if let Some(s) = string_from_header(str_ptr) {
            write_escaped_string(buf, &s);
        } else {
            buf.push_str("null");
        }
        return;
    }

    // Check if it's a pointer (array, object)
    if let Some(ptr) = extract_pointer(bits) {
        // Use type_hint to skip heuristic when possible
        if type_hint == TYPE_OBJECT {
            stringify_object(ptr, buf);
            return;
        }
        if type_hint == TYPE_ARRAY {
            stringify_array(ptr, buf);
            return;
        }

        // Fallback: heuristic detection
        if is_object_pointer(ptr) {
            stringify_object(ptr, buf);
        } else {
            // Try as array
            let arr = ptr as *const perry_runtime::ArrayHeader;
            if !arr.is_null() {
                let len = (*arr).length;
                let cap = (*arr).capacity;
                if len <= cap && cap > 0 && cap < 10000 {
                    stringify_array(ptr, buf);
                    return;
                }
            }
            // Try as string
            let str_ptr = ptr as *const StringHeader;
            if let Some(s) = string_from_header(str_ptr) {
                write_escaped_string(buf, &s);
            } else {
                buf.push_str("null");
            }
        }
        return;
    }

    // It's a regular number
    write_number(buf, value);
}

/// Stringify an object pointer into buffer
unsafe fn stringify_object(ptr: *const u8, buf: &mut String) {
    let obj = ptr as *const perry_runtime::ObjectHeader;
    let num_fields = (*obj).field_count;
    buf.push('{');

    let keys_arr = (*obj).keys_array;
    let keys_len = (*keys_arr).length;
    let keys_elements = (keys_arr as *const u8)
        .add(std::mem::size_of::<perry_runtime::ArrayHeader>()) as *const f64;
    let fields_ptr = (ptr as *const u8)
        .add(std::mem::size_of::<perry_runtime::ObjectHeader>()) as *const f64;

    for f in 0..num_fields {
        if f > 0 {
            buf.push(',');
        }

        // Get field name from keys array
        if (f as u32) < keys_len {
            let key_f64 = *keys_elements.add(f as usize);
            let key_bits = key_f64.to_bits();
            let key_tag = key_bits & 0xFFFF_0000_0000_0000;
            let key_ptr = if key_tag == STRING_TAG || key_tag == POINTER_TAG {
                (key_bits & POINTER_MASK) as *const StringHeader
            } else {
                key_bits as *const StringHeader
            };
            if let Some(key_str) = string_from_header(key_ptr) {
                buf.push('"');
                buf.push_str(&key_str);
                buf.push_str("\":");
            } else {
                let _ = write!(buf, "\"field{}\":", f);
            }
        } else {
            let _ = write!(buf, "\"field{}\":", f);
        }

        // Get field value and stringify inline (no FFI call)
        let field_val = *fields_ptr.add(f as usize);
        stringify_value(field_val, TYPE_UNKNOWN, buf);
    }
    buf.push('}');
}

/// Stringify an array pointer into buffer
unsafe fn stringify_array(ptr: *const u8, buf: &mut String) {
    let arr = ptr as *const perry_runtime::ArrayHeader;
    let len = (*arr).length;
    let elements = (ptr as *const u8).add(std::mem::size_of::<perry_runtime::ArrayHeader>()) as *const f64;

    buf.push('[');
    for i in 0..len {
        if i > 0 {
            buf.push(',');
        }
        let elem = *elements.add(i as usize);
        let elem_bits = elem.to_bits();
        let elem_tag = elem_bits & 0xFFFF_0000_0000_0000;

        // Inline type dispatch instead of calling stringify_value for common cases
        if elem_tag == STRING_TAG {
            let str_ptr = (elem_bits & POINTER_MASK) as *const StringHeader;
            if let Some(s) = string_from_header(str_ptr) {
                write_escaped_string(buf, &s);
            } else {
                buf.push_str("null");
            }
        } else if elem_bits == TAG_NULL {
            buf.push_str("null");
        } else if elem_bits == TAG_TRUE {
            buf.push_str("true");
        } else if elem_bits == TAG_FALSE {
            buf.push_str("false");
        } else if elem_tag == POINTER_TAG || is_raw_pointer(elem_bits) {
            // Nested object or array
            let elem_ptr = if elem_tag == POINTER_TAG {
                (elem_bits & POINTER_MASK) as *const u8
            } else {
                elem_bits as *const u8
            };
            if is_object_pointer(elem_ptr) {
                stringify_object(elem_ptr, buf);
            } else {
                // Try as array
                let arr_elem = elem_ptr as *const perry_runtime::ArrayHeader;
                let arr_len = (*arr_elem).length;
                let arr_cap = (*arr_elem).capacity;
                if arr_len <= arr_cap && arr_cap > 0 && arr_cap < 10000 {
                    stringify_array(elem_ptr, buf);
                } else {
                    // Try as string
                    let str_ptr = elem_ptr as *const StringHeader;
                    if let Some(s) = string_from_header(str_ptr) {
                        write_escaped_string(buf, &s);
                    } else {
                        buf.push_str("null");
                    }
                }
            }
        } else {
            // Number
            write_number(buf, elem);
        }
    }
    buf.push(']');
}

/// Generic JSON.stringify that handles any JSValue
/// Takes a f64 (NaN-boxed JSValue) and a type_hint (0=unknown, 1=object, 2=array)
/// Returns a string pointer
#[no_mangle]
pub unsafe extern "C" fn js_json_stringify(value: f64, type_hint: u32) -> *mut StringHeader {
    let mut buf = String::with_capacity(256);
    stringify_value(value, type_hint, &mut buf);
    js_string_from_bytes(buf.as_ptr(), buf.len() as u32)
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
