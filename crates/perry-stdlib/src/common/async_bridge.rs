//! Async bridge: connects Rust async (tokio) with the perry Promise system.
//!
//! The perry runtime has a Promise implementation that expects synchronous
//! resolution callbacks. We need to bridge this with tokio's async runtime
//! for database operations.

use std::future::Future;
use std::sync::Mutex;

use once_cell::sync::Lazy;
use tokio::runtime::Runtime;

/// Global tokio runtime for all async stdlib operations
pub static RUNTIME: Lazy<Runtime> = Lazy::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .expect("Failed to create tokio runtime")
});

/// Pending promise resolutions
/// Format: (promise_ptr, is_success, result_value)
static PENDING_RESOLUTIONS: Lazy<Mutex<Vec<PendingResolution>>> =
    Lazy::new(|| Mutex::new(Vec::new()));

/// A pending promise resolution
struct PendingResolution {
    /// Pointer to the Promise object (as usize for Send)
    promise_ptr: usize,
    /// True if resolved successfully, false if rejected
    is_success: bool,
    /// The result value (as u64 bits for JSValue)
    result_bits: u64,
}

/// Get a reference to the global runtime
pub fn runtime() -> &'static Runtime {
    &RUNTIME
}

/// Spawn an async task on the global runtime
pub fn spawn<F>(future: F)
where
    F: Future<Output = ()> + Send + 'static,
{
    RUNTIME.spawn(future);
}

/// Block on an async task (use sparingly, mainly for initialization)
pub fn block_on<F, T>(future: F) -> T
where
    F: Future<Output = T>,
{
    RUNTIME.block_on(future)
}

/// Queue a promise resolution to be processed later
pub fn queue_promise_resolution(promise_ptr: usize, is_success: bool, result_bits: u64) {
    let mut pending = PENDING_RESOLUTIONS.lock().unwrap();
    pending.push(PendingResolution {
        promise_ptr,
        is_success,
        result_bits,
    });
}

/// Process all pending promise resolutions
///
/// This should be called from the main event loop to process async completions.
/// Returns the number of resolutions processed.
#[no_mangle]
pub extern "C" fn js_stdlib_process_pending() -> i32 {
    let mut pending = PENDING_RESOLUTIONS.lock().unwrap();
    let count = pending.len() as i32;

    for resolution in pending.drain(..) {
        let promise_ptr = resolution.promise_ptr as *mut perry_runtime::Promise;
        if resolution.is_success {
            // Call js_promise_resolve
            perry_runtime::js_promise_resolve(
                promise_ptr,
                f64::from_bits(resolution.result_bits),
            );
        } else {
            // Call js_promise_reject
            perry_runtime::js_promise_reject(
                promise_ptr,
                f64::from_bits(resolution.result_bits),
            );
        }
    }

    count
}

/// Spawn an async operation that will resolve a Promise when complete
///
/// # Safety
/// The promise_ptr must be a valid pointer to a Promise object
pub unsafe fn spawn_for_promise<F>(promise_ptr: *mut u8, future: F)
where
    F: Future<Output = Result<u64, String>> + Send + 'static,
{
    let ptr = promise_ptr as usize; // Convert to usize for Send

    RUNTIME.spawn(async move {
        match future.await {
            Ok(result_bits) => {
                queue_promise_resolution(ptr, true, result_bits);
            }
            Err(error_msg) => {
                // Create an error string and get its bits
                let error_bits = create_error_value(&error_msg);
                queue_promise_resolution(ptr, false, error_bits);
            }
        }
    });
}

/// Create a JSValue representing an error from a string message
fn create_error_value(msg: &str) -> u64 {
    let str_ptr = perry_runtime::js_string_from_bytes(msg.as_ptr(), msg.len() as u32);
    // Return the pointer as bits
    // In a full implementation, we'd wrap this in an Error object
    perry_runtime::JSValue::pointer(str_ptr as *const u8).bits()
}
