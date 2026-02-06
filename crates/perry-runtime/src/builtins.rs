//! Built-in functions and objects
//!
//! Provides runtime implementations of JavaScript built-ins like console.log

use crate::JSValue;
use crate::string::{StringHeader, js_string_from_bytes};

/// Print a value to stdout (console.log implementation)
#[no_mangle]
pub extern "C" fn js_console_log(value: JSValue) {
    if value.is_undefined() {
        println!("undefined");
    } else if value.is_null() {
        println!("null");
    } else if value.is_bool() {
        println!("{}", value.as_bool());
    } else if value.is_number() {
        let n = value.as_number();
        // Print integers without decimal point
        if n.fract() == 0.0 && n.abs() < (i64::MAX as f64) {
            println!("{}", n as i64);
        } else {
            println!("{}", n);
        }
    } else if value.is_int32() {
        println!("{}", value.as_int32());
    } else {
        println!("{:?}", value);
    }
}

/// Print a dynamic value to stdout (for union types, etc.)
/// Takes an f64 that uses proper NaN-boxing to distinguish types.
/// - Numbers are stored as regular f64 values
/// - Strings are stored as NaN-boxed pointers (tag 0x7FFF)
/// - Objects are stored as NaN-boxed pointers (tag 0x7FFD)
#[no_mangle]
pub extern "C" fn js_console_log_dynamic(value: f64) {
    let jsval = JSValue::from_bits(value.to_bits());

    if jsval.is_undefined() {
        println!("undefined");
    } else if jsval.is_null() {
        println!("null");
    } else if jsval.is_bool() {
        println!("{}", jsval.as_bool());
    } else if jsval.is_string() {
        // String pointer (uses STRING_TAG 0x7FFF)
        let ptr = jsval.as_string_ptr();
        if ptr.is_null() {
            println!("null");
        } else {
            unsafe {
                let len = (*ptr).length as usize;
                let data = (ptr as *const u8).add(std::mem::size_of::<StringHeader>());
                let bytes = std::slice::from_raw_parts(data, len);
                if let Ok(s) = std::str::from_utf8(bytes) {
                    println!("{}", s);
                } else {
                    println!("[invalid utf8]");
                }
            }
        }
    } else if jsval.is_pointer() {
        // Object/array pointer - format as JSON
        println!("{}", format_jsvalue(value, 0));
    } else if jsval.is_int32() {
        println!("{}", jsval.as_int32());
    } else {
        // Must be a regular number
        let n = value;
        if n.is_nan() {
            println!("NaN");
        } else if n.is_infinite() {
            if n > 0.0 { println!("Infinity"); } else { println!("-Infinity"); }
        } else if n.fract() == 0.0 && n.abs() < (i64::MAX as f64) {
            println!("{}", n as i64);
        } else {
            println!("{}", n);
        }
    }
}

/// Print a number to stdout (optimized path for known numbers)
#[no_mangle]
pub extern "C" fn js_console_log_number(value: f64) {
    if value.fract() == 0.0 && value.abs() < (i64::MAX as f64) {
        println!("{}", value as i64);
    } else {
        println!("{}", value);
    }
}

/// Print an i32 to stderr (console.error)
#[no_mangle]
pub extern "C" fn js_console_error_i32(value: i32) {
    eprintln!("{}", value);
}

/// Print a dynamic value to stderr (console.error for union types)
#[no_mangle]
pub extern "C" fn js_console_error_dynamic(value: f64) {
    let jsval = JSValue::from_bits(value.to_bits());

    if jsval.is_undefined() {
        eprintln!("undefined");
    } else if jsval.is_null() {
        eprintln!("null");
    } else if jsval.is_bool() {
        eprintln!("{}", jsval.as_bool());
    } else if jsval.is_string() {
        let ptr = jsval.as_string_ptr();
        if ptr.is_null() {
            eprintln!("null");
        } else {
            unsafe {
                let len = (*ptr).length as usize;
                let data = (ptr as *const u8).add(std::mem::size_of::<StringHeader>());
                let bytes = std::slice::from_raw_parts(data, len);
                if let Ok(s) = std::str::from_utf8(bytes) {
                    eprintln!("{}", s);
                } else {
                    eprintln!("[invalid utf8]");
                }
            }
        }
    } else if jsval.is_pointer() {
        // Object/array pointer - format as JSON
        eprintln!("{}", format_jsvalue(value, 0));
    } else if jsval.is_int32() {
        eprintln!("{}", jsval.as_int32());
    } else {
        let n = value;
        if n.is_nan() {
            eprintln!("NaN");
        } else if n.is_infinite() {
            if n > 0.0 { eprintln!("Infinity"); } else { eprintln!("-Infinity"); }
        } else if n.fract() == 0.0 && n.abs() < (i64::MAX as f64) {
            eprintln!("{}", n as i64);
        } else {
            eprintln!("{}", n);
        }
    }
}

