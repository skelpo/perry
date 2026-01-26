//! Promise implementation for async/await support
//!
//! This is a simplified Promise implementation for the Perry runtime.
//! It supports basic resolve/reject and then/catch chaining.

use std::cell::RefCell;
use std::ptr;

/// Promise state
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PromiseState {
    Pending = 0,
    Fulfilled = 1,
    Rejected = 2,
}

/// Callback type for promise handlers
pub type PromiseCallback = extern "C" fn(f64) -> f64;

/// A Promise represents an eventual completion (or failure) of an async operation
#[repr(C)]
pub struct Promise {
    /// Current state of the promise
    state: PromiseState,
    /// The resolved value (if fulfilled)
    value: f64,
    /// The rejection reason (if rejected)
    reason: f64,
    /// Callback to run when fulfilled
    on_fulfilled: Option<PromiseCallback>,
    /// Callback to run when rejected
    on_rejected: Option<PromiseCallback>,
    /// Next promise in the chain (for .then())
    next: *mut Promise,
}

impl Promise {
    fn new() -> Self {
        Promise {
            state: PromiseState::Pending,
            value: 0.0,
            reason: 0.0,
            on_fulfilled: None,
            on_rejected: None,
            next: ptr::null_mut(),
        }
    }
}

// Global task queue for pending promise callbacks
thread_local! {
    static TASK_QUEUE: RefCell<Vec<(*mut Promise, f64, bool)>> = RefCell::new(Vec::new());
}

/// Allocate a new Promise
#[no_mangle]
pub extern "C" fn js_promise_new() -> *mut Promise {
    let promise = Box::new(Promise::new());
    Box::into_raw(promise)
}

/// Free a Promise
#[no_mangle]
pub extern "C" fn js_promise_free(promise: *mut Promise) {
    if !promise.is_null() {
        unsafe {
            let _ = Box::from_raw(promise);
        }
    }
}

/// Get promise state (0=pending, 1=fulfilled, 2=rejected)
#[no_mangle]
pub extern "C" fn js_promise_state(promise: *mut Promise) -> i32 {
    if promise.is_null() {
        return -1;
    }
    unsafe { (*promise).state as i32 }
}

/// Get promise value (if fulfilled)
#[no_mangle]
pub extern "C" fn js_promise_value(promise: *mut Promise) -> f64 {
    if promise.is_null() {
        return 0.0;
    }
    unsafe { (*promise).value }
}

/// Resolve a promise with a value
#[no_mangle]
pub extern "C" fn js_promise_resolve(promise: *mut Promise, value: f64) {
    if promise.is_null() {
        return;
    }
    unsafe {
        if (*promise).state != PromiseState::Pending {
            return; // Already settled
        }
        (*promise).state = PromiseState::Fulfilled;
        (*promise).value = value;

        // Schedule callbacks
        if let Some(callback) = (*promise).on_fulfilled {
            TASK_QUEUE.with(|q| {
                q.borrow_mut().push((promise, value, true));
            });
        }
    }
}

/// Reject a promise with a reason
#[no_mangle]
pub extern "C" fn js_promise_reject(promise: *mut Promise, reason: f64) {
    if promise.is_null() {
        return;
    }
    unsafe {
        if (*promise).state != PromiseState::Pending {
            return; // Already settled
        }
        (*promise).state = PromiseState::Rejected;
        (*promise).reason = reason;

        // Schedule callbacks
        if let Some(callback) = (*promise).on_rejected {
            TASK_QUEUE.with(|q| {
                q.borrow_mut().push((promise, reason, false));
            });
        }
    }
}

/// Register fulfillment callback, returns a new promise for chaining
#[no_mangle]
pub extern "C" fn js_promise_then(
    promise: *mut Promise,
    on_fulfilled: Option<PromiseCallback>,
    on_rejected: Option<PromiseCallback>,
) -> *mut Promise {
    if promise.is_null() {
        return ptr::null_mut();
    }

    let next = js_promise_new();

    unsafe {
        (*promise).on_fulfilled = on_fulfilled;
        (*promise).on_rejected = on_rejected;
        (*promise).next = next;

        // If already settled, schedule callback immediately
        match (*promise).state {
            PromiseState::Fulfilled => {
                if on_fulfilled.is_some() {
                    TASK_QUEUE.with(|q| {
                        q.borrow_mut().push((promise, (*promise).value, true));
                    });
                }
            }
            PromiseState::Rejected => {
                if on_rejected.is_some() {
                    TASK_QUEUE.with(|q| {
                        q.borrow_mut().push((promise, (*promise).reason, false));
                    });
                }
            }
            PromiseState::Pending => {}
        }
    }

    next
}

/// Register rejection callback, returns a new promise for chaining
/// This is equivalent to .catch(onRejected) in JavaScript
#[no_mangle]
pub extern "C" fn js_promise_catch(
    promise: *mut Promise,
    on_rejected: Option<PromiseCallback>,
) -> *mut Promise {
    js_promise_then(promise, None, on_rejected)
}

/// Register finally callback, returns a new promise for chaining
/// This is equivalent to .finally(onFinally) in JavaScript
#[no_mangle]
pub extern "C" fn js_promise_finally(
    promise: *mut Promise,
    on_finally: Option<PromiseCallback>,
) -> *mut Promise {
    // For finally, we pass the same callback for both fulfilled and rejected
    // The finally callback doesn't receive any arguments in JS
    js_promise_then(promise, on_finally, on_finally)
}

