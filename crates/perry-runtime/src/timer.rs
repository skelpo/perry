//! Timer support for setTimeout/setInterval
//!
//! Provides a simple timer queue that integrates with the Promise runtime.

use std::cell::RefCell;
use std::time::{Duration, Instant};
use crate::promise::{Promise, js_promise_new, js_promise_resolve};

/// A scheduled timer
struct Timer {
    /// When this timer should fire
    deadline: Instant,
    /// The promise to resolve when the timer fires
    promise: *mut Promise,
    /// The value to resolve with (typically undefined/0.0)
    value: f64,
}

// Global timer queue
thread_local! {
    static TIMER_QUEUE: RefCell<Vec<Timer>> = RefCell::new(Vec::new());
    static START_TIME: RefCell<Option<Instant>> = RefCell::new(None);
}

/// Initialize the timer system (called once at startup)
fn ensure_initialized() {
    START_TIME.with(|st| {
        if st.borrow().is_none() {
            *st.borrow_mut() = Some(Instant::now());
        }
    });
}

/// Get current time in milliseconds since program start
#[no_mangle]
pub extern "C" fn js_timer_now() -> f64 {
    ensure_initialized();
    START_TIME.with(|st| {
        st.borrow().map(|start| start.elapsed().as_millis() as f64).unwrap_or(0.0)
    })
}

/// Schedule a timer that resolves a promise after delay_ms milliseconds
/// Returns the promise that will be resolved
#[no_mangle]
pub extern "C" fn js_set_timeout(delay_ms: f64) -> *mut Promise {
    ensure_initialized();

    let promise = js_promise_new();
    let delay = Duration::from_millis(delay_ms.max(0.0) as u64);
    let deadline = Instant::now() + delay;

    TIMER_QUEUE.with(|q| {
        q.borrow_mut().push(Timer {
            deadline,
            promise,
            value: 0.0, // setTimeout resolves with undefined
        });
    });

    promise
}

/// Schedule a timer with a specific resolve value
#[no_mangle]
pub extern "C" fn js_set_timeout_value(delay_ms: f64, value: f64) -> *mut Promise {
    ensure_initialized();

    let promise = js_promise_new();
    let delay = Duration::from_millis(delay_ms.max(0.0) as u64);
    let deadline = Instant::now() + delay;

    TIMER_QUEUE.with(|q| {
        q.borrow_mut().push(Timer {
            deadline,
            promise,
            value,
        });
    });

    promise
}

/// Process any expired timers, resolving their promises
/// Returns the number of timers that fired
#[no_mangle]
pub extern "C" fn js_timer_tick() -> i32 {
    let now = Instant::now();
    let mut fired = 0;

    // Collect expired timers
    let expired: Vec<Timer> = TIMER_QUEUE.with(|q| {
        let mut queue = q.borrow_mut();
        let mut expired = Vec::new();
        let mut i = 0;
        while i < queue.len() {
            if queue[i].deadline <= now {
                expired.push(queue.remove(i));
            } else {
                i += 1;
            }
        }
        expired
    });

    // Resolve the expired timers' promises
    for timer in expired {
        js_promise_resolve(timer.promise, timer.value);
        fired += 1;
    }

    fired
}

/// Check if there are any pending timers
#[no_mangle]
pub extern "C" fn js_timer_has_pending() -> i32 {
    TIMER_QUEUE.with(|q| if q.borrow().is_empty() { 0 } else { 1 })
}

/// Get the time until the next timer fires (in ms), or -1 if no timers
#[no_mangle]
pub extern "C" fn js_timer_next_deadline() -> f64 {
    let now = Instant::now();

    TIMER_QUEUE.with(|q| {
        q.borrow()
            .iter()
            .map(|t| {
                if t.deadline <= now {
                    0.0
                } else {
                    (t.deadline - now).as_millis() as f64
                }
            })
            .min_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or(-1.0)
    })
}

/// Sleep for the specified number of milliseconds
/// This is a blocking sleep - use sparingly
#[no_mangle]
pub extern "C" fn js_sleep_ms(ms: f64) {
    if ms > 0.0 {
        std::thread::sleep(Duration::from_millis(ms as u64));
    }
}

/// JS-style setTimeout that takes a callback function and delay
/// The callback is a closure pointer that will be called with no arguments
/// Returns a timer ID (currently always 0)
#[no_mangle]
pub extern "C" fn js_set_timeout_callback(callback: i64, delay_ms: f64) -> i64 {
    use crate::closure::js_closure_call0;

    ensure_initialized();

    // Schedule the callback to be called after the delay
    // For now, we create a timer and store the callback
    // The callback will be invoked when the timer fires

    let delay = Duration::from_millis(delay_ms.max(0.0) as u64);
    let deadline = Instant::now() + delay;

    // Store callback in a special timer structure
    CALLBACK_TIMERS.with(|q| {
        q.borrow_mut().push(CallbackTimer {
            deadline,
            callback,
        });
    });

    0 // timer ID (not really used currently)
}

/// A scheduled timer with a callback
struct CallbackTimer {
    /// When this timer should fire
    deadline: Instant,
    /// The closure pointer to call
    callback: i64,
}

thread_local! {
    static CALLBACK_TIMERS: RefCell<Vec<CallbackTimer>> = RefCell::new(Vec::new());
}