/// Print a number to stderr (console.error for numbers)
#[no_mangle]
pub extern "C" fn js_console_error_number(value: f64) {
    if value.fract() == 0.0 && value.abs() < (i64::MAX as f64) {
        eprintln!("{}", value as i64);
    } else {
        eprintln!("{}", value);
    }
}

/// Print an i32 to stderr (console.warn)
#[no_mangle]
pub extern "C" fn js_console_warn_i32(value: i32) {
    eprintln!("{}", value);
}

/// Print a dynamic value to stderr (console.warn for union types)
#[no_mangle]
pub extern "C" fn js_console_warn_dynamic(value: f64) {
    let jsval = JSValue::from_bits(value.to_bits());

    if jsval.is_undefined() {
        eprintln!("undefined");
    } else if jsval.is_null() {
        eprintln!("null");
    } else if jsval.is_bool() {
        eprintln!("{}", jsval.as_bool());
    } else if jsval.is_string() {
        let ptr = jsval.as_string_ptr();
        if ptr.is_null() {
            eprintln!("null");
        } else {
            unsafe {
                let len = (*ptr).length as usize;
                let data = (ptr as *const u8).add(std::mem::size_of::<StringHeader>());
                let bytes = std::slice::from_raw_parts(data, len);
                if let Ok(s) = std::str::from_utf8(bytes) {
                    eprintln!("{}", s);
                } else {
                    eprintln!("[invalid utf8]");
                }
            }
        }
    } else if jsval.is_pointer() {
        // Object/array pointer - format as JSON
        eprintln!("{}", format_jsvalue(value, 0));
    } else if jsval.is_int32() {
        eprintln!("{}", jsval.as_int32());
    } else {
        let n = value;
        if n.is_nan() {
            eprintln!("NaN");
        } else if n.is_infinite() {
            if n > 0.0 { eprintln!("Infinity"); } else { eprintln!("-Infinity"); }
        } else if n.fract() == 0.0 && n.abs() < (i64::MAX as f64) {
            eprintln!("{}", n as i64);
        } else {
            eprintln!("{}", n);
        }
    }
}

/// Print a number to stderr (console.warn for numbers)
#[no_mangle]
pub extern "C" fn js_console_warn_number(value: f64) {
    if value.fract() == 0.0 && value.abs() < (i64::MAX as f64) {
        eprintln!("{}", value as i64);
    } else {
        eprintln!("{}", value);
    }
}

/// Print an i32 to stdout
#[no_mangle]
pub extern "C" fn js_console_log_i32(value: i32) {
    println!("{}", value);
}

/// Print an i64 to stdout
#[no_mangle]
pub extern "C" fn js_console_log_i64(value: i64) {
    println!("{}", value);
}

