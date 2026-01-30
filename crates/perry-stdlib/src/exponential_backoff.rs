//! Exponential Backoff implementation
//!
//! Native implementation of the `exponential-backoff` npm package.
//! Provides retry functionality with exponential delays.

use perry_runtime::{
    js_promise_new, js_promise_resolve, js_promise_reject, Promise,
    ClosureHeader, js_closure_call0,
};
use std::time::Duration;
use std::thread;

/// Execute a function with exponential backoff retry logic
/// fn_ptr: Closure to execute (should return a Promise)
/// options_ptr: Object containing options (numOfAttempts, startingDelay, etc.)
/// Returns: Promise that resolves with the result or rejects after all retries fail
#[no_mangle]
pub extern "C" fn backOff(
    fn_ptr: *const ClosureHeader,
    _options_ptr: *const perry_runtime::ObjectHeader,
) -> *mut Promise {
    let promise = unsafe { js_promise_new() };

    if fn_ptr.is_null() {
        unsafe {
            js_promise_reject(promise, f64::NAN);
        }
        return promise;
    }

    // Default backoff options
    let num_of_attempts: u32 = 3;
    let starting_delay: u64 = 100;
    let max_delay: u64 = 10000;
    let time_multiple: f64 = 2.0;

    // TODO: Parse options from options_ptr if provided

    let mut attempt = 0;
    let mut current_delay = starting_delay;

    loop {
        attempt += 1;

        // Call the function
        let result = unsafe { js_closure_call0(fn_ptr) };

        // For simplicity, assume the function succeeds if result is not NaN
        // In a real implementation, we'd need to handle Promise results
        if !result.is_nan() {
            unsafe {
                js_promise_resolve(promise, result);
            }
            return promise;
        }

        // Check if we've exhausted retries
        if attempt >= num_of_attempts {
            unsafe {
                js_promise_reject(promise, f64::NAN);
            }
            return promise;
        }

        // Wait before retrying
        thread::sleep(Duration::from_millis(current_delay));

        // Increase delay exponentially
        current_delay = ((current_delay as f64) * time_multiple).min(max_delay as f64) as u64;
    }
}

/// Simplified backOff that takes just the function and retry count
#[no_mangle]
pub extern "C" fn js_backoff_simple(
    fn_ptr: *const ClosureHeader,
    num_attempts: i32,
    delay_ms: i32,
) -> f64 {
    if fn_ptr.is_null() {
        return f64::NAN;
    }

    let mut attempt = 0;
    let mut current_delay = delay_ms.max(10) as u64;

    loop {
        attempt += 1;

        // Call the function
        let result = unsafe { js_closure_call0(fn_ptr) };

        // Success if not NaN
        if !result.is_nan() {
            return result;
        }

        // Check if we've exhausted retries
        if attempt >= num_attempts {
            return f64::NAN;
        }

        // Wait before retrying
        thread::sleep(Duration::from_millis(current_delay));

        // Increase delay exponentially
        current_delay = (current_delay * 2).min(10000);
    }
}
