//! BigInt runtime support for Perry
//!
//! Provides 256-bit integer arithmetic for cryptocurrency operations.
//! Uses primitive_types::U256 for the underlying representation.

use std::alloc::{alloc, Layout};

/// BigInt is stored as a heap-allocated U256 (256-bit unsigned integer)
/// Layout: 32 bytes (4 x u64)
#[repr(C)]
pub struct BigIntHeader {
    /// The 256-bit value stored as 4 x u64 in little-endian order
    pub limbs: [u64; 4],
}

/// Create a BigInt from a u64 value
#[no_mangle]
pub extern "C" fn js_bigint_from_u64(value: u64) -> *mut BigIntHeader {
    let layout = Layout::new::<BigIntHeader>();
    unsafe {
        let ptr = alloc(layout) as *mut BigIntHeader;
        if ptr.is_null() {
            panic!("Failed to allocate BigInt");
        }
        (*ptr).limbs = [value, 0, 0, 0];
        ptr
    }
}

/// Create a BigInt from a signed i64 value
#[no_mangle]
pub extern "C" fn js_bigint_from_i64(value: i64) -> *mut BigIntHeader {
    let layout = Layout::new::<BigIntHeader>();
    unsafe {
        let ptr = alloc(layout) as *mut BigIntHeader;
        if ptr.is_null() {
            panic!("Failed to allocate BigInt");
        }
        if value >= 0 {
            (*ptr).limbs = [value as u64, 0, 0, 0];
        } else {
            // Two's complement for negative numbers
            (*ptr).limbs = [value as u64, u64::MAX, u64::MAX, u64::MAX];
        }
        ptr
    }
}

/// Create a BigInt from a string (decimal or hex with 0x prefix)
#[no_mangle]
pub extern "C" fn js_bigint_from_string(data: *const u8, len: u32) -> *mut BigIntHeader {
    let layout = Layout::new::<BigIntHeader>();
    unsafe {
        let ptr = alloc(layout) as *mut BigIntHeader;
        if ptr.is_null() {
            panic!("Failed to allocate BigInt");
        }

        let bytes = std::slice::from_raw_parts(data, len as usize);
        let s = std::str::from_utf8_unchecked(bytes);

        // Parse the string
        let (is_hex, s) = if s.starts_with("0x") || s.starts_with("0X") {
            (true, &s[2..])
        } else {
            (false, s)
        };

        let mut limbs = [0u64; 4];

        if is_hex {
            // Parse hex string
            let mut chars = s.chars().rev();
            for limb in limbs.iter_mut() {
                let mut value = 0u64;
                for i in 0..16 {
                    if let Some(c) = chars.next() {
                        let digit = match c {
                            '0'..='9' => c as u64 - '0' as u64,
                            'a'..='f' => c as u64 - 'a' as u64 + 10,
                            'A'..='F' => c as u64 - 'A' as u64 + 10,
                            _ => continue,
                        };
                        value |= digit << (i * 4);
                    } else {
                        break;
                    }
                }
                *limb = value;
            }
        } else {
            // Parse decimal string using long multiplication
            for c in s.chars() {
                if let Some(digit) = c.to_digit(10) {
                    // Multiply by 10 and add digit
                    let mut carry = digit as u64;
                    for limb in limbs.iter_mut() {
                        let product = (*limb as u128) * 10 + carry as u128;
                        *limb = product as u64;
                        carry = (product >> 64) as u64;
                    }
                }
            }
        }

        (*ptr).limbs = limbs;
        ptr
    }
}

/// Add two BigInts
#[no_mangle]
pub extern "C" fn js_bigint_add(a: *const BigIntHeader, b: *const BigIntHeader) -> *mut BigIntHeader {
    let layout = Layout::new::<BigIntHeader>();
    unsafe {
        let ptr = alloc(layout) as *mut BigIntHeader;
        if ptr.is_null() {
            panic!("Failed to allocate BigInt");
        }

        let a_limbs = (*a).limbs;
        let b_limbs = (*b).limbs;
        let mut result = [0u64; 4];
        let mut carry = 0u64;

        for i in 0..4 {
            let sum = (a_limbs[i] as u128) + (b_limbs[i] as u128) + (carry as u128);
            result[i] = sum as u64;
            carry = (sum >> 64) as u64;
        }

        (*ptr).limbs = result;
        ptr
    }
}

