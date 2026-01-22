//! Redis client module (ioredis compatible)
//!
//! Native implementation of the 'ioredis' npm package using the Rust redis crate.
//! Provides async Redis operations.

use perry_runtime::{js_string_from_bytes, JSValue, StringHeader};
use redis::AsyncCommands;
use std::collections::HashMap;
use std::sync::Mutex;

use crate::common::async_bridge::{queue_promise_resolution, spawn};

// Connection handle storage
lazy_static::lazy_static! {
    static ref REDIS_CONNECTIONS: Mutex<HashMap<usize, redis::aio::MultiplexedConnection>> = Mutex::new(HashMap::new());
    static ref NEXT_CONN_ID: Mutex<usize> = Mutex::new(1);
}

/// Helper to extract string from StringHeader pointer
unsafe fn string_from_header(ptr: *const StringHeader) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    let len = (*ptr).length as usize;
    let data_ptr = (ptr as *const u8).add(std::mem::size_of::<StringHeader>());
    let bytes = std::slice::from_raw_parts(data_ptr, len);
    std::str::from_utf8(bytes).ok().map(|s| s.to_string())
}

/// Create a new Redis connection
/// new Redis() or new Redis(port) or new Redis(port, host) or new Redis(options)
#[no_mangle]
pub unsafe extern "C" fn js_ioredis_new(
    _config_ptr: *const std::ffi::c_void,
) -> *mut perry_runtime::Promise {
    let promise = perry_runtime::js_promise_new();
    let promise_ptr = promise as usize;

    // Default connection URL
    let url = "redis://127.0.0.1:6379".to_string();

    // TODO: Parse config object for host, port, password, db

    spawn(async move {
        let client = match redis::Client::open(url) {
            Ok(c) => c,
            Err(e) => {
                let err_msg = format!("Redis connection error: {}", e);
                let err_str = js_string_from_bytes(err_msg.as_ptr(), err_msg.len() as u32);
                let err_bits = JSValue::pointer(err_str as *const u8).bits();
                queue_promise_resolution(promise_ptr, false, err_bits);
                return;
            }
        };

        match client.get_multiplexed_async_connection().await {
            Ok(conn) => {
                let mut id_guard = NEXT_CONN_ID.lock().unwrap();
                let conn_id = *id_guard;
                *id_guard += 1;
                drop(id_guard);

                REDIS_CONNECTIONS.lock().unwrap().insert(conn_id, conn);

                // Return connection handle as f64
                let result_bits = (conn_id as f64).to_bits();
                queue_promise_resolution(promise_ptr, true, result_bits);
            }
            Err(e) => {
                let err_msg = format!("Redis connection error: {}", e);
                let err_str = js_string_from_bytes(err_msg.as_ptr(), err_msg.len() as u32);
                let err_bits = JSValue::pointer(err_str as *const u8).bits();
                queue_promise_resolution(promise_ptr, false, err_bits);
            }
        }
    });

    promise
}

/// SET command
/// redis.set(key, value) -> Promise<"OK">
#[no_mangle]
pub unsafe extern "C" fn js_ioredis_set(
    handle: i64,
    key_ptr: *const StringHeader,
    value_ptr: *const StringHeader,
) -> *mut perry_runtime::Promise {
    let promise = perry_runtime::js_promise_new();
    let promise_ptr = promise as usize;

    let key = match string_from_header(key_ptr) {
        Some(k) => k,
        None => {
            let err_msg = "Invalid key";
            let err_str = js_string_from_bytes(err_msg.as_ptr(), err_msg.len() as u32);
            let err_bits = JSValue::pointer(err_str as *const u8).bits();
            queue_promise_resolution(promise_ptr, false, err_bits);
            return promise;
        }
    };

    let value = match string_from_header(value_ptr) {
        Some(v) => v,
        None => {
            let err_msg = "Invalid value";
            let err_str = js_string_from_bytes(err_msg.as_ptr(), err_msg.len() as u32);
            let err_bits = JSValue::pointer(err_str as *const u8).bits();
            queue_promise_resolution(promise_ptr, false, err_bits);
            return promise;
        }
    };

    let conn_id = handle as usize;

    spawn(async move {
        let mut conn = {
            let guard = REDIS_CONNECTIONS.lock().unwrap();
            match guard.get(&conn_id) {
                Some(c) => c.clone(),
                None => {
                    let err_msg = "Invalid Redis connection";
                    let err_str = js_string_from_bytes(err_msg.as_ptr(), err_msg.len() as u32);
                    let err_bits = JSValue::pointer(err_str as *const u8).bits();
                    queue_promise_resolution(promise_ptr, false, err_bits);
                    return;
                }
            }
        };

        let result: redis::RedisResult<()> = conn.set(&key, &value).await;
        match result {
            Ok(_) => {
                let ok_str = "OK";
                let result_str = js_string_from_bytes(ok_str.as_ptr(), ok_str.len() as u32);
                let result_bits = JSValue::pointer(result_str as *const u8).bits();
                queue_promise_resolution(promise_ptr, true, result_bits);
            }
            Err(e) => {
                let err_msg = format!("Redis SET error: {}", e);
                let err_str = js_string_from_bytes(err_msg.as_ptr(), err_msg.len() as u32);
                let err_bits = JSValue::pointer(err_str as *const u8).bits();
                queue_promise_resolution(promise_ptr, false, err_bits);
            }
        }
    });

    promise
}