/// Process all pending promise callbacks (run microtasks)
#[no_mangle]
pub extern "C" fn js_promise_run_microtasks() -> i32 {
    let mut ran = 0;

    // First, tick timers to resolve any expired timer promises
    ran += crate::timer::js_timer_tick();

    // Process callback timers (setTimeout with callbacks)
    ran += crate::timer::js_callback_timer_tick();

    // Process interval timers (setInterval)
    ran += crate::timer::js_interval_timer_tick();

    // Process any scheduled resolutions (simulates async completions)
    ran += process_scheduled_resolves();

    // Then process the task queue
    loop {
        let task = TASK_QUEUE.with(|q| q.borrow_mut().pop());

        match task {
            Some((promise, value, is_fulfilled)) => {
                unsafe {
                    let result = if is_fulfilled {
                        if let Some(callback) = (*promise).on_fulfilled {
                            callback(value)
                        } else {
                            value
                        }
                    } else {
                        if let Some(callback) = (*promise).on_rejected {
                            callback(value)
                        } else {
                            value
                        }
                    };

                    // Resolve the next promise in chain
                    if !(*promise).next.is_null() {
                        js_promise_resolve((*promise).next, result);
                    }
                }
                ran += 1;
            }
            None => break,
        }
    }

    ran
}

/// Create a resolved promise with the given value
#[no_mangle]
pub extern "C" fn js_promise_resolved(value: f64) -> *mut Promise {
    let promise = js_promise_new();
    js_promise_resolve(promise, value);
    promise
}

/// Create a rejected promise with the given reason
#[no_mangle]
pub extern "C" fn js_promise_rejected(reason: f64) -> *mut Promise {
    let promise = js_promise_new();
    js_promise_reject(promise, reason);
    promise
}

/// Check if a value is a promise (by checking if it's a valid pointer)
/// This is a simplified check - in reality we'd need type tags
#[no_mangle]
pub extern "C" fn js_is_promise(ptr: *mut Promise) -> i32 {
    if ptr.is_null() {
        return 0;
    }
    // Basic sanity check - could be more sophisticated
    1
}

// Queue for scheduled promise resolutions
thread_local! {
    static SCHEDULED_RESOLVES: RefCell<Vec<(*mut Promise, f64)>> = RefCell::new(Vec::new());
}

/// Schedule a promise to be resolved with a value when microtasks run
/// This simulates an async operation completing
#[no_mangle]
pub extern "C" fn js_promise_schedule_resolve(promise: *mut Promise, value: f64) {
    SCHEDULED_RESOLVES.with(|q| {
        q.borrow_mut().push((promise, value));
    });
}

/// Process scheduled resolutions (called by js_promise_run_microtasks)
fn process_scheduled_resolves() -> i32 {
    let mut count = 0;
    loop {
        let item = SCHEDULED_RESOLVES.with(|q| q.borrow_mut().pop());
        match item {
            Some((promise, value)) => {
                js_promise_resolve(promise, value);
                count += 1;
            }
            None => break,
        }
    }
    count
}

/// Create a new Promise with an executor callback.
/// The executor receives (resolve, reject) as arguments.
/// resolve and reject are closures that call js_promise_resolve/js_promise_reject.
///
/// Arguments:
/// - executor: A closure that takes 2 arguments (resolve_fn, reject_fn)
#[no_mangle]
pub extern "C" fn js_promise_new_with_executor(executor: *const crate::closure::ClosureHeader) -> *mut Promise {
    use crate::closure::{js_closure_alloc, js_closure_call2, js_closure_set_capture_ptr};

    let promise = js_promise_new();
    let promise_i64 = promise as i64;

    // Create resolve closure that captures the promise pointer
    // The resolve function signature is: (closure: *const ClosureHeader, value: f64) -> f64
    let resolve_closure = js_closure_alloc(promise_resolve_fn as *const u8, 1);
    js_closure_set_capture_ptr(resolve_closure, 0, promise_i64);

    // Create reject closure that captures the promise pointer
    let reject_closure = js_closure_alloc(promise_reject_fn as *const u8, 1);
    js_closure_set_capture_ptr(reject_closure, 0, promise_i64);

    // Call the executor with (resolve_closure, reject_closure)
    // The closures are passed as f64 by bitcasting the pointer bits
    // This preserves the exact bits of the pointer when passed through f64 ABI
    let resolve_f64: f64 = unsafe { std::mem::transmute(resolve_closure as i64) };
    let reject_f64: f64 = unsafe { std::mem::transmute(reject_closure as i64) };
    unsafe {
        js_closure_call2(executor, resolve_f64, reject_f64);
    }

    promise
}

/// Internal resolve function for Promise executor callbacks.
/// Called when user calls resolve(value) inside the executor.
extern "C" fn promise_resolve_fn(closure: *const crate::closure::ClosureHeader, value: f64) -> f64 {
    use crate::closure::js_closure_get_capture_ptr;

    let promise_ptr = js_closure_get_capture_ptr(closure, 0) as *mut Promise;
    js_promise_resolve(promise_ptr, value);
    0.0 // resolve returns undefined
}

/// Internal reject function for Promise executor callbacks.
/// Called when user calls reject(reason) inside the executor.
extern "C" fn promise_reject_fn(closure: *const crate::closure::ClosureHeader, reason: f64) -> f64 {
    use crate::closure::js_closure_get_capture_ptr;

    let promise_ptr = js_closure_get_capture_ptr(closure, 0) as *mut Promise;
    js_promise_reject(promise_ptr, reason);
    0.0 // reject returns undefined
}