/// Subtract two BigInts (a - b)
#[no_mangle]
pub extern "C" fn js_bigint_sub(a: *const BigIntHeader, b: *const BigIntHeader) -> *mut BigIntHeader {
    let layout = Layout::new::<BigIntHeader>();
    unsafe {
        let ptr = alloc(layout) as *mut BigIntHeader;
        if ptr.is_null() {
            panic!("Failed to allocate BigInt");
        }

        let a_limbs = (*a).limbs;
        let b_limbs = (*b).limbs;
        let mut result = [0u64; 4];
        let mut borrow = 0i128;

        for i in 0..4 {
            let diff = (a_limbs[i] as i128) - (b_limbs[i] as i128) - borrow;
            if diff < 0 {
                result[i] = (diff + (1i128 << 64)) as u64;
                borrow = 1;
            } else {
                result[i] = diff as u64;
                borrow = 0;
            }
        }

        (*ptr).limbs = result;
        ptr
    }
}

/// Multiply two BigInts
#[no_mangle]
pub extern "C" fn js_bigint_mul(a: *const BigIntHeader, b: *const BigIntHeader) -> *mut BigIntHeader {
    let layout = Layout::new::<BigIntHeader>();
    unsafe {
        let ptr = alloc(layout) as *mut BigIntHeader;
        if ptr.is_null() {
            panic!("Failed to allocate BigInt");
        }

        let a_limbs = (*a).limbs;
        let b_limbs = (*b).limbs;
        let mut result = [0u64; 4];

        // School multiplication (only keeping lower 256 bits)
        for i in 0..4 {
            let mut carry = 0u128;
            for j in 0..(4 - i) {
                let product = (a_limbs[i] as u128) * (b_limbs[j] as u128)
                    + (result[i + j] as u128)
                    + carry;
                result[i + j] = product as u64;
                carry = product >> 64;
            }
        }

        (*ptr).limbs = result;
        ptr
    }
}

/// Divide two BigInts (a / b)
#[no_mangle]
pub extern "C" fn js_bigint_div(a: *const BigIntHeader, b: *const BigIntHeader) -> *mut BigIntHeader {
    let layout = Layout::new::<BigIntHeader>();
    unsafe {
        let ptr = alloc(layout) as *mut BigIntHeader;
        if ptr.is_null() {
            panic!("Failed to allocate BigInt");
        }

        let a_limbs = (*a).limbs;
        let b_limbs = (*b).limbs;

        // Check for division by zero
        if b_limbs == [0, 0, 0, 0] {
            panic!("Division by zero");
        }

        // Simple binary division
        let mut quotient = [0u64; 4];
        let mut remainder = [0u64; 4];

        for i in (0..256).rev() {
            // Shift remainder left by 1
            let mut carry = 0u64;
            for limb in remainder.iter_mut() {
                let new_carry = *limb >> 63;
                *limb = (*limb << 1) | carry;
                carry = new_carry;
            }

            // Set LSB of remainder from dividend
            let limb_idx = i / 64;
            let bit_idx = i % 64;
            remainder[0] |= (a_limbs[limb_idx] >> bit_idx) & 1;

            // If remainder >= divisor, subtract and set quotient bit
            if compare_limbs(&remainder, &b_limbs) >= 0 {
                subtract_limbs(&mut remainder, &b_limbs);
                let q_limb_idx = i / 64;
                let q_bit_idx = i % 64;
                quotient[q_limb_idx] |= 1u64 << q_bit_idx;
            }
        }

        (*ptr).limbs = quotient;
        ptr
    }
}

/// Modulo of two BigInts (a % b)
#[no_mangle]
pub extern "C" fn js_bigint_mod(a: *const BigIntHeader, b: *const BigIntHeader) -> *mut BigIntHeader {
    let layout = Layout::new::<BigIntHeader>();
    unsafe {
        let ptr = alloc(layout) as *mut BigIntHeader;
        if ptr.is_null() {
            panic!("Failed to allocate BigInt");
        }

        let a_limbs = (*a).limbs;
        let b_limbs = (*b).limbs;

        // Check for division by zero
        if b_limbs == [0, 0, 0, 0] {
            panic!("Division by zero");
        }

        // Simple binary division, return remainder
        let mut remainder = [0u64; 4];

        for i in (0..256).rev() {
            // Shift remainder left by 1
            let mut carry = 0u64;
            for limb in remainder.iter_mut() {
                let new_carry = *limb >> 63;
                *limb = (*limb << 1) | carry;
                carry = new_carry;
            }

            // Set LSB of remainder from dividend
            let limb_idx = i / 64;
            let bit_idx = i % 64;
            remainder[0] |= (a_limbs[limb_idx] >> bit_idx) & 1;

            // If remainder >= divisor, subtract
            if compare_limbs(&remainder, &b_limbs) >= 0 {
                subtract_limbs(&mut remainder, &b_limbs);
            }
        }

        (*ptr).limbs = remainder;
        ptr
    }
}

