//! RegExp runtime support for Perry
//!
//! Provides JavaScript-compatible regular expression operations using the Rust regex crate.
//! RegExp objects are heap-allocated and store the compiled pattern and flags.

use regex::Regex;
use std::alloc::{alloc, Layout};
use std::ptr;

use crate::array::ArrayHeader;
use crate::string::StringHeader;

/// Header for heap-allocated RegExp objects
#[repr(C)]
pub struct RegExpHeader {
    /// Pointer to the compiled Regex object (boxed)
    regex_ptr: *mut Regex,
    /// Original pattern string (for debugging/serialization)
    pattern_ptr: *const StringHeader,
    /// Flags string (e.g., "gi" for global+ignoreCase)
    flags_ptr: *const StringHeader,
    /// Cached flags for quick access
    pub case_insensitive: bool,
    pub global: bool,
    pub multiline: bool,
}

/// Internal helper: Get string data from StringHeader
fn string_as_str<'a>(s: *const StringHeader) -> &'a str {
    unsafe {
        let len = (*s).length as usize;
        let data = (s as *const u8).add(std::mem::size_of::<StringHeader>());
        let bytes = std::slice::from_raw_parts(data, len);
        std::str::from_utf8_unchecked(bytes)
    }
}

/// Internal helper: Create a StringHeader from a Rust &str
fn js_string_from_str(s: &str) -> *mut StringHeader {
    crate::string::js_string_from_bytes(s.as_ptr(), s.len() as u32)
}

/// Create a new RegExp from pattern and flags strings
/// Returns a pointer to RegExpHeader
#[no_mangle]
pub extern "C" fn js_regexp_new(pattern: *const StringHeader, flags: *const StringHeader) -> *mut RegExpHeader {
    let pattern_str = if pattern.is_null() { "" } else { string_as_str(pattern) };
    let flags_str = if flags.is_null() { "" } else { string_as_str(flags) };

    // Parse flags
    let case_insensitive = flags_str.contains('i');
    let global = flags_str.contains('g');
    let multiline = flags_str.contains('m');

    // Build the regex pattern with flags
    let regex_pattern = if case_insensitive || multiline {
        let mut prefix = String::from("(?");
        if case_insensitive { prefix.push('i'); }
        if multiline { prefix.push('m'); }
        prefix.push(')');
        format!("{}{}", prefix, pattern_str)
    } else {
        pattern_str.to_string()
    };

    // Try to compile the regex
    let regex = match Regex::new(&regex_pattern) {
        Ok(r) => r,
        Err(_) => {
            // Return a dummy regex that matches nothing on error
            // In production, this should throw an exception
            Regex::new("(?!.*)").unwrap()
        }
    };

    // Allocate the header
    let layout = Layout::new::<RegExpHeader>();
    unsafe {
        let ptr = alloc(layout) as *mut RegExpHeader;
        if ptr.is_null() {
            panic!("Failed to allocate RegExp");
        }

        // Box the regex and store it
        let regex_box = Box::new(regex);
        (*ptr).regex_ptr = Box::into_raw(regex_box);
        (*ptr).pattern_ptr = pattern;
        (*ptr).flags_ptr = flags;
        (*ptr).case_insensitive = case_insensitive;
        (*ptr).global = global;
        (*ptr).multiline = multiline;

        ptr
    }
}

/// Test if a string matches the regex pattern
/// regex.test(string) -> boolean
#[no_mangle]
pub extern "C" fn js_regexp_test(re: *const RegExpHeader, s: *const StringHeader) -> bool {
    if re.is_null() || s.is_null() {
        return false;
    }

    let str_data = string_as_str(s);

    unsafe {
        let regex = &*(*re).regex_ptr;
        regex.is_match(str_data)
    }
}

/// Find matches in a string
/// string.match(regex) -> string[] | null (returns array pointer, null if no match)
#[no_mangle]
pub extern "C" fn js_string_match(s: *const StringHeader, re: *const RegExpHeader) -> *mut ArrayHeader {
    if s.is_null() || re.is_null() {
        return ptr::null_mut();
    }

    let str_data = string_as_str(s);

    unsafe {
        let regex = &*(*re).regex_ptr;
        let global = (*re).global;

        if global {
            // Global flag: return all matches
            let matches: Vec<&str> = regex.find_iter(str_data).map(|m| m.as_str()).collect();

            if matches.is_empty() {
                return ptr::null_mut();
            }

            // Create array of string pointers
            let arr = crate::array::js_array_alloc(matches.len() as u32);
            (*arr).length = matches.len() as u32;
            let elements_ptr = (arr as *mut u8).add(std::mem::size_of::<ArrayHeader>()) as *mut f64;

            for (i, m) in matches.iter().enumerate() {
                let str_ptr = js_string_from_str(m);
                let ptr_as_u64 = str_ptr as u64;
                let ptr_as_f64 = f64::from_bits(ptr_as_u64);
                std::ptr::write(elements_ptr.add(i), ptr_as_f64);
            }

            arr
        } else {
            // Non-global: return first match only (or with capture groups)
            match regex.captures(str_data) {
                Some(caps) => {
                    // Return array with full match and capture groups
                    let arr = crate::array::js_array_alloc(caps.len() as u32);
                    (*arr).length = caps.len() as u32;
                    let elements_ptr = (arr as *mut u8).add(std::mem::size_of::<ArrayHeader>()) as *mut f64;

                    for (i, cap) in caps.iter().enumerate() {
                        if let Some(m) = cap {
                            let str_ptr = js_string_from_str(m.as_str());
                            let ptr_as_u64 = str_ptr as u64;
                            let ptr_as_f64 = f64::from_bits(ptr_as_u64);
                            std::ptr::write(elements_ptr.add(i), ptr_as_f64);
                        } else {
                            // Undefined capture group - store as 0 (null ptr as f64)
                            std::ptr::write(elements_ptr.add(i), 0.0);
                        }
                    }

                    arr
                }
                None => ptr::null_mut(),
            }
        }
    }
}

