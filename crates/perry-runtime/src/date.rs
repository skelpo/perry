//! Date operations runtime support
//!
//! Provides JavaScript Date functionality using system time.
//! Dates are represented internally as i64 timestamps (milliseconds since Unix epoch).

use std::time::{SystemTime, UNIX_EPOCH};

/// Get current timestamp in milliseconds (Date.now())
#[no_mangle]
pub extern "C" fn js_date_now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as f64)
        .unwrap_or(0.0)
}

/// Create a new Date from current time, returning timestamp in milliseconds
#[no_mangle]
pub extern "C" fn js_date_new() -> f64 {
    js_date_now()
}

/// Create a new Date from a timestamp (milliseconds since epoch)
#[no_mangle]
pub extern "C" fn js_date_new_from_timestamp(timestamp: f64) -> f64 {
    timestamp
}

/// Get timestamp from Date (date.getTime())
/// Since we store dates as timestamps, this is an identity function
#[no_mangle]
pub extern "C" fn js_date_get_time(timestamp: f64) -> f64 {
    timestamp
}

/// Convert Date to ISO 8601 string (date.toISOString())
/// Returns a pointer to a StringHeader
#[no_mangle]
pub extern "C" fn js_date_to_iso_string(timestamp: f64) -> *mut crate::StringHeader {
    use std::alloc::{alloc, Layout};

    let ts_ms = timestamp as i64;
    let secs = ts_ms / 1000;
    let millis = (ts_ms % 1000).abs() as u32;

    // Calculate date components from Unix timestamp
    // This is a simplified implementation - proper implementation would use chrono crate
    let (year, month, day, hour, minute, second) = timestamp_to_components(secs);

    // Format as ISO 8601: YYYY-MM-DDTHH:mm:ss.sssZ
    let iso_string = format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        year, month, day, hour, minute, second, millis
    );

    let bytes = iso_string.as_bytes();
    let len = bytes.len();

    unsafe {
        let layout = Layout::from_size_align(
            std::mem::size_of::<crate::StringHeader>() + len,
            std::mem::align_of::<crate::StringHeader>()
        ).unwrap();

        let ptr = alloc(layout) as *mut crate::StringHeader;
        (*ptr).length = len as u32;

        let data_ptr = (ptr as *mut u8).add(std::mem::size_of::<crate::StringHeader>());
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), data_ptr, len);

        ptr
    }
}

/// Get the full year (date.getFullYear())
#[no_mangle]
pub extern "C" fn js_date_get_full_year(timestamp: f64) -> f64 {
    let ts_ms = timestamp as i64;
    let secs = ts_ms / 1000;
    let (year, _, _, _, _, _) = timestamp_to_components(secs);
    year as f64
}

/// Get the month (0-11) (date.getMonth())
#[no_mangle]
pub extern "C" fn js_date_get_month(timestamp: f64) -> f64 {
    let ts_ms = timestamp as i64;
    let secs = ts_ms / 1000;
    let (_, month, _, _, _, _) = timestamp_to_components(secs);
    (month - 1) as f64  // JavaScript months are 0-indexed
}

/// Get the day of month (1-31) (date.getDate())
#[no_mangle]
pub extern "C" fn js_date_get_date(timestamp: f64) -> f64 {
    let ts_ms = timestamp as i64;
    let secs = ts_ms / 1000;
    let (_, _, day, _, _, _) = timestamp_to_components(secs);
    day as f64
}

/// Get the hour (0-23) (date.getHours())
#[no_mangle]
pub extern "C" fn js_date_get_hours(timestamp: f64) -> f64 {
    let ts_ms = timestamp as i64;
    let secs = ts_ms / 1000;
    let (_, _, _, hour, _, _) = timestamp_to_components(secs);
    hour as f64
}

/// Get the minutes (0-59) (date.getMinutes())
#[no_mangle]
pub extern "C" fn js_date_get_minutes(timestamp: f64) -> f64 {
    let ts_ms = timestamp as i64;
    let secs = ts_ms / 1000;
    let (_, _, _, _, minute, _) = timestamp_to_components(secs);
    minute as f64
}

/// Get the seconds (0-59) (date.getSeconds())
#[no_mangle]
pub extern "C" fn js_date_get_seconds(timestamp: f64) -> f64 {
    let ts_ms = timestamp as i64;
    let secs = ts_ms / 1000;
    let (_, _, _, _, _, second) = timestamp_to_components(secs);
    second as f64
}

/// Get the milliseconds (0-999) (date.getMilliseconds())
#[no_mangle]
pub extern "C" fn js_date_get_milliseconds(timestamp: f64) -> f64 {
    let ts_ms = timestamp as i64;
    (ts_ms % 1000).abs() as f64
}

/// Convert Unix timestamp (seconds) to date components (year, month, day, hour, minute, second)
/// Returns components in UTC
fn timestamp_to_components(secs: i64) -> (i32, u32, u32, u32, u32, u32) {
    // Handle negative timestamps (dates before 1970)
    let is_negative = secs < 0;
    let abs_secs = if is_negative { -secs } else { secs } as u64;

    // Extract time of day
    let second = (abs_secs % 60) as u32;
    let minute = ((abs_secs / 60) % 60) as u32;
    let hour = ((abs_secs / 3600) % 24) as u32;

    // Calculate days from Unix epoch
    let mut days = if is_negative {
        -((abs_secs / 86400) as i64) - if abs_secs % 86400 != 0 { 1 } else { 0 }
    } else {
        (abs_secs / 86400) as i64
    };

    // For negative timestamps, adjust time components
    let (hour, minute, second) = if is_negative && abs_secs % 86400 != 0 {
        let remaining = abs_secs % 86400;
        let adjusted = 86400 - remaining;
        (
            ((adjusted / 3600) % 24) as u32,
            ((adjusted / 60) % 60) as u32,
            (adjusted % 60) as u32,
        )
    } else {
        (hour, minute, second)
    };

    // Days since 1970-01-01
    // Using a simplified algorithm based on Howard Hinnant's date algorithms
    let z = days + 719468; // Days from 0000-03-01 to 1970-01-01 is 719468

    let era = if z >= 0 { z / 146097 } else { (z - 146096) / 146097 };
    let doe = (z - era * 146097) as u32; // Day of era [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // Year of era [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // Day of year [0, 365]
    let mp = (5 * doy + 2) / 153; // Month proxy [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // Day [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // Month [1, 12]
    let y = if m <= 2 { y + 1 } else { y };

    (y as i32, m, d, hour, minute, second)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_date_now() {
        let now = js_date_now();
        // Should be a reasonable timestamp (after 2020)
        assert!(now > 1577836800000.0); // 2020-01-01
    }

    #[test]
    fn test_timestamp_to_components() {
        // Test Unix epoch (1970-01-01 00:00:00 UTC)
        let (y, m, d, h, min, s) = timestamp_to_components(0);
        assert_eq!((y, m, d, h, min, s), (1970, 1, 1, 0, 0, 0));

        // Test 2024-01-15 12:30:45 UTC (timestamp: 1705321845)
        let (y, m, d, h, min, s) = timestamp_to_components(1705321845);
        assert_eq!((y, m, d, h, min, s), (2024, 1, 15, 12, 30, 45));
    }
}