/// Power of two BigInts (a ** b) using binary exponentiation
/// Note: b is interpreted as a u64 (only lower 64 bits are used)
#[no_mangle]
pub extern "C" fn js_bigint_pow(a: *const BigIntHeader, b: *const BigIntHeader) -> *mut BigIntHeader {
    let layout = Layout::new::<BigIntHeader>();
    unsafe {
        let ptr = alloc(layout) as *mut BigIntHeader;
        if ptr.is_null() {
            panic!("Failed to allocate BigInt");
        }

        // Get exponent as u64 (only lower 64 bits)
        let exp = (*b).limbs[0];

        if exp == 0 {
            // Anything to the power of 0 is 1
            (*ptr).limbs = [1, 0, 0, 0];
            return ptr;
        }

        // Binary exponentiation
        let mut result = [1u64, 0, 0, 0];
        let mut base = (*a).limbs;
        let mut e = exp;

        while e > 0 {
            if e & 1 == 1 {
                result = mul_limbs(&result, &base);
            }
            base = mul_limbs(&base, &base);
            e >>= 1;
        }

        (*ptr).limbs = result;
        ptr
    }
}

/// Multiply two limb arrays (helper for pow)
fn mul_limbs(a: &[u64; 4], b: &[u64; 4]) -> [u64; 4] {
    let mut result = [0u64; 4];
    for i in 0..4 {
        let mut carry = 0u128;
        for j in 0..(4 - i) {
            let product = (a[i] as u128) * (b[j] as u128)
                + (result[i + j] as u128)
                + carry;
            result[i + j] = product as u64;
            carry = product >> 64;
        }
    }
    result
}

/// Left shift BigInt by b bits (a << b)
/// Note: b is interpreted as a u64 (only lower 64 bits are used)
#[no_mangle]
pub extern "C" fn js_bigint_shl(a: *const BigIntHeader, b: *const BigIntHeader) -> *mut BigIntHeader {
    let layout = Layout::new::<BigIntHeader>();
    unsafe {
        let ptr = alloc(layout) as *mut BigIntHeader;
        if ptr.is_null() {
            panic!("Failed to allocate BigInt");
        }

        let shift = (*b).limbs[0] as usize;
        if shift >= 256 {
            // Shift by 256 or more bits results in zero
            (*ptr).limbs = [0, 0, 0, 0];
            return ptr;
        }

        let a_limbs = (*a).limbs;
        let mut result = [0u64; 4];

        // Calculate full limb shifts and bit shifts within a limb
        let limb_shift = shift / 64;
        let bit_shift = (shift % 64) as u32;

        if bit_shift == 0 {
            // Simple case: only limb-aligned shift
            for i in limb_shift..4 {
                result[i] = a_limbs[i - limb_shift];
            }
        } else {
            // General case: shift across limb boundaries
            for i in limb_shift..4 {
                let src_idx = i - limb_shift;
                result[i] |= a_limbs[src_idx] << bit_shift;
                if src_idx > 0 && i > limb_shift {
                    result[i] |= a_limbs[src_idx - 1] >> (64 - bit_shift);
                }
            }
            // Handle carry from lower limb into higher position
            if limb_shift < 4 {
                for i in (limb_shift + 1)..4 {
                    let src_idx = i - limb_shift - 1;
                    result[i] |= a_limbs[src_idx] >> (64 - bit_shift);
                }
            }
        }

        (*ptr).limbs = result;
        ptr
    }
}

/// Right shift BigInt by b bits (a >> b)
/// Note: b is interpreted as a u64 (only lower 64 bits are used)
#[no_mangle]
pub extern "C" fn js_bigint_shr(a: *const BigIntHeader, b: *const BigIntHeader) -> *mut BigIntHeader {
    let layout = Layout::new::<BigIntHeader>();
    unsafe {
        let ptr = alloc(layout) as *mut BigIntHeader;
        if ptr.is_null() {
            panic!("Failed to allocate BigInt");
        }

        let shift = (*b).limbs[0] as usize;
        if shift >= 256 {
            // Shift by 256 or more bits results in zero
            (*ptr).limbs = [0, 0, 0, 0];
            return ptr;
        }

        let a_limbs = (*a).limbs;
        let mut result = [0u64; 4];

        // Calculate full limb shifts and bit shifts within a limb
        let limb_shift = shift / 64;
        let bit_shift = (shift % 64) as u32;

        if bit_shift == 0 {
            // Simple case: only limb-aligned shift
            for i in 0..(4 - limb_shift) {
                result[i] = a_limbs[i + limb_shift];
            }
        } else {
            // General case: shift across limb boundaries
            for i in 0..(4 - limb_shift) {
                let src_idx = i + limb_shift;
                result[i] |= a_limbs[src_idx] >> bit_shift;
                if src_idx + 1 < 4 {
                    result[i] |= a_limbs[src_idx + 1] << (64 - bit_shift);
                }
            }
        }

        (*ptr).limbs = result;
        ptr
    }
}