/// Print multiple values from an array (console.log with spread support)
/// Takes a pointer to an ArrayHeader containing f64 values
/// Helper function to format a JSValue as a string (for spread arrays)
fn format_jsvalue(value: f64, depth: usize) -> String {
    // Prevent stack overflow with deeply nested structures
    if depth > 10 {
        return "[...]".to_string();
    }

    let jsval = JSValue::from_bits(value.to_bits());

    // Debug: check what type we're detecting
    let bits = value.to_bits();
    if bits > 0x7FF0_0000_0000_0000 && (bits >> 48) != 0x7FF8 {
        eprintln!("[DEBUG format_jsvalue] bits=0x{:016X} is_string={} is_pointer={} is_undefined={} is_null={}",
            bits, jsval.is_string(), jsval.is_pointer(), jsval.is_undefined(), jsval.is_null());
    }

    unsafe {
        if jsval.is_undefined() {
            "undefined".to_string()
        } else if jsval.is_null() {
            "null".to_string()
        } else if jsval.is_bool() {
            jsval.as_bool().to_string()
        } else if jsval.is_string() {
            let ptr = jsval.as_string_ptr();
            if ptr.is_null() {
                "null".to_string()
            } else {
                let len = (*ptr).length as usize;
                let data = (ptr as *const u8).add(std::mem::size_of::<StringHeader>());
                let bytes = std::slice::from_raw_parts(data, len);
                std::str::from_utf8(bytes).unwrap_or("[invalid utf8]").to_string()
            }
        } else if jsval.is_bigint() {
            // Format BigInt by converting to string
            let ptr = jsval.as_bigint_ptr();
            if ptr.is_null() {
                "null".to_string()
            } else {
                let str_ptr = crate::bigint::js_bigint_to_string(ptr);
                if str_ptr.is_null() {
                    "0n".to_string()
                } else {
                    let len = (*str_ptr).length as usize;
                    let data = (str_ptr as *const u8).add(std::mem::size_of::<StringHeader>());
                    let bytes = std::slice::from_raw_parts(data, len);
                    let num_str = std::str::from_utf8(bytes).unwrap_or("0");
                    format!("{}n", num_str)
                }
            }
        } else if jsval.is_pointer() {
            let ptr: *const crate::array::ArrayHeader = jsval.as_pointer();
            if ptr.is_null() {
                "null".to_string()
            } else {
                // First check if this is an Error object by checking the object_type field
                // Both ObjectHeader and ErrorHeader have object_type as the first u32 field
                let object_type = *(ptr as *const u32);
                if object_type == crate::error::OBJECT_TYPE_ERROR {
                    // This is an Error object - format as "Error: <message>"
                    let error_ptr = ptr as *const crate::error::ErrorHeader;
                    let name_ptr = (*error_ptr).name;
                    let message_ptr = (*error_ptr).message;

                    let name_str = if name_ptr.is_null() {
                        "Error".to_string()
                    } else {
                        let len = (*name_ptr).length as usize;
                        let data = (name_ptr as *const u8).add(std::mem::size_of::<StringHeader>());
                        let bytes = std::slice::from_raw_parts(data, len);
                        std::str::from_utf8(bytes).unwrap_or("Error").to_string()
                    };

                    let message_str = if message_ptr.is_null() {
                        "".to_string()
                    } else {
                        let len = (*message_ptr).length as usize;
                        let data = (message_ptr as *const u8).add(std::mem::size_of::<StringHeader>());
                        let bytes = std::slice::from_raw_parts(data, len);
                        std::str::from_utf8(bytes).unwrap_or("").to_string()
                    };

                    if message_str.is_empty() {
                        name_str
                    } else {
                        format!("{}: {}", name_str, message_str)
                    }
                } else {
                    // Check if this looks like an array by examining ArrayHeader fields
                    // ArrayHeader has length (u32) and capacity (u32), so check if they look valid
                    let maybe_arr = ptr;
                    let length = (*maybe_arr).length as usize;
                    let capacity = (*maybe_arr).capacity as usize;

                    // Heuristic: if capacity >= length and both are reasonable, it's likely an array
                    if capacity >= length && length < 1_000_000 && capacity < 10_000_000
                        && capacity > 0 // arrays have non-zero capacity
                    {
                        // Format as array
                        let data_ptr = (maybe_arr as *const u8).add(std::mem::size_of::<crate::array::ArrayHeader>()) as *const f64;
                        let mut parts: Vec<String> = Vec::with_capacity(length);
                        for i in 0..length {
                            let elem_value = *data_ptr.add(i);
                            parts.push(format_jsvalue(elem_value, depth + 1));
                        }
                        format!("[{}]", parts.join(", "))
                    } else {
                        // Try to check if it's an object with keys_array
                        let obj_ptr = ptr as *const crate::object::ObjectHeader;
                        let keys_array = (*obj_ptr).keys_array;

                        if !keys_array.is_null() {
                            // This is an object with keys - format as JSON
                            format_object_as_json(obj_ptr, depth)
                        } else {
                            // Class instance without keys_array
                            "[object Object]".to_string()
                        }
                    }
                }
            }
        } else if jsval.is_int32() {
            jsval.as_int32().to_string()
        } else {
            // Regular number
            let n = value;
            if n.is_nan() {
                "NaN".to_string()
            } else if n.is_infinite() {
                if n > 0.0 { "Infinity".to_string() } else { "-Infinity".to_string() }
            } else if n.fract() == 0.0 && n.abs() < (i64::MAX as f64) {
                (n as i64).to_string()
            } else {
                n.to_string()
            }
        }
    }
}

