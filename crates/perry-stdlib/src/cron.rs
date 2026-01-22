//! Cron module (node-cron compatible)
//!
//! Native implementation of the 'node-cron' npm package.
//! Provides cron-based job scheduling.

use perry_runtime::{js_string_from_bytes, StringHeader};
use cron::Schedule;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;
use crate::common::{get_handle, register_handle, Handle, RUNTIME};

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

/// Cron job handle
pub struct CronJobHandle {
    pub schedule: Schedule,
    pub running: Arc<AtomicBool>,
    pub callback_id: u64,
}

// Global callback counter
static CALLBACK_COUNTER: AtomicU64 = AtomicU64::new(1);

/// cron.validate(expression) -> boolean
///
/// Validate a cron expression.
#[no_mangle]
pub unsafe extern "C" fn js_cron_validate(expr_ptr: *const StringHeader) -> bool {
    let expr = match string_from_header(expr_ptr) {
        Some(e) => e,
        None => return false,
    };

    // Convert 5-field cron to 6-field (add seconds)
    let expr = if expr.split_whitespace().count() == 5 {
        format!("0 {}", expr)
    } else {
        expr
    };

    Schedule::from_str(&expr).is_ok()
}

/// cron.schedule(expression, callback_id) -> CronJob
///
/// Schedule a job with a cron expression.
/// Returns a job handle that can be started/stopped.
#[no_mangle]
pub unsafe extern "C" fn js_cron_schedule(
    expr_ptr: *const StringHeader,
    callback_id: f64,
) -> Handle {
    let expr = match string_from_header(expr_ptr) {
        Some(e) => e,
        None => return -1,
    };

    // Convert 5-field cron to 6-field (add seconds)
    let expr = if expr.split_whitespace().count() == 5 {
        format!("0 {}", expr)
    } else {
        expr
    };

    let schedule = match Schedule::from_str(&expr) {
        Ok(s) => s,
        Err(_) => return -1,
    };

    register_handle(CronJobHandle {
        schedule,
        running: Arc::new(AtomicBool::new(false)),
        callback_id: callback_id as u64,
    })
}

/// job.start() -> void
///
/// Start the scheduled job.
#[no_mangle]
pub unsafe extern "C" fn js_cron_job_start(handle: Handle) {
    if let Some(job) = get_handle::<CronJobHandle>(handle) {
        if job.running.load(Ordering::SeqCst) {
            return; // Already running
        }

        job.running.store(true, Ordering::SeqCst);
        let running = job.running.clone();
        let schedule = job.schedule.clone();
        let callback_id = job.callback_id;

        RUNTIME.spawn(async move {
            use chrono::Utc;

            while running.load(Ordering::SeqCst) {
                let now = Utc::now();
                if let Some(next) = schedule.upcoming(Utc).next() {
                    let duration = next.signed_duration_since(now);
                    if duration.num_milliseconds() > 0 {
                        tokio::time::sleep(tokio::time::Duration::from_millis(
                            duration.num_milliseconds() as u64,
                        ))
                        .await;
                    }

                    if running.load(Ordering::SeqCst) {
                        // Here we would call the JavaScript callback
                        // For now, we just mark that it fired
                        // In a real implementation, we'd invoke js_callback_invoke(callback_id)
                    }
                } else {
                    break;
                }
            }
        });
    }
}

/// job.stop() -> void
///
/// Stop the scheduled job.
#[no_mangle]
pub unsafe extern "C" fn js_cron_job_stop(handle: Handle) {
    if let Some(job) = get_handle::<CronJobHandle>(handle) {
        job.running.store(false, Ordering::SeqCst);
    }
}

/// job.isRunning() -> boolean
///
/// Check if the job is currently running.
#[no_mangle]
pub unsafe extern "C" fn js_cron_job_is_running(handle: Handle) -> bool {
    if let Some(job) = get_handle::<CronJobHandle>(handle) {
        return job.running.load(Ordering::SeqCst);
    }
    false
}

/// Get the next scheduled execution time as ISO string
#[no_mangle]
pub unsafe extern "C" fn js_cron_next_date(handle: Handle) -> *mut StringHeader {
    if let Some(job) = get_handle::<CronJobHandle>(handle) {
        if let Some(next) = job.schedule.upcoming(chrono::Utc).next() {
            let iso = next.to_rfc3339();
            return js_string_from_bytes(iso.as_ptr(), iso.len() as u32);
        }
    }
    std::ptr::null_mut()
}