/// Replace matches in a string
/// string.replace(regex, replacement) -> string
#[no_mangle]
pub extern "C" fn js_string_replace_regex(
    s: *const StringHeader,
    re: *const RegExpHeader,
    replacement: *const StringHeader,
) -> *mut StringHeader {
    if s.is_null() {
        return js_string_from_str("");
    }

    let str_data = string_as_str(s);
    let repl_str = if replacement.is_null() { "undefined" } else { string_as_str(replacement) };

    if re.is_null() {
        // If regex is null, return original string
        return js_string_from_str(str_data);
    }

    unsafe {
        let regex = &*(*re).regex_ptr;
        let global = (*re).global;

        let result = if global {
            // Global flag: replace all occurrences
            regex.replace_all(str_data, repl_str).to_string()
        } else {
            // Non-global: replace first occurrence only
            regex.replace(str_data, repl_str).to_string()
        };

        js_string_from_str(&result)
    }
}

/// Replace with a simple string pattern (not regex)
/// string.replace(pattern, replacement) -> string
#[no_mangle]
pub extern "C" fn js_string_replace_string(
    s: *const StringHeader,
    pattern: *const StringHeader,
    replacement: *const StringHeader,
) -> *mut StringHeader {
    if s.is_null() {
        return js_string_from_str("");
    }

    let str_data = string_as_str(s);
    let pattern_str = if pattern.is_null() { "" } else { string_as_str(pattern) };
    let repl_str = if replacement.is_null() { "undefined" } else { string_as_str(replacement) };

    // String.replace with a string pattern only replaces the first occurrence
    let result = str_data.replacen(pattern_str, repl_str, 1);
    js_string_from_str(&result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::string::js_string_from_bytes;

    fn make_string(s: &str) -> *mut StringHeader {
        js_string_from_bytes(s.as_ptr(), s.len() as u32)
    }

    #[test]
    fn test_regexp_test_basic() {
        let pattern = make_string("hello");
        let flags = make_string("");
        let re = js_regexp_new(pattern, flags);

        let test_str = make_string("hello world");
        assert!(js_regexp_test(re, test_str));

        let test_str2 = make_string("goodbye world");
        assert!(!js_regexp_test(re, test_str2));
    }

    #[test]
    fn test_regexp_test_case_insensitive() {
        let pattern = make_string("hello");
        let flags = make_string("i");
        let re = js_regexp_new(pattern, flags);

        let test_str = make_string("HELLO World");
        assert!(js_regexp_test(re, test_str));
    }

    #[test]
    fn test_string_match() {
        let pattern = make_string(r"\w+");
        let flags = make_string("");
        let re = js_regexp_new(pattern, flags);

        let test_str = make_string("hello world");
        let result = js_string_match(test_str, re);
        assert!(!result.is_null());

        unsafe {
            assert_eq!((*result).length, 1); // One match (first word)
        }
    }

    #[test]
    fn test_string_match_global() {
        let pattern = make_string(r"\w+");
        let flags = make_string("g");
        let re = js_regexp_new(pattern, flags);

        let test_str = make_string("hello world");
        let result = js_string_match(test_str, re);
        assert!(!result.is_null());

        unsafe {
            assert_eq!((*result).length, 2); // Two matches (hello, world)
        }
    }

    #[test]
    fn test_string_replace() {
        let pattern = make_string("world");
        let flags = make_string("");
        let re = js_regexp_new(pattern, flags);

        let test_str = make_string("hello world");
        let replacement = make_string("universe");
        let result = js_string_replace_regex(test_str, re, replacement);

        assert_eq!(string_as_str(result), "hello universe");
    }

    #[test]
    fn test_string_replace_global() {
        let pattern = make_string("o");
        let flags = make_string("g");
        let re = js_regexp_new(pattern, flags);

        let test_str = make_string("hello world");
        let replacement = make_string("0");
        let result = js_string_replace_regex(test_str, re, replacement);

        assert_eq!(string_as_str(result), "hell0 w0rld");
    }
}