/// Format an object as JSON-like string
/// Reads keys from the keys_array and values from the fields
unsafe fn format_object_as_json(obj_ptr: *const crate::object::ObjectHeader, depth: usize) -> String {
    if depth > 10 {
        return "{...}".to_string();
    }

    let keys_array = (*obj_ptr).keys_array;
    if keys_array.is_null() {
        return "{}".to_string();
    }

    let key_count = crate::array::js_array_length(keys_array) as usize;
    if key_count == 0 {
        return "{}".to_string();
    }

    let mut parts: Vec<String> = Vec::with_capacity(key_count);

    for i in 0..key_count {
        // Get the key (NaN-boxed string pointer)
        let key_val = crate::array::js_array_get(keys_array, i as u32);
        let key_str = if key_val.is_string() {
            let key_ptr = key_val.as_string_ptr();
            if key_ptr.is_null() {
                continue;
            }
            let len = (*key_ptr).length as usize;
            let data = (key_ptr as *const u8).add(std::mem::size_of::<StringHeader>());
            let bytes = std::slice::from_raw_parts(data, len);
            std::str::from_utf8(bytes).unwrap_or("").to_string()
        } else {
            continue;
        };

        // Get the value
        let value = crate::object::js_object_get_field_f64(obj_ptr, i as u32);
        let value_str = format_jsvalue_for_json(value, depth + 1);

        parts.push(format!("{}: {}", key_str, value_str));
    }

    format!("{{ {} }}", parts.join(", "))
}