/// GET command
/// redis.get(key) -> Promise<string | null>
#[no_mangle]
pub unsafe extern "C" fn js_ioredis_get(
    handle: i64,
    key_ptr: *const StringHeader,
) -> *mut perry_runtime::Promise {
    let promise = perry_runtime::js_promise_new();
    let promise_ptr = promise as usize;

    let key = match string_from_header(key_ptr) {
        Some(k) => k,
        None => {
            let err_msg = "Invalid key";
            let err_str = js_string_from_bytes(err_msg.as_ptr(), err_msg.len() as u32);
            let err_bits = JSValue::pointer(err_str as *const u8).bits();
            queue_promise_resolution(promise_ptr, false, err_bits);
            return promise;
        }
    };

    let conn_id = handle as usize;

    spawn(async move {
        let mut conn = {
            let guard = REDIS_CONNECTIONS.lock().unwrap();
            match guard.get(&conn_id) {
                Some(c) => c.clone(),
                None => {
                    let err_msg = "Invalid Redis connection";
                    let err_str = js_string_from_bytes(err_msg.as_ptr(), err_msg.len() as u32);
                    let err_bits = JSValue::pointer(err_str as *const u8).bits();
                    queue_promise_resolution(promise_ptr, false, err_bits);
                    return;
                }
            }
        };

        let result: redis::RedisResult<Option<String>> = conn.get(&key).await;
        match result {
            Ok(Some(value)) => {
                let result_str = js_string_from_bytes(value.as_ptr(), value.len() as u32);
                let result_bits = JSValue::pointer(result_str as *const u8).bits();
                queue_promise_resolution(promise_ptr, true, result_bits);
            }
            Ok(None) => {
                // Return null (use JSValue::null())
                let result_bits = JSValue::null().bits();
                queue_promise_resolution(promise_ptr, true, result_bits);
            }
            Err(e) => {
                let err_msg = format!("Redis GET error: {}", e);
                let err_str = js_string_from_bytes(err_msg.as_ptr(), err_msg.len() as u32);
                let err_bits = JSValue::pointer(err_str as *const u8).bits();
                queue_promise_resolution(promise_ptr, false, err_bits);
            }
        }
    });

    promise
}

/// DEL command
/// redis.del(key) -> Promise<number>
#[no_mangle]
pub unsafe extern "C" fn js_ioredis_del(
    handle: i64,
    key_ptr: *const StringHeader,
) -> *mut perry_runtime::Promise {
    let promise = perry_runtime::js_promise_new();
    let promise_ptr = promise as usize;

    let key = match string_from_header(key_ptr) {
        Some(k) => k,
        None => {
            let err_msg = "Invalid key";
            let err_str = js_string_from_bytes(err_msg.as_ptr(), err_msg.len() as u32);
            let err_bits = JSValue::pointer(err_str as *const u8).bits();
            queue_promise_resolution(promise_ptr, false, err_bits);
            return promise;
        }
    };

    let conn_id = handle as usize;

    spawn(async move {
        let mut conn = {
            let guard = REDIS_CONNECTIONS.lock().unwrap();
            match guard.get(&conn_id) {
                Some(c) => c.clone(),
                None => {
                    let err_msg = "Invalid Redis connection";
                    let err_str = js_string_from_bytes(err_msg.as_ptr(), err_msg.len() as u32);
                    let err_bits = JSValue::pointer(err_str as *const u8).bits();
                    queue_promise_resolution(promise_ptr, false, err_bits);
                    return;
                }
            }
        };

        let result: redis::RedisResult<i64> = conn.del(&key).await;
        match result {
            Ok(count) => {
                let result_bits = (count as f64).to_bits();
                queue_promise_resolution(promise_ptr, true, result_bits);
            }
            Err(e) => {
                let err_msg = format!("Redis DEL error: {}", e);
                let err_str = js_string_from_bytes(err_msg.as_ptr(), err_msg.len() as u32);
                let err_bits = JSValue::pointer(err_str as *const u8).bits();
                queue_promise_resolution(promise_ptr, false, err_bits);
            }
        }
    });

    promise
}

