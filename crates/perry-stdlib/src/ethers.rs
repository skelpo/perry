//! ethers.js utilities
//!
//! Provides formatUnits and parseUnits for BigInt/decimal conversion.

use perry_runtime::{js_string_from_bytes, js_bigint_from_string, BigIntHeader, StringHeader};

/// formatUnits(value: bigint, decimals: number) -> string
/// Converts a BigInt to a human-readable string with the given number of decimals.
/// Example: formatUnits(1000000n, 6) -> "1.0"
#[no_mangle]
pub extern "C" fn js_ethers_format_units(bigint_ptr: *const BigIntHeader, decimals: f64) -> *mut StringHeader {
    if bigint_ptr.is_null() {
        let s = "0";
        return js_string_from_bytes(s.as_ptr(), s.len() as u32);
    }

    let decimals = decimals as i32;
    if decimals < 0 || decimals > 77 {
        let s = "0";
        return js_string_from_bytes(s.as_ptr(), s.len() as u32);
    }

    unsafe {
        // Read the BigInt value - fixed 256-bit (4 limbs)
        let bigint = &*bigint_ptr;
        let limbs = &bigint.limbs;

        // Convert to big integer string (always positive in current impl)
        let value_str = limbs_to_string(limbs);

        // Format with decimals
        let formatted = format_with_decimals(&value_str, decimals as usize);

        js_string_from_bytes(formatted.as_ptr(), formatted.len() as u32)
    }
}

/// parseUnits(value: string, decimals: number) -> bigint
/// Parses a human-readable string to a BigInt with the given number of decimals.
/// Example: parseUnits("1.0", 6) -> 1000000n
#[no_mangle]
pub extern "C" fn js_ethers_parse_units(str_ptr: *const StringHeader, decimals: f64) -> *mut BigIntHeader {
    if str_ptr.is_null() {
        let s = "0";
        return js_bigint_from_string(s.as_ptr(), s.len() as u32);
    }

    let decimals = decimals as i32;
    if decimals < 0 || decimals > 77 {
        let s = "0";
        return js_bigint_from_string(s.as_ptr(), s.len() as u32);
    }

    unsafe {
        let len = (*str_ptr).length as usize;
        let data = (str_ptr as *const u8).add(std::mem::size_of::<StringHeader>());
        let bytes = std::slice::from_raw_parts(data, len);

        if let Ok(s) = std::str::from_utf8(bytes) {
            let parsed = parse_units_to_string(s.trim(), decimals as usize);
            js_bigint_from_string(parsed.as_ptr(), parsed.len() as u32)
        } else {
            let s = "0";
            js_bigint_from_string(s.as_ptr(), s.len() as u32)
        }
    }
}

/// Convert limbs (little-endian u64 array, fixed 4 limbs) to decimal string
fn limbs_to_string(limbs: &[u64; 4]) -> String {
    if limbs.iter().all(|&x| x == 0) {
        return "0".to_string();
    }

    // For U256, we can use repeated division by 10
    // Work with a copy of the limbs
    let mut work = *limbs;
    let mut digits = Vec::with_capacity(78); // max digits for U256

    while !is_zero(&work) {
        let remainder = div_by_10(&mut work);
        digits.push((b'0' + remainder) as char);
    }

    // Reverse to get correct order
    digits.reverse();
    digits.into_iter().collect()
}

/// Check if U256 (4 limbs) is zero
fn is_zero(limbs: &[u64; 4]) -> bool {
    limbs[0] == 0 && limbs[1] == 0 && limbs[2] == 0 && limbs[3] == 0
}

/// Divide U256 (4 limbs in little-endian) by 10, return remainder
fn div_by_10(limbs: &mut [u64; 4]) -> u8 {
    let mut remainder: u128 = 0;

    // Process from most significant to least significant limb
    for i in (0..4).rev() {
        let current = (remainder << 64) | (limbs[i] as u128);
        limbs[i] = (current / 10) as u64;
        remainder = current % 10;
    }

    remainder as u8
}

/// Format a number string with decimal places
fn format_with_decimals(value: &str, decimals: usize) -> String {
    if decimals == 0 {
        return value.to_string();
    }

    let is_negative = value.starts_with('-');
    let value = if is_negative { &value[1..] } else { value };

    let len = value.len();

    if len <= decimals {
        // Value is less than 1
        let zeros = decimals - len;
        let result = format!("0.{}{}", "0".repeat(zeros), value);
        if is_negative { format!("-{}", result) } else { result }
    } else {
        // Insert decimal point
        let split_pos = len - decimals;
        let result = format!("{}.{}", &value[..split_pos], &value[split_pos..]);
        if is_negative { format!("-{}", result) } else { result }
    }
}

/// Parse a decimal string to an integer string with the given decimal places
fn parse_units_to_string(value: &str, decimals: usize) -> String {
    let is_negative = value.starts_with('-');
    let value = if is_negative { &value[1..] } else { value };

    // Split on decimal point
    let parts: Vec<&str> = value.split('.').collect();
    let integer_part = parts[0];
    let decimal_part = if parts.len() > 1 { parts[1] } else { "" };

    // Build the result by padding or truncating decimal part
    let decimal_len = decimal_part.len();
    let result = if decimal_len == decimals {
        format!("{}{}", integer_part, decimal_part)
    } else if decimal_len < decimals {
        // Pad with zeros
        format!("{}{}{}", integer_part, decimal_part, "0".repeat(decimals - decimal_len))
    } else {
        // Truncate (round down)
        format!("{}{}", integer_part, &decimal_part[..decimals])
    };

    // Remove leading zeros (but keep at least one digit)
    let result = result.trim_start_matches('0');
    let result = if result.is_empty() { "0" } else { result };

    if is_negative && result != "0" {
        format!("-{}", result)
    } else {
        result.to_string()
    }
}