/// Format a JSValue for JSON output (strings get quotes)
fn format_jsvalue_for_json(value: f64, depth: usize) -> String {
    if depth > 10 {
        return "\"...\"".to_string();
    }

    let jsval = JSValue::from_bits(value.to_bits());

    unsafe {
        if jsval.is_undefined() {
            "undefined".to_string()
        } else if jsval.is_null() {
            "null".to_string()
        } else if jsval.is_bool() {
            jsval.as_bool().to_string()
        } else if jsval.is_string() {
            let ptr = jsval.as_string_ptr();
            if ptr.is_null() {
                "null".to_string()
            } else {
                let len = (*ptr).length as usize;
                let data = (ptr as *const u8).add(std::mem::size_of::<StringHeader>());
                let bytes = std::slice::from_raw_parts(data, len);
                let s = std::str::from_utf8(bytes).unwrap_or("[invalid utf8]");
                // Escape and quote strings for JSON-like output
                format!("'{}'", escape_string(s))
            }
        } else if jsval.is_bigint() {
            let ptr = jsval.as_bigint_ptr();
            if ptr.is_null() {
                "null".to_string()
            } else {
                let str_ptr = crate::bigint::js_bigint_to_string(ptr);
                if str_ptr.is_null() {
                    "0n".to_string()
                } else {
                    let len = (*str_ptr).length as usize;
                    let data = (str_ptr as *const u8).add(std::mem::size_of::<StringHeader>());
                    let bytes = std::slice::from_raw_parts(data, len);
                    let num_str = std::str::from_utf8(bytes).unwrap_or("0");
                    format!("{}n", num_str)
                }
            }
        } else if jsval.is_pointer() {
            let ptr: *const crate::array::ArrayHeader = jsval.as_pointer();
            if ptr.is_null() {
                "null".to_string()
            } else {
                // First check if this is an Error object
                let object_type = *(ptr as *const u32);
                if object_type == crate::error::OBJECT_TYPE_ERROR {
                    // Format Error as "Error: <message>"
                    let error_ptr = ptr as *const crate::error::ErrorHeader;
                    let name_ptr = (*error_ptr).name;
                    let message_ptr = (*error_ptr).message;

                    let name_str = if name_ptr.is_null() {
                        "Error".to_string()
                    } else {
                        let len = (*name_ptr).length as usize;
                        let data = (name_ptr as *const u8).add(std::mem::size_of::<StringHeader>());
                        let bytes = std::slice::from_raw_parts(data, len);
                        std::str::from_utf8(bytes).unwrap_or("Error").to_string()
                    };

                    let message_str = if message_ptr.is_null() {
                        "".to_string()
                    } else {
                        let len = (*message_ptr).length as usize;
                        let data = (message_ptr as *const u8).add(std::mem::size_of::<StringHeader>());
                        let bytes = std::slice::from_raw_parts(data, len);
                        std::str::from_utf8(bytes).unwrap_or("").to_string()
                    };

                    if message_str.is_empty() {
                        name_str
                    } else {
                        format!("{}: {}", name_str, message_str)
                    }
                } else {
                    // Check if it's an object with keys
                    let obj_ptr = ptr as *const crate::object::ObjectHeader;
                    let keys_array = (*obj_ptr).keys_array;

                    if !keys_array.is_null() {
                        format_object_as_json(obj_ptr, depth)
                    } else {
                        // Check if array
                        let maybe_arr = ptr;
                        let length = (*maybe_arr).length as usize;
                        let capacity = (*maybe_arr).capacity as usize;

                        if capacity >= length && length < 1_000_000 && capacity < 10_000_000 {
                            let data_ptr = (maybe_arr as *const u8).add(std::mem::size_of::<crate::array::ArrayHeader>()) as *const f64;
                            let mut parts: Vec<String> = Vec::with_capacity(length);
                            for i in 0..length {
                                let elem_value = *data_ptr.add(i);
                                parts.push(format_jsvalue_for_json(elem_value, depth + 1));
                            }
                            format!("[{}]", parts.join(", "))
                        } else {
                            "[object Object]".to_string()
                        }
                    }
                }
            }
        } else if jsval.is_int32() {
            jsval.as_int32().to_string()
        } else {
            let n = value;
            if n.is_nan() {
                "NaN".to_string()
            } else if n.is_infinite() {
                if n > 0.0 { "Infinity".to_string() } else { "-Infinity".to_string() }
            } else if n.fract() == 0.0 && n.abs() < (i64::MAX as f64) {
                (n as i64).to_string()
            } else {
                n.to_string()
            }
        }
    }
}

/// Escape special characters in a string for display
fn escape_string(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => result.push_str("\\\\"),
            '\'' => result.push_str("\\'"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            _ => result.push(c),
        }
    }
    result
}

#[no_mangle]
pub extern "C" fn js_console_log_spread(arr_ptr: *const crate::array::ArrayHeader) {
    if arr_ptr.is_null() {
        println!();
        return;
    }

    unsafe {
        let length = (*arr_ptr).length as usize;
        let data_ptr = (arr_ptr as *const u8).add(std::mem::size_of::<crate::array::ArrayHeader>()) as *const f64;

        eprintln!("[DEBUG js_console_log_spread] array length={}", length);
        let mut parts: Vec<String> = Vec::with_capacity(length);
        for i in 0..length {
            let value = *data_ptr.add(i);
            let bits = value.to_bits();
            eprintln!("[DEBUG spread] i={} bits=0x{:016X}", i, bits);
            parts.push(format_jsvalue(value, 0));
        }
        println!("{}", parts.join(" "));
    }
}

/// Print multiple values to stderr (console.error with spread support)
#[no_mangle]
pub extern "C" fn js_console_error_spread(arr_ptr: *const crate::array::ArrayHeader) {
    if arr_ptr.is_null() {
        eprintln!();
        return;
    }

    unsafe {
        let length = (*arr_ptr).length as usize;
        let data_ptr = (arr_ptr as *const u8).add(std::mem::size_of::<crate::array::ArrayHeader>()) as *const f64;

        let mut parts: Vec<String> = Vec::with_capacity(length);
        for i in 0..length {
            let value = *data_ptr.add(i);
            parts.push(format_jsvalue(value, 0));
        }
        eprintln!("{}", parts.join(" "));
    }
}