/// Get the next N scheduled execution times
#[no_mangle]
pub unsafe extern "C" fn js_cron_next_dates(
    handle: Handle,
    count: f64,
) -> *mut perry_runtime::ArrayHeader {
    use perry_runtime::{js_array_alloc, js_array_push, JSValue};

    let result = js_array_alloc(0);
    let count = count as usize;

    if let Some(job) = get_handle::<CronJobHandle>(handle) {
        for next in job.schedule.upcoming(chrono::Utc).take(count) {
            let iso = next.to_rfc3339();
            let ptr = js_string_from_bytes(iso.as_ptr(), iso.len() as u32);
            js_array_push(result, JSValue::string_ptr(ptr));
        }
    }

    result
}

/// Parse cron expression and get human-readable description
#[no_mangle]
pub unsafe extern "C" fn js_cron_describe(expr_ptr: *const StringHeader) -> *mut StringHeader {
    let expr = match string_from_header(expr_ptr) {
        Some(e) => e,
        None => return std::ptr::null_mut(),
    };

    let parts: Vec<&str> = expr.split_whitespace().collect();
    let description = match parts.len() {
        5 => {
            // minute hour day month weekday
            format!(
                "At minute {} of hour {}, on day {} of month {}, on weekday {}",
                parts[0], parts[1], parts[2], parts[3], parts[4]
            )
        }
        6 => {
            // second minute hour day month weekday
            format!(
                "At second {} minute {} of hour {}, on day {} of month {}, on weekday {}",
                parts[0], parts[1], parts[2], parts[3], parts[4], parts[5]
            )
        }
        _ => "Invalid cron expression".to_string(),
    };

    js_string_from_bytes(description.as_ptr(), description.len() as u32)
}

// ============================================================================
// Interval/Timeout helpers (not strictly cron, but commonly used together)
// ============================================================================

/// Set an interval (simplified - returns handle)
#[no_mangle]
pub extern "C" fn js_cron_set_interval(callback_id: f64, interval_ms: f64) -> Handle {
    let running = Arc::new(AtomicBool::new(true));
    let running_clone = running.clone();
    let interval = interval_ms as u64;

    RUNTIME.spawn(async move {
        while running_clone.load(Ordering::SeqCst) {
            tokio::time::sleep(tokio::time::Duration::from_millis(interval)).await;
            if running_clone.load(Ordering::SeqCst) {
                // Invoke callback (in real impl: js_callback_invoke(callback_id))
            }
        }
    });

    // Store running flag in a handle
    struct IntervalHandle {
        running: Arc<AtomicBool>,
    }

    register_handle(IntervalHandle { running })
}

/// Clear an interval
#[no_mangle]
pub unsafe extern "C" fn js_cron_clear_interval(handle: Handle) {
    struct IntervalHandle {
        running: Arc<AtomicBool>,
    }

    if let Some(interval) = get_handle::<IntervalHandle>(handle) {
        interval.running.store(false, Ordering::SeqCst);
    }
}

/// Set a timeout (simplified - returns handle)
#[no_mangle]
pub extern "C" fn js_cron_set_timeout(callback_id: f64, timeout_ms: f64) -> Handle {
    let cancelled = Arc::new(AtomicBool::new(false));
    let cancelled_clone = cancelled.clone();
    let timeout = timeout_ms as u64;

    RUNTIME.spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_millis(timeout)).await;
        if !cancelled_clone.load(Ordering::SeqCst) {
            // Invoke callback (in real impl: js_callback_invoke(callback_id))
        }
    });

    struct TimeoutHandle {
        cancelled: Arc<AtomicBool>,
    }

    register_handle(TimeoutHandle { cancelled })
}

/// Clear a timeout
#[no_mangle]
pub unsafe extern "C" fn js_cron_clear_timeout(handle: Handle) {
    struct TimeoutHandle {
        cancelled: Arc<AtomicBool>,
    }

    if let Some(timeout) = get_handle::<TimeoutHandle>(handle) {
        timeout.cancelled.store(true, Ordering::SeqCst);
    }
}
