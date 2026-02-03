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

/// Closure pointer type for promise handlers (closures, not raw function pointers)
pub type ClosurePtr = *const crate::closure::ClosureHeader;

/// A Promise represents an eventual completion (or failure) of an async operation
#[repr(C)]
pub struct Promise {
    /// Current state of the promise
    state: PromiseState,
    /// The resolved value (if fulfilled)
    value: f64,
    /// The rejection reason (if rejected)
    reason: f64,
    /// Closure to run when fulfilled (null if none)
    on_fulfilled: ClosurePtr,
    /// Closure to run when rejected (null if none)
    on_rejected: ClosurePtr,
    /// Next promise in the chain (for .then())
    next: *mut Promise,
}

impl Promise {
    fn new() -> Self {
        Promise {
            state: PromiseState::Pending,
            value: 0.0,
            reason: 0.0,
            on_fulfilled: ptr::null(),
            on_rejected: ptr::null(),
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

/// Get promise reason (if rejected)
#[no_mangle]
pub extern "C" fn js_promise_reason(promise: *mut Promise) -> f64 {
    if promise.is_null() {
        return 0.0;
    }
    unsafe { (*promise).reason }
}

/// Get promise result (value if fulfilled, reason if rejected)
/// This is what await should use to get the result of a promise.
/// For fulfilled promises, returns the resolved value.
/// For rejected promises, returns the rejection reason.
/// For pending promises (should not happen in normal use), returns 0.0.
#[no_mangle]
pub extern "C" fn js_promise_result(promise: *mut Promise) -> f64 {
    if promise.is_null() {
        return 0.0;
    }
    unsafe {
        match (*promise).state {
            PromiseState::Fulfilled => (*promise).value,
            PromiseState::Rejected => (*promise).reason,
            PromiseState::Pending => 0.0,
        }
    }
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
        if !(*promise).on_fulfilled.is_null() {
            TASK_QUEUE.with(|q| {
                q.borrow_mut().push((promise, value, true));
            });
        }
    }
}

/// Resolve a promise with another promise (Promise chaining/unwrapping)
/// When the inner promise resolves, the outer promise adopts its value
#[no_mangle]
pub extern "C" fn js_promise_resolve_with_promise(outer: *mut Promise, inner: *mut Promise) {
    if outer.is_null() || inner.is_null() {
        return;
    }

    unsafe {
        if (*outer).state != PromiseState::Pending {
            return; // Already settled
        }

        // Check inner promise state
        match (*inner).state {
            PromiseState::Fulfilled => {
                // Inner already resolved - resolve outer with inner's value
                js_promise_resolve(outer, (*inner).value);
            }
            PromiseState::Rejected => {
                // Inner already rejected - reject outer with inner's reason
                js_promise_reject(outer, (*inner).reason);
            }
            PromiseState::Pending => {
                // Inner is pending - schedule resolution when inner settles
                // We create a closure that captures the outer promise pointer
                // and resolves it when called with the inner's value
                let outer_i64 = outer as i64;

                // Create a resolve forwarding closure
                let resolve_closure = crate::closure::js_closure_alloc(
                    promise_forward_resolve as *const u8,
                    1
                );
                crate::closure::js_closure_set_capture_ptr(resolve_closure, 0, outer_i64);

                // Create a reject forwarding closure
                let reject_closure = crate::closure::js_closure_alloc(
                    promise_forward_reject as *const u8,
                    1
                );
                crate::closure::js_closure_set_capture_ptr(reject_closure, 0, outer_i64);

                // Register the forwarding callbacks on the inner promise
                (*inner).on_fulfilled = resolve_closure;
                (*inner).on_rejected = reject_closure;
                (*inner).next = ptr::null_mut(); // Don't chain, we handle resolution ourselves
            }
        }
    }
}

/// Internal callback for forwarding resolve from inner to outer promise
extern "C" fn promise_forward_resolve(closure: *const crate::closure::ClosureHeader, value: f64) -> f64 {
    let outer_ptr = crate::closure::js_closure_get_capture_ptr(closure, 0) as *mut Promise;
    js_promise_resolve(outer_ptr, value);
    0.0
}

/// Internal callback for forwarding reject from inner to outer promise
extern "C" fn promise_forward_reject(closure: *const crate::closure::ClosureHeader, reason: f64) -> f64 {
    let outer_ptr = crate::closure::js_closure_get_capture_ptr(closure, 0) as *mut Promise;
    js_promise_reject(outer_ptr, reason);
    0.0
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
        if !(*promise).on_rejected.is_null() {
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
    on_fulfilled: ClosurePtr,
    on_rejected: ClosurePtr,
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
                if !on_fulfilled.is_null() {
                    TASK_QUEUE.with(|q| {
                        q.borrow_mut().push((promise, (*promise).value, true));
                    });
                }
            }
            PromiseState::Rejected => {
                if !on_rejected.is_null() {
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
    on_rejected: ClosurePtr,
) -> *mut Promise {
    js_promise_then(promise, ptr::null(), on_rejected)
}

/// Register finally callback, returns a new promise for chaining
/// This is equivalent to .finally(onFinally) in JavaScript
#[no_mangle]
pub extern "C" fn js_promise_finally(
    promise: *mut Promise,
    on_finally: ClosurePtr,
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
                        let callback = (*promise).on_fulfilled;
                        if !callback.is_null() {
                            crate::closure::js_closure_call1(callback, value)
                        } else {
                            value
                        }
                    } else {
                        let callback = (*promise).on_rejected;
                        if !callback.is_null() {
                            crate::closure::js_closure_call1(callback, value)
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

/// Promise.all - takes an array of promises and returns a promise that resolves
/// with an array of all resolved values, or rejects if any promise rejects.
///
/// Arguments:
/// - promises_arr: pointer to an ArrayHeader containing promise pointers (as NaN-boxed f64)
///
/// Returns: a new Promise that resolves with an array of results
#[no_mangle]
pub extern "C" fn js_promise_all(promises_arr: *const crate::array::ArrayHeader) -> *mut Promise {
    use crate::array::{ArrayHeader, js_array_alloc, js_array_get_f64, js_array_length, js_array_set_f64};
    use crate::closure::{js_closure_alloc, js_closure_set_capture_ptr, js_closure_set_capture_f64};
    use crate::value::js_nanbox_get_pointer;

    // Create the result promise
    let result_promise = js_promise_new();

    if promises_arr.is_null() {
        // Promise.all([]) resolves immediately with empty array
        let empty_arr = js_array_alloc(0);
        unsafe { (*empty_arr).length = 0; }
        let arr_f64 = crate::value::js_nanbox_pointer(empty_arr as i64);
        js_promise_resolve(result_promise, arr_f64);
        return result_promise;
    }

    let count = js_array_length(promises_arr);

    if count == 0 {
        // Promise.all([]) resolves immediately with empty array
        let empty_arr = js_array_alloc(0);
        unsafe { (*empty_arr).length = 0; }
        let arr_f64 = crate::value::js_nanbox_pointer(empty_arr as i64);
        js_promise_resolve(result_promise, arr_f64);
        return result_promise;
    }

    // Allocate result array to hold resolved values
    let results_arr = js_array_alloc(count);
    unsafe { (*results_arr).length = count; }

    // Initialize all elements to undefined
    const TAG_UNDEFINED: u64 = 0x7FFC_0000_0000_0001;
    for i in 0..count {
        js_array_set_f64(results_arr, i, f64::from_bits(TAG_UNDEFINED));
    }

    // Allocate state: [remaining_count, rejected_flag]
    // We use an array to hold mutable shared state across closures
    let state_arr = js_array_alloc(2);
    unsafe { (*state_arr).length = 2; }
    js_array_set_f64(state_arr, 0, count as f64);  // remaining count
    js_array_set_f64(state_arr, 1, 0.0);            // rejected flag (0 = not rejected)

    // For each promise in the array, attach a .then handler
    for i in 0..count {
        let promise_f64 = js_array_get_f64(promises_arr, i);

        // Extract promise pointer from NaN-boxed value
        let promise_ptr = js_nanbox_get_pointer(promise_f64) as *mut Promise;

        if promise_ptr.is_null() {
            // Not a promise - treat as already resolved value
            // Store the value directly and decrement count
            js_array_set_f64(results_arr, i, promise_f64);
            let remaining = js_array_get_f64(state_arr, 0) - 1.0;
            js_array_set_f64(state_arr, 0, remaining);
            continue;
        }

        // Create fulfill closure for this promise
        // Captures: [result_promise, results_arr, state_arr, index]
        let fulfill_closure = js_closure_alloc(promise_all_fulfill_handler as *const u8, 4);
        js_closure_set_capture_ptr(fulfill_closure, 0, result_promise as i64);
        js_closure_set_capture_ptr(fulfill_closure, 1, results_arr as i64);
        js_closure_set_capture_ptr(fulfill_closure, 2, state_arr as i64);
        js_closure_set_capture_f64(fulfill_closure, 3, i as f64);

        // Create reject closure for this promise
        // Captures: [result_promise, state_arr]
        let reject_closure = js_closure_alloc(promise_all_reject_handler as *const u8, 2);
        js_closure_set_capture_ptr(reject_closure, 0, result_promise as i64);
        js_closure_set_capture_ptr(reject_closure, 1, state_arr as i64);

        // Attach handlers to the promise
        js_promise_then(promise_ptr, fulfill_closure, reject_closure);
    }

    // Check if all were non-promises (already resolved)
    let remaining = js_array_get_f64(state_arr, 0);
    if remaining == 0.0 {
        let arr_f64 = crate::value::js_nanbox_pointer(results_arr as i64);
        js_promise_resolve(result_promise, arr_f64);
    }

    result_promise
}

/// Internal handler called when a promise in Promise.all fulfills
extern "C" fn promise_all_fulfill_handler(closure: *const crate::closure::ClosureHeader, value: f64) -> f64 {
    use crate::array::{js_array_get_f64, js_array_set_f64, ArrayHeader};
    use crate::closure::{js_closure_get_capture_ptr, js_closure_get_capture_f64};

    let result_promise = js_closure_get_capture_ptr(closure, 0) as *mut Promise;
    let results_arr = js_closure_get_capture_ptr(closure, 1) as *mut ArrayHeader;
    let state_arr = js_closure_get_capture_ptr(closure, 2) as *mut ArrayHeader;
    let index = js_closure_get_capture_f64(closure, 3) as u32;

    // Check if already rejected
    let rejected = js_array_get_f64(state_arr, 1);
    if rejected != 0.0 {
        return 0.0;
    }

    // Store the resolved value
    js_array_set_f64(results_arr, index, value);

    // Decrement remaining count
    let remaining = js_array_get_f64(state_arr, 0) - 1.0;
    js_array_set_f64(state_arr, 0, remaining);

    // If all promises have resolved, resolve the result promise with the array
    if remaining == 0.0 {
        let arr_f64 = crate::value::js_nanbox_pointer(results_arr as i64);
        js_promise_resolve(result_promise, arr_f64);
    }

    0.0
}

/// Internal handler called when a promise in Promise.all rejects
extern "C" fn promise_all_reject_handler(closure: *const crate::closure::ClosureHeader, reason: f64) -> f64 {
    use crate::array::{js_array_get_f64, js_array_set_f64, ArrayHeader};
    use crate::closure::js_closure_get_capture_ptr;

    let result_promise = js_closure_get_capture_ptr(closure, 0) as *mut Promise;
    let state_arr = js_closure_get_capture_ptr(closure, 1) as *mut ArrayHeader;

    // Check if already rejected (only reject once)
    let rejected = js_array_get_f64(state_arr, 1);
    if rejected != 0.0 {
        return 0.0;
    }

    // Mark as rejected
    js_array_set_f64(state_arr, 1, 1.0);

    // Reject the result promise with the reason
    js_promise_reject(result_promise, reason);

    0.0
}