/// EXISTS command
/// redis.exists(key) -> Promise<number>
#[no_mangle]
pub unsafe extern "C" fn js_ioredis_exists(
    handle: i64,
    key_ptr: *const StringHeader,
) -> *mut perry_runtime::Promise {
    let promise = perry_runtime::js_promise_new();
    let promise_ptr = promise as usize;

    let key = match string_from_header(key_ptr) {
        Some(k) => k,
        None => {
            let err_msg = "Invalid key";
            let err_str = js_string_from_bytes(err_msg.as_ptr(), err_msg.len() as u32);
            let err_bits = JSValue::pointer(err_str as *const u8).bits();
            queue_promise_resolution(promise_ptr, false, err_bits);
            return promise;
        }
    };

    let conn_id = handle as usize;

    spawn(async move {
        let mut conn = {
            let guard = REDIS_CONNECTIONS.lock().unwrap();
            match guard.get(&conn_id) {
                Some(c) => c.clone(),
                None => {
                    let err_msg = "Invalid Redis connection";
                    let err_str = js_string_from_bytes(err_msg.as_ptr(), err_msg.len() as u32);
                    let err_bits = JSValue::pointer(err_str as *const u8).bits();
                    queue_promise_resolution(promise_ptr, false, err_bits);
                    return;
                }
            }
        };

        let result: redis::RedisResult<i64> = conn.exists(&key).await;
        match result {
            Ok(count) => {
                let result_bits = (count as f64).to_bits();
                queue_promise_resolution(promise_ptr, true, result_bits);
            }
            Err(e) => {
                let err_msg = format!("Redis EXISTS error: {}", e);
                let err_str = js_string_from_bytes(err_msg.as_ptr(), err_msg.len() as u32);
                let err_bits = JSValue::pointer(err_str as *const u8).bits();
                queue_promise_resolution(promise_ptr, false, err_bits);
            }
        }
    });

    promise
}

/// INCR command
/// redis.incr(key) -> Promise<number>
#[no_mangle]
pub unsafe extern "C" fn js_ioredis_incr(
    handle: i64,
    key_ptr: *const StringHeader,
) -> *mut perry_runtime::Promise {
    let promise = perry_runtime::js_promise_new();
    let promise_ptr = promise as usize;

    let key = match string_from_header(key_ptr) {
        Some(k) => k,
        None => {
            let err_msg = "Invalid key";
            let err_str = js_string_from_bytes(err_msg.as_ptr(), err_msg.len() as u32);
            let err_bits = JSValue::pointer(err_str as *const u8).bits();
            queue_promise_resolution(promise_ptr, false, err_bits);
            return promise;
        }
    };

    let conn_id = handle as usize;

    spawn(async move {
        let mut conn = {
            let guard = REDIS_CONNECTIONS.lock().unwrap();
            match guard.get(&conn_id) {
                Some(c) => c.clone(),
                None => {
                    let err_msg = "Invalid Redis connection";
                    let err_str = js_string_from_bytes(err_msg.as_ptr(), err_msg.len() as u32);
                    let err_bits = JSValue::pointer(err_str as *const u8).bits();
                    queue_promise_resolution(promise_ptr, false, err_bits);
                    return;
                }
            }
        };

        let result: redis::RedisResult<i64> = conn.incr(&key, 1).await;
        match result {
            Ok(val) => {
                let result_bits = (val as f64).to_bits();
                queue_promise_resolution(promise_ptr, true, result_bits);
            }
            Err(e) => {
                let err_msg = format!("Redis INCR error: {}", e);
                let err_str = js_string_from_bytes(err_msg.as_ptr(), err_msg.len() as u32);
                let err_bits = JSValue::pointer(err_str as *const u8).bits();
                queue_promise_resolution(promise_ptr, false, err_bits);
            }
        }
    });

    promise
}