/// Bitwise AND of two BigInts (a & b)
#[no_mangle]
pub extern "C" fn js_bigint_and(a: *const BigIntHeader, b: *const BigIntHeader) -> *mut BigIntHeader {
    let layout = Layout::new::<BigIntHeader>();
    unsafe {
        let ptr = alloc(layout) as *mut BigIntHeader;
        if ptr.is_null() {
            panic!("Failed to allocate BigInt");
        }

        let a_limbs = (*a).limbs;
        let b_limbs = (*b).limbs;
        let mut result = [0u64; 4];

        for i in 0..4 {
            result[i] = a_limbs[i] & b_limbs[i];
        }

        (*ptr).limbs = result;
        ptr
    }
}

/// Bitwise OR of two BigInts (a | b)
#[no_mangle]
pub extern "C" fn js_bigint_or(a: *const BigIntHeader, b: *const BigIntHeader) -> *mut BigIntHeader {
    let layout = Layout::new::<BigIntHeader>();
    unsafe {
        let ptr = alloc(layout) as *mut BigIntHeader;
        if ptr.is_null() {
            panic!("Failed to allocate BigInt");
        }

        let a_limbs = (*a).limbs;
        let b_limbs = (*b).limbs;
        let mut result = [0u64; 4];

        for i in 0..4 {
            result[i] = a_limbs[i] | b_limbs[i];
        }

        (*ptr).limbs = result;
        ptr
    }
}

/// Bitwise XOR of two BigInts (a ^ b)
#[no_mangle]
pub extern "C" fn js_bigint_xor(a: *const BigIntHeader, b: *const BigIntHeader) -> *mut BigIntHeader {
    let layout = Layout::new::<BigIntHeader>();
    unsafe {
        let ptr = alloc(layout) as *mut BigIntHeader;
        if ptr.is_null() {
            panic!("Failed to allocate BigInt");
        }

        let a_limbs = (*a).limbs;
        let b_limbs = (*b).limbs;
        let mut result = [0u64; 4];

        for i in 0..4 {
            result[i] = a_limbs[i] ^ b_limbs[i];
        }

        (*ptr).limbs = result;
        ptr
    }
}

/// Compare two BigInts (-1 if a < b, 0 if equal, 1 if a > b)
#[no_mangle]
pub extern "C" fn js_bigint_cmp(a: *const BigIntHeader, b: *const BigIntHeader) -> i32 {
    unsafe {
        compare_limbs(&(*a).limbs, &(*b).limbs)
    }
}

/// Check if two BigInts are equal
#[no_mangle]
pub extern "C" fn js_bigint_eq(a: *const BigIntHeader, b: *const BigIntHeader) -> bool {
    unsafe {
        (*a).limbs == (*b).limbs
    }
}

/// Convert BigInt to f64 (may lose precision)
#[no_mangle]
pub extern "C" fn js_bigint_to_f64(a: *const BigIntHeader) -> f64 {
    unsafe {
        let limbs = (*a).limbs;
        let mut result = 0.0f64;
        let mut multiplier = 1.0f64;
        for limb in limbs.iter() {
            result += (*limb as f64) * multiplier;
            multiplier *= 18446744073709551616.0; // 2^64
        }
        result
    }
}

/// Convert BigInt to string
#[no_mangle]
pub extern "C" fn js_bigint_to_string(a: *const BigIntHeader) -> *mut crate::string::StringHeader {
    unsafe {
        let limbs = (*a).limbs;

        // Convert to decimal string
        let mut digits = Vec::new();
        let mut temp = limbs;

        // Check if zero
        if temp == [0, 0, 0, 0] {
            return crate::string::js_string_from_bytes("0".as_ptr(), 1);
        }

        while temp != [0, 0, 0, 0] {
            let mut remainder = 0u128;
            for i in (0..4).rev() {
                let dividend = (remainder << 64) + temp[i] as u128;
                temp[i] = (dividend / 10) as u64;
                remainder = dividend % 10;
            }
            digits.push((remainder as u8 + b'0') as char);
        }

        digits.reverse();
        let s: String = digits.into_iter().collect();
        crate::string::js_string_from_bytes(s.as_ptr(), s.len() as u32)
    }
}

