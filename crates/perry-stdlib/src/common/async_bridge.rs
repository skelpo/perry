//! Async bridge: connects Rust async (tokio) with the perry Promise system.
//!
//! The perry runtime has a Promise implementation that expects synchronous
//! resolution callbacks. We need to bridge this with tokio's async runtime
//! for database operations.
//!
//! IMPORTANT: perry-runtime uses thread-local arenas for memory allocation.
//! This means JSValue objects created on tokio worker threads will be allocated
//! from a different arena than the main thread, causing memory corruption.
//!
//! To avoid this, async operations should:
//! 1. NOT create JSValue objects (arrays, strings, objects) in async blocks
//! 2. Store raw Rust data and use deferred conversion callbacks
//! 3. The conversion callbacks run on the main thread during js_stdlib_process_pending

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

/// Pending deferred resolutions - these store raw data and a conversion function
/// that runs on the main thread to create JSValues safely
static PENDING_DEFERRED: Lazy<Mutex<Vec<DeferredResolution>>> =
    Lazy::new(|| Mutex::new(Vec::new()));

/// A pending promise resolution (for simple values that don't need conversion)
struct PendingResolution {
    /// Pointer to the Promise object (as usize for Send)
    promise_ptr: usize,
    /// True if resolved successfully, false if rejected
    is_success: bool,
    /// The result value (as u64 bits for JSValue)
    result_bits: u64,
}

/// A deferred promise resolution with a conversion callback
/// The converter function runs on the main thread to safely create JSValues
struct DeferredResolution {
    /// Pointer to the Promise object (as usize for Send)
    promise_ptr: usize,
    /// True if resolved successfully, false if rejected
    is_success: bool,
    /// Boxed converter function that creates the JSValue on the main thread
    /// Returns the JSValue bits
    converter: Box<dyn FnOnce() -> u64 + Send>,
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
/// NOTE: Only use this for simple values (numbers, booleans, undefined, null)
/// that don't involve pointer allocations. For complex values like arrays,
/// objects, or strings, use queue_deferred_resolution instead.
pub fn queue_promise_resolution(promise_ptr: usize, is_success: bool, result_bits: u64) {
    let mut pending = PENDING_RESOLUTIONS.lock().unwrap();
    pending.push(PendingResolution {
        promise_ptr,
        is_success,
        result_bits,
    });
}

/// Queue a deferred promise resolution with a conversion callback
/// The converter function will run on the main thread to safely create JSValues
/// using the main thread's arena allocator.
pub fn queue_deferred_resolution<F>(promise_ptr: usize, is_success: bool, converter: F)
where
    F: FnOnce() -> u64 + Send + 'static,
{
    let mut pending = PENDING_DEFERRED.lock().unwrap();
    pending.push(DeferredResolution {
        promise_ptr,
        is_success,
        converter: Box::new(converter),
    });
}

/// Process all pending promise resolutions
///
/// This should be called from the main event loop to process async completions.
/// Returns the number of resolutions processed.
#[no_mangle]
pub extern "C" fn js_stdlib_process_pending() -> i32 {
    let mut count = 0i32;

    // Process simple resolutions first
    {
        let mut pending = PENDING_RESOLUTIONS.lock().unwrap();
        count += pending.len() as i32;

        for resolution in pending.drain(..) {
            let promise_ptr = resolution.promise_ptr as *mut perry_runtime::Promise;
            if resolution.is_success {
                perry_runtime::js_promise_resolve(
                    promise_ptr,
                    f64::from_bits(resolution.result_bits),
                );
            } else {
                perry_runtime::js_promise_reject(
                    promise_ptr,
                    f64::from_bits(resolution.result_bits),
                );
            }
        }
    }

    // Process deferred resolutions - these run converter functions on the main thread
    {
        let mut pending = PENDING_DEFERRED.lock().unwrap();
        let deferred_count = pending.len();
        count += deferred_count as i32;

        for resolution in pending.drain(..) {
            let promise_ptr = resolution.promise_ptr as *mut perry_runtime::Promise;
            // Run the converter on the main thread to create JSValues safely
            let result_bits = (resolution.converter)();

            if resolution.is_success {
                perry_runtime::js_promise_resolve(
                    promise_ptr,
                    f64::from_bits(result_bits),
                );
            } else {
                perry_runtime::js_promise_reject(
                    promise_ptr,
                    f64::from_bits(result_bits),
                );
            }
        }
    }

    count
}

/// Spawn an async operation that will resolve a Promise when complete
///
/// WARNING: This function assumes the returned u64 bits represent a simple value
/// (number, boolean, undefined, null) that doesn't contain heap pointers.
/// For complex values (arrays, objects, strings), use spawn_for_promise_deferred instead.
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
                // Store the error message and create the string on the main thread
                queue_deferred_resolution(ptr, false, move || {
                    let str_ptr = perry_runtime::js_string_from_bytes(
                        error_msg.as_ptr(),
                        error_msg.len() as u32,
                    );
                    // Use string_ptr for proper type identification (STRING_TAG, not POINTER_TAG)
                    perry_runtime::JSValue::string_ptr(str_ptr).bits()
                });
            }
        }
    });
}

/// Spawn an async operation with deferred JSValue creation
///
/// This is the safe way to create complex JSValues (arrays, objects, strings)
/// from async operations. The async block returns raw Rust data, and the
/// converter function creates the JSValue on the main thread.
///
/// # Type Parameters
/// - `T`: The raw data type produced by the async operation (must be Send + 'static)
/// - `F`: The async future type
/// - `C`: The converter function type
///
/// # Arguments
/// - `promise_ptr`: Pointer to the Promise object
/// - `future`: Async future that produces Result<T, String>
/// - `converter`: Function that converts T to JSValue bits (runs on main thread)
///
/// # Safety
/// The promise_ptr must be a valid pointer to a Promise object
pub unsafe fn spawn_for_promise_deferred<T, F, C>(
    promise_ptr: *mut u8,
    future: F,
    converter: C,
)
where
    T: Send + 'static,
    F: Future<Output = Result<T, String>> + Send + 'static,
    C: FnOnce(T) -> u64 + Send + 'static,
{
    let ptr = promise_ptr as usize;

    RUNTIME.spawn(async move {
        match future.await {
            Ok(data) => {
                // Queue deferred resolution with the converter
                queue_deferred_resolution(ptr, true, move || converter(data));
            }
            Err(error_msg) => {
                // Create error string on main thread
                queue_deferred_resolution(ptr, false, move || {
                    let str_ptr = perry_runtime::js_string_from_bytes(
                        error_msg.as_ptr(),
                        error_msg.len() as u32,
                    );
                    // Use string_ptr for proper type identification (STRING_TAG, not POINTER_TAG)
                    perry_runtime::JSValue::string_ptr(str_ptr).bits()
                });
            }
        }
    });
}

/// Create a JSValue representing an error from a string message
/// NOTE: This must only be called from the main thread!
fn create_error_value(msg: &str) -> u64 {
    let str_ptr = perry_runtime::js_string_from_bytes(msg.as_ptr(), msg.len() as u32);
    // Return the string as bits - use string_ptr for proper type identification
    // In a full implementation, we'd wrap this in an Error object
    perry_runtime::JSValue::string_ptr(str_ptr).bits()
}
