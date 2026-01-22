//! Slugify module (slugify compatible)
//!
//! Native implementation of the 'slugify' npm package.
//! Converts strings to URL-friendly slugs.

use perry_runtime::{js_string_from_bytes, StringHeader};

/// Helper to extract string from StringHeader pointer
unsafe fn string_from_header(ptr: *const StringHeader) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    let len = (*ptr).length as usize;
    let data_ptr = (ptr as *const u8).add(std::mem::size_of::<StringHeader>());
    let bytes = std::slice::from_raw_parts(data_ptr, len);
    std::str::from_utf8(bytes).ok().map(|s| s.to_string())
}

/// Character replacement map for common accented characters
fn replace_accents(c: char) -> Option<char> {
    match c {
        'á' | 'à' | 'â' | 'ä' | 'ã' | 'å' => Some('a'),
        'Á' | 'À' | 'Â' | 'Ä' | 'Ã' | 'Å' => Some('a'),
        'é' | 'è' | 'ê' | 'ë' => Some('e'),
        'É' | 'È' | 'Ê' | 'Ë' => Some('e'),
        'í' | 'ì' | 'î' | 'ï' => Some('i'),
        'Í' | 'Ì' | 'Î' | 'Ï' => Some('i'),
        'ó' | 'ò' | 'ô' | 'ö' | 'õ' | 'ø' => Some('o'),
        'Ó' | 'Ò' | 'Ô' | 'Ö' | 'Õ' | 'Ø' => Some('o'),
        'ú' | 'ù' | 'û' | 'ü' => Some('u'),
        'Ú' | 'Ù' | 'Û' | 'Ü' => Some('u'),
        'ý' | 'ÿ' => Some('y'),
        'Ý' | 'Ÿ' => Some('y'),
        'ñ' => Some('n'),
        'Ñ' => Some('n'),
        'ç' => Some('c'),
        'Ç' => Some('c'),
        'ß' => Some('s'),
        'æ' => Some('a'),
        'Æ' => Some('a'),
        'œ' => Some('o'),
        'Œ' => Some('o'),
        'ð' => Some('d'),
        'Ð' => Some('d'),
        'þ' => Some('t'),
        'Þ' => Some('t'),
        _ => None,
    }
}

/// Convert a string to a URL-friendly slug
/// slugify(string) -> string
#[no_mangle]
pub unsafe extern "C" fn js_slugify(input_ptr: *const StringHeader) -> *mut StringHeader {
    js_slugify_with_options(input_ptr, std::ptr::null(), std::ptr::null())
}

/// Convert a string to a URL-friendly slug with options
/// slugify(string, { replacement, lower }) -> string
#[no_mangle]
pub unsafe extern "C" fn js_slugify_with_options(
    input_ptr: *const StringHeader,
    replacement_ptr: *const StringHeader,
    _options_ptr: *const StringHeader, // Reserved for future options
) -> *mut StringHeader {
    let input = match string_from_header(input_ptr) {
        Some(s) => s,
        None => return std::ptr::null_mut(),
    };

    let replacement = string_from_header(replacement_ptr).unwrap_or_else(|| "-".to_string());
    let replacement_char = replacement.chars().next().unwrap_or('-');

    let mut result = String::with_capacity(input.len());
    let mut last_was_separator = true; // Start true to trim leading separators

    for c in input.chars() {
        // Check for accent replacement first
        let c = replace_accents(c).unwrap_or(c);

        if c.is_ascii_alphanumeric() {
            result.push(c.to_ascii_lowercase());
            last_was_separator = false;
        } else if c.is_whitespace() || c == '_' || c == '-' || c == '/' || c == '\\' {
            // Replace whitespace and common separators
            if !last_was_separator {
                result.push(replacement_char);
                last_was_separator = true;
            }
        }
        // Other characters are stripped
    }

    // Remove trailing separator
    if result.ends_with(replacement_char) {
        result.pop();
    }

    js_string_from_bytes(result.as_ptr(), result.len() as u32)
}

/// Slugify with strict mode (only alphanumeric)
/// slugify(string, { strict: true }) -> string
#[no_mangle]
pub unsafe extern "C" fn js_slugify_strict(input_ptr: *const StringHeader) -> *mut StringHeader {
    let input = match string_from_header(input_ptr) {
        Some(s) => s,
        None => return std::ptr::null_mut(),
    };

    let mut result = String::with_capacity(input.len());
    let mut last_was_separator = true;

    for c in input.chars() {
        let c = replace_accents(c).unwrap_or(c);

        if c.is_ascii_alphanumeric() {
            result.push(c.to_ascii_lowercase());
            last_was_separator = false;
        } else if !last_was_separator {
            result.push('-');
            last_was_separator = true;
        }
    }

    if result.ends_with('-') {
        result.pop();
    }

    js_string_from_bytes(result.as_ptr(), result.len() as u32)
}
