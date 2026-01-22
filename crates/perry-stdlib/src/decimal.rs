//! Decimal/Big.js implementation
//!
//! Native implementation of Big.js and Decimal.js for arbitrary precision math.
//! Uses Rust's rust_decimal crate for precise decimal arithmetic.

use perry_runtime::{js_string_from_bytes, StringHeader};
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

use crate::common::{get_handle_mut, register_handle, Handle};

/// DecimalHandle stores a Decimal value
pub struct DecimalHandle {
    value: Decimal,
}

impl DecimalHandle {
    pub fn new(value: Decimal) -> Self {
        DecimalHandle { value }
    }
}

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

/// Create a new Decimal from a number
#[no_mangle]
pub extern "C" fn js_decimal_from_number(value: f64) -> Handle {
    let decimal = Decimal::from_f64(value).unwrap_or(Decimal::ZERO);
    register_handle(DecimalHandle::new(decimal))
}

/// Create a new Decimal from a string
#[no_mangle]
pub unsafe extern "C" fn js_decimal_from_string(value_ptr: *const StringHeader) -> Handle {
    let value_str = match string_from_header(value_ptr) {
        Some(s) => s,
        None => return register_handle(DecimalHandle::new(Decimal::ZERO)),
    };

    let decimal = Decimal::from_str(&value_str).unwrap_or(Decimal::ZERO);
    register_handle(DecimalHandle::new(decimal))
}

/// Decimal.plus(other) - Addition
#[no_mangle]
pub extern "C" fn js_decimal_plus(handle: Handle, other: Handle) -> Handle {
    let a = get_handle_mut::<DecimalHandle>(handle).map(|h| h.value).unwrap_or(Decimal::ZERO);
    let b = get_handle_mut::<DecimalHandle>(other).map(|h| h.value).unwrap_or(Decimal::ZERO);
    register_handle(DecimalHandle::new(a + b))
}

/// Decimal.plus with number
#[no_mangle]
pub extern "C" fn js_decimal_plus_number(handle: Handle, other: f64) -> Handle {
    let a = get_handle_mut::<DecimalHandle>(handle).map(|h| h.value).unwrap_or(Decimal::ZERO);
    let b = Decimal::from_f64(other).unwrap_or(Decimal::ZERO);
    register_handle(DecimalHandle::new(a + b))
}

/// Decimal.minus(other) - Subtraction
#[no_mangle]
pub extern "C" fn js_decimal_minus(handle: Handle, other: Handle) -> Handle {
    let a = get_handle_mut::<DecimalHandle>(handle).map(|h| h.value).unwrap_or(Decimal::ZERO);
    let b = get_handle_mut::<DecimalHandle>(other).map(|h| h.value).unwrap_or(Decimal::ZERO);
    register_handle(DecimalHandle::new(a - b))
}

/// Decimal.minus with number
#[no_mangle]
pub extern "C" fn js_decimal_minus_number(handle: Handle, other: f64) -> Handle {
    let a = get_handle_mut::<DecimalHandle>(handle).map(|h| h.value).unwrap_or(Decimal::ZERO);
    let b = Decimal::from_f64(other).unwrap_or(Decimal::ZERO);
    register_handle(DecimalHandle::new(a - b))
}

/// Decimal.times(other) - Multiplication
#[no_mangle]
pub extern "C" fn js_decimal_times(handle: Handle, other: Handle) -> Handle {
    let a = get_handle_mut::<DecimalHandle>(handle).map(|h| h.value).unwrap_or(Decimal::ZERO);
    let b = get_handle_mut::<DecimalHandle>(other).map(|h| h.value).unwrap_or(Decimal::ZERO);
    register_handle(DecimalHandle::new(a * b))
}

/// Decimal.times with number
#[no_mangle]
pub extern "C" fn js_decimal_times_number(handle: Handle, other: f64) -> Handle {
    let a = get_handle_mut::<DecimalHandle>(handle).map(|h| h.value).unwrap_or(Decimal::ZERO);
    let b = Decimal::from_f64(other).unwrap_or(Decimal::ZERO);
    register_handle(DecimalHandle::new(a * b))
}

/// Decimal.div(other) - Division
#[no_mangle]
pub extern "C" fn js_decimal_div(handle: Handle, other: Handle) -> Handle {
    let a = get_handle_mut::<DecimalHandle>(handle).map(|h| h.value).unwrap_or(Decimal::ZERO);
    let b = get_handle_mut::<DecimalHandle>(other).map(|h| h.value).unwrap_or(Decimal::ONE);

    if b.is_zero() {
        return register_handle(DecimalHandle::new(Decimal::ZERO));
    }

    register_handle(DecimalHandle::new(a / b))
}