/// Print multiple values to stderr (console.warn with spread support)
#[no_mangle]
pub extern "C" fn js_console_warn_spread(arr_ptr: *const crate::array::ArrayHeader) {
    // console.warn is essentially the same as console.error in Node.js
    js_console_error_spread(arr_ptr);
}

/// Print an array in the format [element1, element2, ...]
#[no_mangle]
pub extern "C" fn js_array_print(arr_ptr: *const crate::array::ArrayHeader) {
    if arr_ptr.is_null() {
        println!("null");
        return;
    }

    unsafe {
        let length = (*arr_ptr).length as usize;
        let data_ptr = (arr_ptr as *const u8).add(std::mem::size_of::<crate::array::ArrayHeader>()) as *const f64;

        let mut parts: Vec<String> = Vec::with_capacity(length);
        for i in 0..length {
            let value = *data_ptr.add(i);
            parts.push(format_jsvalue_for_json(value, 0));
        }
        println!("[{}]", parts.join(", "));
    }
}

// Arithmetic operations on JSValue (with type coercion)

#[no_mangle]
pub extern "C" fn js_add(a: JSValue, b: JSValue) -> JSValue {
    // For MVP, just handle number + number
    JSValue::number(a.to_number() + b.to_number())
}

#[no_mangle]
pub extern "C" fn js_sub(a: JSValue, b: JSValue) -> JSValue {
    JSValue::number(a.to_number() - b.to_number())
}

#[no_mangle]
pub extern "C" fn js_mul(a: JSValue, b: JSValue) -> JSValue {
    JSValue::number(a.to_number() * b.to_number())
}

#[no_mangle]
pub extern "C" fn js_div(a: JSValue, b: JSValue) -> JSValue {
    JSValue::number(a.to_number() / b.to_number())
}

#[no_mangle]
pub extern "C" fn js_mod(a: JSValue, b: JSValue) -> JSValue {
    JSValue::number(a.to_number() % b.to_number())
}

// Comparison operations

#[no_mangle]
pub extern "C" fn js_eq(a: JSValue, b: JSValue) -> JSValue {
    // Strict equality for numbers
    if a.is_number() && b.is_number() {
        JSValue::bool(a.as_number() == b.as_number())
    } else if a.bits() == b.bits() {
        JSValue::bool(true)
    } else {
        JSValue::bool(false)
    }
}

#[no_mangle]
pub extern "C" fn js_lt(a: JSValue, b: JSValue) -> JSValue {
    JSValue::bool(a.to_number() < b.to_number())
}

#[no_mangle]
pub extern "C" fn js_le(a: JSValue, b: JSValue) -> JSValue {
    JSValue::bool(a.to_number() <= b.to_number())
}

#[no_mangle]
pub extern "C" fn js_gt(a: JSValue, b: JSValue) -> JSValue {
    JSValue::bool(a.to_number() > b.to_number())
}

#[no_mangle]
pub extern "C" fn js_ge(a: JSValue, b: JSValue) -> JSValue {
    JSValue::bool(a.to_number() >= b.to_number())
}

/// Return the typeof a value as a string
/// Takes an f64 that uses NaN-boxing to distinguish types.
/// Returns a pointer to a string: "undefined", "boolean", "number", "string", "object", "function"
#[no_mangle]
pub extern "C" fn js_value_typeof(value: f64) -> *mut StringHeader {
    use crate::string::js_string_from_bytes;

    let jsval = JSValue::from_bits(value.to_bits());

    let type_str = if jsval.is_undefined() {
        "undefined"
    } else if jsval.is_null() {
        // typeof null === "object" in JavaScript
        "object"
    } else if jsval.is_bool() {
        "boolean"
    } else if jsval.is_string() {
        // String pointer (uses STRING_TAG)
        "string"
    } else if jsval.is_pointer() {
        // Object/array/closure pointer - check if it's a closure
        let ptr = jsval.as_pointer::<u8>();
        if !ptr.is_null() {
            // ClosureHeader has type_tag at offset 12 (after func_ptr:8 + capture_count:4)
            let type_tag = unsafe { *(ptr.add(12) as *const u32) };
            if type_tag == crate::closure::CLOSURE_MAGIC {
                "function"
            } else {
                "object"
            }
        } else {
            "object"
        }
    } else if jsval.is_int32() {
        "number"
    } else {
        // Regular f64 number
        "number"
    };

    js_string_from_bytes(type_str.as_ptr(), type_str.len() as u32)
}