/// Print BigInt to stdout (for debugging)
#[no_mangle]
pub extern "C" fn js_bigint_print(a: *const BigIntHeader) {
    unsafe {
        let limbs = (*a).limbs;

        // Convert to decimal string
        let mut digits = Vec::new();
        let mut temp = limbs;

        // Check if zero
        if temp == [0, 0, 0, 0] {
            println!("0n");
            return;
        }

        while temp != [0, 0, 0, 0] {
            let mut remainder = 0u128;
            for i in (0..4).rev() {
                let dividend = (remainder << 64) + temp[i] as u128;
                temp[i] = (dividend / 10) as u64;
                remainder = dividend % 10;
            }
            digits.push((remainder as u8 + b'0') as char);
        }

        digits.reverse();
        let s: String = digits.into_iter().collect();
        println!("{}n", s);
    }
}

/// Print BigInt to stderr (console.error)
#[no_mangle]
pub extern "C" fn js_bigint_error(a: *const BigIntHeader) {
    unsafe {
        let limbs = (*a).limbs;

        // Convert to decimal string
        let mut digits = Vec::new();
        let mut temp = limbs;

        // Check if zero
        if temp == [0, 0, 0, 0] {
            eprintln!("0n");
            return;
        }

        while temp != [0, 0, 0, 0] {
            let mut remainder = 0u128;
            for i in (0..4).rev() {
                let dividend = (remainder << 64) + temp[i] as u128;
                temp[i] = (dividend / 10) as u64;
                remainder = dividend % 10;
            }
            digits.push((remainder as u8 + b'0') as char);
        }

        digits.reverse();
        let s: String = digits.into_iter().collect();
        eprintln!("{}n", s);
    }
}

/// Print BigInt to stderr (console.warn)
#[no_mangle]
pub extern "C" fn js_bigint_warn(a: *const BigIntHeader) {
    unsafe {
        let limbs = (*a).limbs;

        // Convert to decimal string
        let mut digits = Vec::new();
        let mut temp = limbs;

        // Check if zero
        if temp == [0, 0, 0, 0] {
            eprintln!("0n");
            return;
        }

        while temp != [0, 0, 0, 0] {
            let mut remainder = 0u128;
            for i in (0..4).rev() {
                let dividend = (remainder << 64) + temp[i] as u128;
                temp[i] = (dividend / 10) as u64;
                remainder = dividend % 10;
            }
            digits.push((remainder as u8 + b'0') as char);
        }

        digits.reverse();
        let s: String = digits.into_iter().collect();
        eprintln!("{}n", s);
    }
}

// Helper functions

fn compare_limbs(a: &[u64; 4], b: &[u64; 4]) -> i32 {
    for i in (0..4).rev() {
        if a[i] > b[i] {
            return 1;
        }
        if a[i] < b[i] {
            return -1;
        }
    }
    0
}

fn subtract_limbs(a: &mut [u64; 4], b: &[u64; 4]) {
    let mut borrow = 0i128;
    for i in 0..4 {
        let diff = (a[i] as i128) - (b[i] as i128) - borrow;
        if diff < 0 {
            a[i] = (diff + (1i128 << 64)) as u64;
            borrow = 1;
        } else {
            a[i] = diff as u64;
            borrow = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bigint_from_u64() {
        let bi = js_bigint_from_u64(12345);
        unsafe {
            assert_eq!((*bi).limbs[0], 12345);
            assert_eq!((*bi).limbs[1], 0);
        }
    }

    #[test]
    fn test_bigint_add() {
        let a = js_bigint_from_u64(100);
        let b = js_bigint_from_u64(200);
        let c = js_bigint_add(a, b);
        unsafe {
            assert_eq!((*c).limbs[0], 300);
        }
    }

    #[test]
    fn test_bigint_mul() {
        let a = js_bigint_from_u64(1000);
        let b = js_bigint_from_u64(2000);
        let c = js_bigint_mul(a, b);
        unsafe {
            assert_eq!((*c).limbs[0], 2_000_000);
        }
    }

    #[test]
    fn test_bigint_from_string() {
        let s = "123456789";
        let bi = js_bigint_from_string(s.as_ptr(), s.len() as u32);
        unsafe {
            assert_eq!((*bi).limbs[0], 123456789);
        }
    }

    #[test]
    fn test_bigint_from_hex() {
        let s = "0xFFFFFFFFFFFFFFFF"; // max u64
        let bi = js_bigint_from_string(s.as_ptr(), s.len() as u32);
        unsafe {
            assert_eq!((*bi).limbs[0], u64::MAX);
            assert_eq!((*bi).limbs[1], 0);
        }
    }
}
