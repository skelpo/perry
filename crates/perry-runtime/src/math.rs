//! Math operations runtime support

use rand::Rng;

/// Math.pow(base, exponent) -> number
#[no_mangle]
pub extern "C" fn js_math_pow(base: f64, exp: f64) -> f64 {
    base.powf(exp)
}

/// Floating-point modulo using the C library's fmod
/// This is often faster than the inline computation a - trunc(a/b) * b
#[no_mangle]
pub extern "C" fn js_math_fmod(a: f64, b: f64) -> f64 {
    a % b  // Rust's % operator maps to libm fmod
}

/// Math.random() -> number (0 <= x < 1)
#[no_mangle]
pub extern "C" fn js_math_random() -> f64 {
    let mut rng = rand::thread_rng();
    rng.gen::<f64>()
}