/// parseInt(string, radix?) -> number
/// Parses a string and returns an integer.
/// If the string cannot be parsed, returns NaN.
#[no_mangle]
pub extern "C" fn js_parse_int(str_ptr: *const StringHeader, radix: f64) -> f64 {
    if str_ptr.is_null() {
        return f64::NAN;
    }

    unsafe {
        let len = (*str_ptr).length as usize;
        let data = (str_ptr as *const u8).add(std::mem::size_of::<StringHeader>());
        let bytes = std::slice::from_raw_parts(data, len);

        if let Ok(s) = std::str::from_utf8(bytes) {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                return f64::NAN;
            }

            // Determine radix
            let radix = if radix.is_nan() || radix == 0.0 {
                10
            } else {
                radix as u32
            };

            // Handle sign
            let (is_negative, trimmed) = if trimmed.starts_with('-') {
                (true, &trimmed[1..])
            } else if trimmed.starts_with('+') {
                (false, &trimmed[1..])
            } else {
                (false, trimmed)
            };

            // Handle hex prefix (only if radix is 16 or auto)
            let (actual_radix, trimmed) = if (radix == 16 || radix == 10) &&
                (trimmed.starts_with("0x") || trimmed.starts_with("0X")) {
                (16, &trimmed[2..])
            } else {
                (radix, trimmed)
            };

            // Parse characters until we hit a non-digit
            let valid_chars: String = trimmed.chars()
                .take_while(|c| c.is_digit(actual_radix))
                .collect();

            if valid_chars.is_empty() {
                return f64::NAN;
            }

            match i64::from_str_radix(&valid_chars, actual_radix) {
                Ok(n) => {
                    let result = if is_negative { -n } else { n };
                    result as f64
                }
                Err(_) => f64::NAN,
            }
        } else {
            f64::NAN
        }
    }
}

/// parseFloat(string) -> number
/// Parses a string and returns a floating-point number.
#[no_mangle]
pub extern "C" fn js_parse_float(str_ptr: *const StringHeader) -> f64 {
    if str_ptr.is_null() {
        return f64::NAN;
    }

    unsafe {
        let len = (*str_ptr).length as usize;
        let data = (str_ptr as *const u8).add(std::mem::size_of::<StringHeader>());
        let bytes = std::slice::from_raw_parts(data, len);

        if let Ok(s) = std::str::from_utf8(bytes) {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                return f64::NAN;
            }

            // Parse as much of the string as is a valid float
            // JavaScript parseFloat stops at first invalid character
            let valid_chars: String = trimmed.chars()
                .scan(false, |seen_dot, c| {
                    if c.is_ascii_digit() {
                        Some(c)
                    } else if c == '.' && !*seen_dot {
                        *seen_dot = true;
                        Some(c)
                    } else if c == '-' || c == '+' {
                        Some(c)
                    } else if c == 'e' || c == 'E' {
                        Some(c)
                    } else {
                        None
                    }
                })
                .collect();

            match valid_chars.parse::<f64>() {
                Ok(n) => n,
                Err(_) => f64::NAN,
            }
        } else {
            f64::NAN
        }
    }
}

/// Number(value) -> number
/// Converts a value to a number.
#[no_mangle]
pub extern "C" fn js_number_coerce(value: f64) -> f64 {
    let jsval = JSValue::from_bits(value.to_bits());

    if jsval.is_undefined() {
        f64::NAN
    } else if jsval.is_null() {
        0.0
    } else if jsval.is_bool() {
        if jsval.as_bool() { 1.0 } else { 0.0 }
    } else if jsval.is_string() {
        // Parse string as number
        let ptr = jsval.as_string_ptr();
        if ptr.is_null() {
            return f64::NAN;
        }
        unsafe {
            let len = (*ptr).length as usize;
            let data = (ptr as *const u8).add(std::mem::size_of::<StringHeader>());
            let bytes = std::slice::from_raw_parts(data, len);
            if let Ok(s) = std::str::from_utf8(bytes) {
                let trimmed = s.trim();
                if trimmed.is_empty() {
                    return 0.0;
                }
                match trimmed.parse::<f64>() {
                    Ok(n) => n,
                    Err(_) => f64::NAN,
                }
            } else {
                f64::NAN
            }
        }
    } else {
        // Already a number
        value
    }
}