/// Process any expired callback timers
/// Returns the number of callbacks that were called
#[no_mangle]
pub extern "C" fn js_callback_timer_tick() -> i32 {
    use crate::closure::js_closure_call0;

    let now = Instant::now();
    let mut fired = 0;

    // Collect expired timers
    let expired: Vec<CallbackTimer> = CALLBACK_TIMERS.with(|q| {
        let mut queue = q.borrow_mut();
        let mut expired = Vec::new();
        let mut i = 0;
        while i < queue.len() {
            if queue[i].deadline <= now {
                expired.push(queue.remove(i));
            } else {
                i += 1;
            }
        }
        expired
    });

    // Call the callbacks
    for timer in expired {
        // Call the closure with no arguments
        // The closure pointer is an i64 (pointer to ClosureHeader)
        unsafe {
            js_closure_call0(timer.callback as *const crate::closure::ClosureHeader);
        }
        fired += 1;
    }

    fired
}

/// Check if there are any pending callback timers
#[no_mangle]
pub extern "C" fn js_callback_timer_has_pending() -> i32 {
    CALLBACK_TIMERS.with(|q| if q.borrow().is_empty() { 0 } else { 1 })
}

// ============================================================================
// setInterval / clearInterval support
// ============================================================================

/// An interval timer that fires repeatedly
struct IntervalTimer {
    /// Unique ID for this interval
    id: i64,
    /// The closure pointer to call
    callback: i64,
    /// Interval duration in milliseconds
    interval_ms: u64,
    /// When this interval should next fire
    next_deadline: Instant,
    /// Whether this interval has been cleared
    cleared: bool,
}

thread_local! {
    /// Active interval timers
    static INTERVAL_TIMERS: RefCell<Vec<IntervalTimer>> = RefCell::new(Vec::new());
    /// Next interval ID to assign
    static NEXT_INTERVAL_ID: RefCell<i64> = RefCell::new(1);
}

/// JS-style setInterval that takes a callback function and interval
/// The callback is a closure pointer that will be called repeatedly
/// Returns an interval ID that can be used with clearInterval
#[no_mangle]
pub extern "C" fn setInterval(callback: i64, interval_ms: f64) -> i64 {
    ensure_initialized();

    let interval = interval_ms.max(0.0) as u64;
    let next_deadline = Instant::now() + Duration::from_millis(interval);

    let id = NEXT_INTERVAL_ID.with(|id_cell| {
        let mut id = id_cell.borrow_mut();
        let current = *id;
        *id += 1;
        current
    });

    INTERVAL_TIMERS.with(|timers| {
        timers.borrow_mut().push(IntervalTimer {
            id,
            callback,
            interval_ms: interval,
            next_deadline,
            cleared: false,
        });
    });

    id
}

/// Clear an interval timer by ID
/// After this call, the callback will no longer be invoked
#[no_mangle]
pub extern "C" fn clearInterval(interval_id: i64) {
    INTERVAL_TIMERS.with(|timers| {
        let mut timers = timers.borrow_mut();
        // Mark the interval as cleared instead of removing it
        // This prevents issues if clearInterval is called from within a callback
        for timer in timers.iter_mut() {
            if timer.id == interval_id {
                timer.cleared = true;
                break;
            }
        }
        // Also remove any already-cleared intervals to prevent memory growth
        timers.retain(|t| !t.cleared);
    });
}

/// Process any expired interval timers
/// Returns the number of callbacks that were called
#[no_mangle]
pub extern "C" fn js_interval_timer_tick() -> i32 {
    use crate::closure::js_closure_call0;

    let now = Instant::now();
    let mut fired = 0;

    // Collect callbacks to call and update deadlines
    let callbacks_to_call: Vec<i64> = INTERVAL_TIMERS.with(|timers| {
        let mut timers = timers.borrow_mut();
        let mut callbacks = Vec::new();

        for timer in timers.iter_mut() {
            if !timer.cleared && timer.next_deadline <= now {
                callbacks.push(timer.callback);
                // Schedule the next firing
                timer.next_deadline = now + Duration::from_millis(timer.interval_ms);
            }
        }

        // Clean up any cleared timers
        timers.retain(|t| !t.cleared);

        callbacks
    });

    // Call the callbacks outside of the borrow
    for callback in callbacks_to_call {
        unsafe {
            js_closure_call0(callback as *const crate::closure::ClosureHeader);
        }
        fired += 1;
    }

    fired
}

/// Check if there are any pending interval timers
#[no_mangle]
pub extern "C" fn js_interval_timer_has_pending() -> i32 {
    INTERVAL_TIMERS.with(|timers| {
        let timers = timers.borrow();
        if timers.iter().any(|t| !t.cleared) { 1 } else { 0 }
    })
}

/// Get the time until the next interval timer fires (in ms), or -1 if no timers
#[no_mangle]
pub extern "C" fn js_interval_timer_next_deadline() -> f64 {
    let now = Instant::now();

    INTERVAL_TIMERS.with(|timers| {
        timers.borrow()
            .iter()
            .filter(|t| !t.cleared)
            .map(|t| {
                if t.next_deadline <= now {
                    0.0
                } else {
                    (t.next_deadline - now).as_millis() as f64
                }
            })
            .min_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or(-1.0)
    })
}