/// Decimal.div with number
#[no_mangle]
pub extern "C" fn js_decimal_div_number(handle: Handle, other: f64) -> Handle {
    let a = get_handle_mut::<DecimalHandle>(handle).map(|h| h.value).unwrap_or(Decimal::ZERO);
    let b = Decimal::from_f64(other).unwrap_or(Decimal::ONE);

    if b.is_zero() {
        return register_handle(DecimalHandle::new(Decimal::ZERO));
    }

    register_handle(DecimalHandle::new(a / b))
}

/// Decimal.mod(other) - Modulo
#[no_mangle]
pub extern "C" fn js_decimal_mod(handle: Handle, other: Handle) -> Handle {
    let a = get_handle_mut::<DecimalHandle>(handle).map(|h| h.value).unwrap_or(Decimal::ZERO);
    let b = get_handle_mut::<DecimalHandle>(other).map(|h| h.value).unwrap_or(Decimal::ONE);

    if b.is_zero() {
        return register_handle(DecimalHandle::new(Decimal::ZERO));
    }

    register_handle(DecimalHandle::new(a % b))
}

/// Decimal.pow(n) - Power
#[no_mangle]
pub extern "C" fn js_decimal_pow(handle: Handle, n: f64) -> Handle {
    let a = get_handle_mut::<DecimalHandle>(handle).map(|h| h.value).unwrap_or(Decimal::ZERO);
    let exp = Decimal::from_f64(n).unwrap_or(Decimal::ZERO);

    // Use checked_powd with Decimal exponent
    let result = a.checked_powd(exp).unwrap_or(Decimal::ZERO);
    register_handle(DecimalHandle::new(result))
}

/// Decimal.sqrt() - Square root
#[no_mangle]
pub extern "C" fn js_decimal_sqrt(handle: Handle) -> Handle {
    let a = get_handle_mut::<DecimalHandle>(handle).map(|h| h.value).unwrap_or(Decimal::ZERO);

    // Use the sqrt method from rust_decimal with maths feature
    let result = a.sqrt().unwrap_or(Decimal::ZERO);
    register_handle(DecimalHandle::new(result))
}

/// Decimal.abs() - Absolute value
#[no_mangle]
pub extern "C" fn js_decimal_abs(handle: Handle) -> Handle {
    let a = get_handle_mut::<DecimalHandle>(handle).map(|h| h.value).unwrap_or(Decimal::ZERO);
    register_handle(DecimalHandle::new(a.abs()))
}

/// Decimal.neg() - Negation
#[no_mangle]
pub extern "C" fn js_decimal_neg(handle: Handle) -> Handle {
    let a = get_handle_mut::<DecimalHandle>(handle).map(|h| h.value).unwrap_or(Decimal::ZERO);
    register_handle(DecimalHandle::new(-a))
}

/// Decimal.round() - Round to nearest integer
#[no_mangle]
pub extern "C" fn js_decimal_round(handle: Handle) -> Handle {
    let a = get_handle_mut::<DecimalHandle>(handle).map(|h| h.value).unwrap_or(Decimal::ZERO);
    register_handle(DecimalHandle::new(a.round()))
}

/// Decimal.floor() - Round down
#[no_mangle]
pub extern "C" fn js_decimal_floor(handle: Handle) -> Handle {
    let a = get_handle_mut::<DecimalHandle>(handle).map(|h| h.value).unwrap_or(Decimal::ZERO);
    register_handle(DecimalHandle::new(a.floor()))
}

/// Decimal.ceil() - Round up
#[no_mangle]
pub extern "C" fn js_decimal_ceil(handle: Handle) -> Handle {
    let a = get_handle_mut::<DecimalHandle>(handle).map(|h| h.value).unwrap_or(Decimal::ZERO);
    register_handle(DecimalHandle::new(a.ceil()))
}

/// Decimal.toFixed(decimals) - Format with fixed decimal places
#[no_mangle]
pub extern "C" fn js_decimal_to_fixed(handle: Handle, decimals: f64) -> *const StringHeader {
    let a = get_handle_mut::<DecimalHandle>(handle).map(|h| h.value).unwrap_or(Decimal::ZERO);
    let dp = decimals as u32;

    let rounded = a.round_dp(dp);
    let result = format!("{:.1$}", rounded, dp as usize);

    unsafe { js_string_from_bytes(result.as_ptr(), result.len() as u32) }
}

/// Decimal.toString() - Convert to string
#[no_mangle]
pub extern "C" fn js_decimal_to_string(handle: Handle) -> *const StringHeader {
    let a = get_handle_mut::<DecimalHandle>(handle).map(|h| h.value).unwrap_or(Decimal::ZERO);
    let result = a.to_string();

    unsafe { js_string_from_bytes(result.as_ptr(), result.len() as u32) }
}