/// String(value) -> string
/// Converts a value to a string.
#[no_mangle]
pub extern "C" fn js_string_coerce(value: f64) -> *mut StringHeader {
    let jsval = JSValue::from_bits(value.to_bits());

    let result = if jsval.is_undefined() {
        "undefined".to_string()
    } else if jsval.is_null() {
        "null".to_string()
    } else if jsval.is_bool() {
        if jsval.as_bool() { "true".to_string() } else { "false".to_string() }
    } else if jsval.is_string() {
        // Already a string, return as-is
        return jsval.as_string_ptr() as *mut StringHeader;
    } else if jsval.is_int32() {
        jsval.as_int32().to_string()
    } else {
        // Regular number
        let n = value;
        if n.fract() == 0.0 && n.abs() < (i64::MAX as f64) {
            (n as i64).to_string()
        } else {
            n.to_string()
        }
    };

    js_string_from_bytes(result.as_ptr(), result.len() as u32)
}

/// isNaN(value) -> boolean
/// Returns true if value is NaN.
#[no_mangle]
pub extern "C" fn js_is_nan(value: f64) -> f64 {
    let jsval = JSValue::from_bits(value.to_bits());

    // isNaN first coerces to number, then checks for NaN
    let num = if jsval.is_undefined() {
        f64::NAN
    } else if jsval.is_null() {
        0.0
    } else if jsval.is_bool() {
        if jsval.as_bool() { 1.0 } else { 0.0 }
    } else if jsval.is_string() {
        // Parse string as number
        let ptr = jsval.as_string_ptr();
        if ptr.is_null() {
            f64::NAN
        } else {
            unsafe {
                let len = (*ptr).length as usize;
                let data = (ptr as *const u8).add(std::mem::size_of::<StringHeader>());
                let bytes = std::slice::from_raw_parts(data, len);
                if let Ok(s) = std::str::from_utf8(bytes) {
                    let trimmed = s.trim();
                    if trimmed.is_empty() {
                        0.0
                    } else {
                        match trimmed.parse::<f64>() {
                            Ok(n) => n,
                            Err(_) => f64::NAN,
                        }
                    }
                } else {
                    f64::NAN
                }
            }
        }
    } else {
        value
    };

    if num.is_nan() { 1.0 } else { 0.0 }
}

/// isFinite(value) -> boolean
/// Returns true if value is a finite number.
#[no_mangle]
pub extern "C" fn js_is_finite(value: f64) -> f64 {
    let jsval = JSValue::from_bits(value.to_bits());

    // isFinite first coerces to number, then checks for finite
    let num = if jsval.is_undefined() {
        f64::NAN
    } else if jsval.is_null() {
        0.0
    } else if jsval.is_bool() {
        if jsval.as_bool() { 1.0 } else { 0.0 }
    } else if jsval.is_string() {
        // Parse string as number
        let ptr = jsval.as_string_ptr();
        if ptr.is_null() {
            f64::NAN
        } else {
            unsafe {
                let len = (*ptr).length as usize;
                let data = (ptr as *const u8).add(std::mem::size_of::<StringHeader>());
                let bytes = std::slice::from_raw_parts(data, len);
                if let Ok(s) = std::str::from_utf8(bytes) {
                    let trimmed = s.trim();
                    if trimmed.is_empty() {
                        0.0
                    } else {
                        match trimmed.parse::<f64>() {
                            Ok(n) => n,
                            Err(_) => f64::NAN,
                        }
                    }
                } else {
                    f64::NAN
                }
            }
        }
    } else {
        value
    };

    if num.is_finite() { 1.0 } else { 0.0 }
}