/// DECR command
/// redis.decr(key) -> Promise<number>
#[no_mangle]
pub unsafe extern "C" fn js_ioredis_decr(
    handle: i64,
    key_ptr: *const StringHeader,
) -> *mut perry_runtime::Promise {
    let promise = perry_runtime::js_promise_new();
    let promise_ptr = promise as usize;

    let key = match string_from_header(key_ptr) {
        Some(k) => k,
        None => {
            let err_msg = "Invalid key";
            let err_str = js_string_from_bytes(err_msg.as_ptr(), err_msg.len() as u32);
            let err_bits = JSValue::pointer(err_str as *const u8).bits();
            queue_promise_resolution(promise_ptr, false, err_bits);
            return promise;
        }
    };

    let conn_id = handle as usize;

    spawn(async move {
        let mut conn = {
            let guard = REDIS_CONNECTIONS.lock().unwrap();
            match guard.get(&conn_id) {
                Some(c) => c.clone(),
                None => {
                    let err_msg = "Invalid Redis connection";
                    let err_str = js_string_from_bytes(err_msg.as_ptr(), err_msg.len() as u32);
                    let err_bits = JSValue::pointer(err_str as *const u8).bits();
                    queue_promise_resolution(promise_ptr, false, err_bits);
                    return;
                }
            }
        };

        let result: redis::RedisResult<i64> = conn.decr(&key, 1).await;
        match result {
            Ok(val) => {
                let result_bits = (val as f64).to_bits();
                queue_promise_resolution(promise_ptr, true, result_bits);
            }
            Err(e) => {
                let err_msg = format!("Redis DECR error: {}", e);
                let err_str = js_string_from_bytes(err_msg.as_ptr(), err_msg.len() as u32);
                let err_bits = JSValue::pointer(err_str as *const u8).bits();
                queue_promise_resolution(promise_ptr, false, err_bits);
            }
        }
    });

    promise
}

/// EXPIRE command
/// redis.expire(key, seconds) -> Promise<number>
#[no_mangle]
pub unsafe extern "C" fn js_ioredis_expire(
    handle: i64,
    key_ptr: *const StringHeader,
    seconds: f64,
) -> *mut perry_runtime::Promise {
    let promise = perry_runtime::js_promise_new();
    let promise_ptr = promise as usize;

    let key = match string_from_header(key_ptr) {
        Some(k) => k,
        None => {
            let err_msg = "Invalid key";
            let err_str = js_string_from_bytes(err_msg.as_ptr(), err_msg.len() as u32);
            let err_bits = JSValue::pointer(err_str as *const u8).bits();
            queue_promise_resolution(promise_ptr, false, err_bits);
            return promise;
        }
    };

    let secs = seconds as i64;
    let conn_id = handle as usize;

    spawn(async move {
        let mut conn = {
            let guard = REDIS_CONNECTIONS.lock().unwrap();
            match guard.get(&conn_id) {
                Some(c) => c.clone(),
                None => {
                    let err_msg = "Invalid Redis connection";
                    let err_str = js_string_from_bytes(err_msg.as_ptr(), err_msg.len() as u32);
                    let err_bits = JSValue::pointer(err_str as *const u8).bits();
                    queue_promise_resolution(promise_ptr, false, err_bits);
                    return;
                }
            }
        };

        let result: redis::RedisResult<i64> = conn.expire(&key, secs).await;
        match result {
            Ok(val) => {
                let result_bits = (val as f64).to_bits();
                queue_promise_resolution(promise_ptr, true, result_bits);
            }
            Err(e) => {
                let err_msg = format!("Redis EXPIRE error: {}", e);
                let err_str = js_string_from_bytes(err_msg.as_ptr(), err_msg.len() as u32);
                let err_bits = JSValue::pointer(err_str as *const u8).bits();
                queue_promise_resolution(promise_ptr, false, err_bits);
            }
        }
    });

    promise
}

/// QUIT command - close connection
/// redis.quit() -> Promise<"OK">
#[no_mangle]
pub unsafe extern "C" fn js_ioredis_quit(
    handle: i64,
) -> *mut perry_runtime::Promise {
    let promise = perry_runtime::js_promise_new();
    let promise_ptr = promise as usize;
    let conn_id = handle as usize;

    // Remove connection from storage
    REDIS_CONNECTIONS.lock().unwrap().remove(&conn_id);

    let ok_str = "OK";
    let result_str = js_string_from_bytes(ok_str.as_ptr(), ok_str.len() as u32);
    let result_bits = JSValue::pointer(result_str as *const u8).bits();
    queue_promise_resolution(promise_ptr, true, result_bits);

    promise
}