/// Decimal.toNumber() - Convert to number
#[no_mangle]
pub extern "C" fn js_decimal_to_number(handle: Handle) -> f64 {
    let a = get_handle_mut::<DecimalHandle>(handle).map(|h| h.value).unwrap_or(Decimal::ZERO);
    a.to_f64().unwrap_or(0.0)
}

/// Decimal.eq(other) - Equality comparison
#[no_mangle]
pub extern "C" fn js_decimal_eq(handle: Handle, other: Handle) -> f64 {
    let a = get_handle_mut::<DecimalHandle>(handle).map(|h| h.value).unwrap_or(Decimal::ZERO);
    let b = get_handle_mut::<DecimalHandle>(other).map(|h| h.value).unwrap_or(Decimal::ZERO);
    if a == b { 1.0 } else { 0.0 }
}

/// Decimal.lt(other) - Less than
#[no_mangle]
pub extern "C" fn js_decimal_lt(handle: Handle, other: Handle) -> f64 {
    let a = get_handle_mut::<DecimalHandle>(handle).map(|h| h.value).unwrap_or(Decimal::ZERO);
    let b = get_handle_mut::<DecimalHandle>(other).map(|h| h.value).unwrap_or(Decimal::ZERO);
    if a < b { 1.0 } else { 0.0 }
}

/// Decimal.lte(other) - Less than or equal
#[no_mangle]
pub extern "C" fn js_decimal_lte(handle: Handle, other: Handle) -> f64 {
    let a = get_handle_mut::<DecimalHandle>(handle).map(|h| h.value).unwrap_or(Decimal::ZERO);
    let b = get_handle_mut::<DecimalHandle>(other).map(|h| h.value).unwrap_or(Decimal::ZERO);
    if a <= b { 1.0 } else { 0.0 }
}

/// Decimal.gt(other) - Greater than
#[no_mangle]
pub extern "C" fn js_decimal_gt(handle: Handle, other: Handle) -> f64 {
    let a = get_handle_mut::<DecimalHandle>(handle).map(|h| h.value).unwrap_or(Decimal::ZERO);
    let b = get_handle_mut::<DecimalHandle>(other).map(|h| h.value).unwrap_or(Decimal::ZERO);
    if a > b { 1.0 } else { 0.0 }
}

/// Decimal.gte(other) - Greater than or equal
#[no_mangle]
pub extern "C" fn js_decimal_gte(handle: Handle, other: Handle) -> f64 {
    let a = get_handle_mut::<DecimalHandle>(handle).map(|h| h.value).unwrap_or(Decimal::ZERO);
    let b = get_handle_mut::<DecimalHandle>(other).map(|h| h.value).unwrap_or(Decimal::ZERO);
    if a >= b { 1.0 } else { 0.0 }
}

/// Decimal.isZero() - Check if zero
#[no_mangle]
pub extern "C" fn js_decimal_is_zero(handle: Handle) -> f64 {
    let a = get_handle_mut::<DecimalHandle>(handle).map(|h| h.value).unwrap_or(Decimal::ZERO);
    if a.is_zero() { 1.0 } else { 0.0 }
}

/// Decimal.isPositive() - Check if positive
#[no_mangle]
pub extern "C" fn js_decimal_is_positive(handle: Handle) -> f64 {
    let a = get_handle_mut::<DecimalHandle>(handle).map(|h| h.value).unwrap_or(Decimal::ZERO);
    if a.is_sign_positive() && !a.is_zero() { 1.0 } else { 0.0 }
}

/// Decimal.isNegative() - Check if negative
#[no_mangle]
pub extern "C" fn js_decimal_is_negative(handle: Handle) -> f64 {
    let a = get_handle_mut::<DecimalHandle>(handle).map(|h| h.value).unwrap_or(Decimal::ZERO);
    if a.is_sign_negative() { 1.0 } else { 0.0 }
}

/// Decimal.cmp(other) - Compare: -1, 0, or 1
#[no_mangle]
pub extern "C" fn js_decimal_cmp(handle: Handle, other: Handle) -> f64 {
    let a = get_handle_mut::<DecimalHandle>(handle).map(|h| h.value).unwrap_or(Decimal::ZERO);
    let b = get_handle_mut::<DecimalHandle>(other).map(|h| h.value).unwrap_or(Decimal::ZERO);

    match a.cmp(&b) {
        std::cmp::Ordering::Less => -1.0,
        std::cmp::Ordering::Equal => 0.0,
        std::cmp::Ordering::Greater => 1.0,
    }
}
