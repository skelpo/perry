//! Redis client module for Perry
//!
//! Provides native Redis database connectivity using the `redis` crate.

use crate::object::ObjectHeader;
#[allow(unused_imports)]
use crate::array::ArrayHeader;
use crate::string::{js_string_from_bytes, StringHeader};
use crate::promise::{js_promise_new, js_promise_resolve};
use crate::value::JSValue;
use redis::{Client, Commands, Connection};
use std::collections::HashMap;
use std::sync::Mutex;

/// Get the data pointer for a string
unsafe fn string_data(s: *const StringHeader) -> *const u8 {
    (s as *const u8).add(std::mem::size_of::<StringHeader>())
}

lazy_static::lazy_static! {
    /// Global pool of Redis connections
    static ref REDIS_CLIENTS: Mutex<HashMap<u64, Connection>> = Mutex::new(HashMap::new());
    static ref NEXT_CLIENT_ID: Mutex<u64> = Mutex::new(1);
}

/// Extract a Rust string from a Perry StringHeader pointer
unsafe fn extract_string(str_ptr: *const StringHeader) -> Option<String> {
    if str_ptr.is_null() {
        return None;
    }
    let len = (*str_ptr).length as usize;
    let data = string_data(str_ptr);
    let slice = std::slice::from_raw_parts(data, len);
    String::from_utf8(slice.to_vec()).ok()
}

/// Extract a string field from a config object
unsafe fn get_config_string(config: i64, field: &str) -> Option<String> {
    let obj_ptr = config as *mut ObjectHeader;
    if obj_ptr.is_null() {
        return None;
    }

    // Create a temporary string header for the field name
    let key_str = js_string_from_bytes(field.as_ptr(), field.len() as u32);
    let field_val = crate::object::js_object_get_field_by_name(obj_ptr, key_str);

    // Check if the value is undefined or null
    if field_val.is_undefined() || field_val.is_null() {
        return None;
    }

    // Convert JSValue to string
    if field_val.is_string() {
        let str_ptr = field_val.as_string_ptr();
        if !str_ptr.is_null() {
            let len = (*str_ptr).length as usize;
            let data = string_data(str_ptr);
            let slice = std::slice::from_raw_parts(data, len);
            return String::from_utf8(slice.to_vec()).ok();
        }
    }
    None
}

/// Extract a number field from a config object
unsafe fn get_config_number(config: i64, field: &str) -> Option<i64> {
    let obj_ptr = config as *mut ObjectHeader;
    if obj_ptr.is_null() {
        return None;
    }

    // Create a temporary string header for the field name
    let key_str = js_string_from_bytes(field.as_ptr(), field.len() as u32);
    let field_val = crate::object::js_object_get_field_by_name(obj_ptr, key_str);

    // Check if the value is undefined or null
    if field_val.is_undefined() || field_val.is_null() {
        return None;
    }

    if field_val.is_number() {
        return Some(field_val.as_number() as i64);
    }
    None
}

/// Build Redis URL from config object
unsafe fn build_redis_url(config: i64) -> String {
    let host = get_config_string(config, "host").unwrap_or_else(|| "127.0.0.1".to_string());
    let port = get_config_number(config, "port").unwrap_or(6379);
    let password = get_config_string(config, "password");
    let db = get_config_number(config, "db").unwrap_or(0);

    if let Some(pass) = password {
        format!("redis://:{}@{}:{}/{}", pass, host, port, db)
    } else {
        format!("redis://{}:{}/{}", host, port, db)
    }
}

// ============================================================================
// FFI Functions
// ============================================================================

/// Create a new Redis client connection
/// js_redis_connect(config: i64) -> Promise (i64)
#[no_mangle]
pub extern "C" fn js_redis_connect(config: i64) -> i64 {
    unsafe {
        let promise = js_promise_new();

        let redis_url = build_redis_url(config);

        match Client::open(redis_url.as_str()) {
            Ok(client) => {
                match client.get_connection() {
                    Ok(conn) => {
                        let mut clients = REDIS_CLIENTS.lock().unwrap();
                        let mut next_id = NEXT_CLIENT_ID.lock().unwrap();
                        let client_id = *next_id;
                        *next_id += 1;
                        clients.insert(client_id, conn);

                        // Return the client handle as a resolved promise
                        let handle = f64::from_bits(JSValue::number(client_id as f64).bits());
                        js_promise_resolve(promise, handle);
                    }
                    Err(e) => {
                        eprintln!("Redis connection error: {}", e);
                        js_promise_resolve(promise, f64::from_bits(JSValue::null().bits()));
                    }
                }
            }
            Err(e) => {
                eprintln!("Redis client error: {}", e);
                js_promise_resolve(promise, f64::from_bits(JSValue::null().bits()));
            }
        }

        promise as i64
    }
}

/// Redis GET command
/// js_redis_get(client: i64, key: i64) -> Promise (i64)
#[no_mangle]
pub extern "C" fn js_redis_get(client_handle: i64, key: i64) -> i64 {
    unsafe {
        let promise = js_promise_new();

        let key_str = match extract_string(key as *const StringHeader) {
            Some(s) => s,
            None => {
                js_promise_resolve(promise, f64::from_bits(JSValue::null().bits()));
                return promise as i64;
            }
        };

        let client_id = client_handle as u64;
        let mut clients = REDIS_CLIENTS.lock().unwrap();

        if let Some(conn) = clients.get_mut(&client_id) {
            match conn.get::<_, Option<String>>(&key_str) {
                Ok(Some(value)) => {
                    let str_ptr = js_string_from_bytes(value.as_ptr(), value.len() as u32);
                    js_promise_resolve(promise, f64::from_bits(JSValue::string_ptr(str_ptr).bits()));
                }
                Ok(None) => {
                    js_promise_resolve(promise, f64::from_bits(JSValue::null().bits()));
                }
                Err(e) => {
                    eprintln!("Redis GET error: {}", e);
                    js_promise_resolve(promise, f64::from_bits(JSValue::null().bits()));
                }
            }
        } else {
            eprintln!("Redis client not found: {}", client_id);
            js_promise_resolve(promise, f64::from_bits(JSValue::null().bits()));
        }

        promise as i64
    }
}

/// Redis SET command
/// js_redis_set(client: i64, key: i64, value: i64) -> Promise (i64)
#[no_mangle]
pub extern "C" fn js_redis_set(client_handle: i64, key: i64, value: i64) -> i64 {
    unsafe {
        let promise = js_promise_new();

        let key_str = match extract_string(key as *const StringHeader) {
            Some(s) => s,
            None => {
                js_promise_resolve(promise, f64::from_bits(JSValue::null().bits()));
                return promise as i64;
            }
        };

        let value_str = match extract_string(value as *const StringHeader) {
            Some(s) => s,
            None => {
                js_promise_resolve(promise, f64::from_bits(JSValue::null().bits()));
                return promise as i64;
            }
        };

        let client_id = client_handle as u64;
        let mut clients = REDIS_CLIENTS.lock().unwrap();

        if let Some(conn) = clients.get_mut(&client_id) {
            match conn.set::<_, _, ()>(&key_str, &value_str) {
                Ok(()) => {
                    // Return "OK" like redis-cli
                    let ok_str = js_string_from_bytes("OK".as_ptr(), 2);
                    js_promise_resolve(promise, f64::from_bits(JSValue::string_ptr(ok_str).bits()));
                }
                Err(e) => {
                    eprintln!("Redis SET error: {}", e);
                    js_promise_resolve(promise, f64::from_bits(JSValue::null().bits()));
                }
            }
        } else {
            eprintln!("Redis client not found: {}", client_id);
            js_promise_resolve(promise, f64::from_bits(JSValue::null().bits()));
        }

        promise as i64
    }
}

/// Redis DEL command
/// js_redis_del(client: i64, key: i64) -> Promise (i64)
#[no_mangle]
pub extern "C" fn js_redis_del(client_handle: i64, key: i64) -> i64 {
    unsafe {
        let promise = js_promise_new();

        let key_str = match extract_string(key as *const StringHeader) {
            Some(s) => s,
            None => {
                js_promise_resolve(promise, f64::from_bits(JSValue::null().bits()));
                return promise as i64;
            }
        };

        let client_id = client_handle as u64;
        let mut clients = REDIS_CLIENTS.lock().unwrap();

        if let Some(conn) = clients.get_mut(&client_id) {
            match conn.del::<_, i64>(&key_str) {
                Ok(count) => {
                    js_promise_resolve(promise, f64::from_bits(JSValue::number(count as f64).bits()));
                }
                Err(e) => {
                    eprintln!("Redis DEL error: {}", e);
                    js_promise_resolve(promise, f64::from_bits(JSValue::null().bits()));
                }
            }
        } else {
            eprintln!("Redis client not found: {}", client_id);
            js_promise_resolve(promise, f64::from_bits(JSValue::null().bits()));
        }

        promise as i64
    }
}

/// Redis EXISTS command
/// js_redis_exists(client: i64, key: i64) -> Promise (i64)
#[no_mangle]
pub extern "C" fn js_redis_exists(client_handle: i64, key: i64) -> i64 {
    unsafe {
        let promise = js_promise_new();

        let key_str = match extract_string(key as *const StringHeader) {
            Some(s) => s,
            None => {
                js_promise_resolve(promise, f64::from_bits(JSValue::null().bits()));
                return promise as i64;
            }
        };

        let client_id = client_handle as u64;
        let mut clients = REDIS_CLIENTS.lock().unwrap();

        if let Some(conn) = clients.get_mut(&client_id) {
            match conn.exists::<_, bool>(&key_str) {
                Ok(exists) => {
                    js_promise_resolve(promise, f64::from_bits(JSValue::bool(exists).bits()));
                }
                Err(e) => {
                    eprintln!("Redis EXISTS error: {}", e);
                    js_promise_resolve(promise, f64::from_bits(JSValue::null().bits()));
                }
            }
        } else {
            eprintln!("Redis client not found: {}", client_id);
            js_promise_resolve(promise, f64::from_bits(JSValue::null().bits()));
        }

        promise as i64
    }
}

/// Redis INCR command
/// js_redis_incr(client: i64, key: i64) -> Promise (i64)
#[no_mangle]
pub extern "C" fn js_redis_incr(client_handle: i64, key: i64) -> i64 {
    unsafe {
        let promise = js_promise_new();

        let key_str = match extract_string(key as *const StringHeader) {
            Some(s) => s,
            None => {
                js_promise_resolve(promise, f64::from_bits(JSValue::null().bits()));
                return promise as i64;
            }
        };

        let client_id = client_handle as u64;
        let mut clients = REDIS_CLIENTS.lock().unwrap();

        if let Some(conn) = clients.get_mut(&client_id) {
            match conn.incr::<_, i64, i64>(&key_str, 1) {
                Ok(value) => {
                    js_promise_resolve(promise, f64::from_bits(JSValue::number(value as f64).bits()));
                }
                Err(e) => {
                    eprintln!("Redis INCR error: {}", e);
                    js_promise_resolve(promise, f64::from_bits(JSValue::null().bits()));
                }
            }
        } else {
            eprintln!("Redis client not found: {}", client_id);
            js_promise_resolve(promise, f64::from_bits(JSValue::null().bits()));
        }

        promise as i64
    }
}

/// Redis DECR command
/// js_redis_decr(client: i64, key: i64) -> Promise (i64)
#[no_mangle]
pub extern "C" fn js_redis_decr(client_handle: i64, key: i64) -> i64 {
    unsafe {
        let promise = js_promise_new();

        let key_str = match extract_string(key as *const StringHeader) {
            Some(s) => s,
            None => {
                js_promise_resolve(promise, f64::from_bits(JSValue::null().bits()));
                return promise as i64;
            }
        };

        let client_id = client_handle as u64;
        let mut clients = REDIS_CLIENTS.lock().unwrap();

        if let Some(conn) = clients.get_mut(&client_id) {
            match conn.decr::<_, i64, i64>(&key_str, 1) {
                Ok(value) => {
                    js_promise_resolve(promise, f64::from_bits(JSValue::number(value as f64).bits()));
                }
                Err(e) => {
                    eprintln!("Redis DECR error: {}", e);
                    js_promise_resolve(promise, f64::from_bits(JSValue::null().bits()));
                }
            }
        } else {
            eprintln!("Redis client not found: {}", client_id);
            js_promise_resolve(promise, f64::from_bits(JSValue::null().bits()));
        }

        promise as i64
    }
}

/// Redis HGET command
/// js_redis_hget(client: i64, key: i64, field: i64) -> Promise (i64)
#[no_mangle]
pub extern "C" fn js_redis_hget(client_handle: i64, key: i64, field: i64) -> i64 {
    unsafe {
        let promise = js_promise_new();

        let key_str = match extract_string(key as *const StringHeader) {
            Some(s) => s,
            None => {
                js_promise_resolve(promise, f64::from_bits(JSValue::null().bits()));
                return promise as i64;
            }
        };

        let field_str = match extract_string(field as *const StringHeader) {
            Some(s) => s,
            None => {
                js_promise_resolve(promise, f64::from_bits(JSValue::null().bits()));
                return promise as i64;
            }
        };

        let client_id = client_handle as u64;
        let mut clients = REDIS_CLIENTS.lock().unwrap();

        if let Some(conn) = clients.get_mut(&client_id) {
            match conn.hget::<_, _, Option<String>>(&key_str, &field_str) {
                Ok(Some(value)) => {
                    let str_ptr = js_string_from_bytes(value.as_ptr(), value.len() as u32);
                    js_promise_resolve(promise, f64::from_bits(JSValue::string_ptr(str_ptr).bits()));
                }
                Ok(None) => {
                    js_promise_resolve(promise, f64::from_bits(JSValue::null().bits()));
                }
                Err(e) => {
                    eprintln!("Redis HGET error: {}", e);
                    js_promise_resolve(promise, f64::from_bits(JSValue::null().bits()));
                }
            }
        } else {
            eprintln!("Redis client not found: {}", client_id);
            js_promise_resolve(promise, f64::from_bits(JSValue::null().bits()));
        }

        promise as i64
    }
}

/// Redis HSET command
/// js_redis_hset(client: i64, key: i64, field: i64, value: i64) -> Promise (i64)
#[no_mangle]
pub extern "C" fn js_redis_hset(client_handle: i64, key: i64, field: i64, value: i64) -> i64 {
    unsafe {
        let promise = js_promise_new();

        let key_str = match extract_string(key as *const StringHeader) {
            Some(s) => s,
            None => {
                js_promise_resolve(promise, f64::from_bits(JSValue::null().bits()));
                return promise as i64;
            }
        };

        let field_str = match extract_string(field as *const StringHeader) {
            Some(s) => s,
            None => {
                js_promise_resolve(promise, f64::from_bits(JSValue::null().bits()));
                return promise as i64;
            }
        };

        let value_str = match extract_string(value as *const StringHeader) {
            Some(s) => s,
            None => {
                js_promise_resolve(promise, f64::from_bits(JSValue::null().bits()));
                return promise as i64;
            }
        };

        let client_id = client_handle as u64;
        let mut clients = REDIS_CLIENTS.lock().unwrap();

        if let Some(conn) = clients.get_mut(&client_id) {
            match conn.hset::<_, _, _, i64>(&key_str, &field_str, &value_str) {
                Ok(count) => {
                    js_promise_resolve(promise, f64::from_bits(JSValue::number(count as f64).bits()));
                }
                Err(e) => {
                    eprintln!("Redis HSET error: {}", e);
                    js_promise_resolve(promise, f64::from_bits(JSValue::null().bits()));
                }
            }
        } else {
            eprintln!("Redis client not found: {}", client_id);
            js_promise_resolve(promise, f64::from_bits(JSValue::null().bits()));
        }

        promise as i64
    }
}

/// Redis EXPIRE command
/// js_redis_expire(client: i64, key: i64, seconds: f64) -> Promise (i64)
#[no_mangle]
pub extern "C" fn js_redis_expire(client_handle: i64, key: i64, seconds: f64) -> i64 {
    unsafe {
        let promise = js_promise_new();

        let key_str = match extract_string(key as *const StringHeader) {
            Some(s) => s,
            None => {
                js_promise_resolve(promise, f64::from_bits(JSValue::null().bits()));
                return promise as i64;
            }
        };

        let client_id = client_handle as u64;
        let mut clients = REDIS_CLIENTS.lock().unwrap();

        if let Some(conn) = clients.get_mut(&client_id) {
            match conn.expire::<_, bool>(&key_str, seconds as i64) {
                Ok(result) => {
                    js_promise_resolve(promise, f64::from_bits(JSValue::bool(result).bits()));
                }
                Err(e) => {
                    eprintln!("Redis EXPIRE error: {}", e);
                    js_promise_resolve(promise, f64::from_bits(JSValue::null().bits()));
                }
            }
        } else {
            eprintln!("Redis client not found: {}", client_id);
            js_promise_resolve(promise, f64::from_bits(JSValue::null().bits()));
        }

        promise as i64
    }
}

/// Redis TTL command
/// js_redis_ttl(client: i64, key: i64) -> Promise (i64)
#[no_mangle]
pub extern "C" fn js_redis_ttl(client_handle: i64, key: i64) -> i64 {
    unsafe {
        let promise = js_promise_new();

        let key_str = match extract_string(key as *const StringHeader) {
            Some(s) => s,
            None => {
                js_promise_resolve(promise, f64::from_bits(JSValue::null().bits()));
                return promise as i64;
            }
        };

        let client_id = client_handle as u64;
        let mut clients = REDIS_CLIENTS.lock().unwrap();

        if let Some(conn) = clients.get_mut(&client_id) {
            match conn.ttl::<_, i64>(&key_str) {
                Ok(ttl) => {
                    js_promise_resolve(promise, f64::from_bits(JSValue::number(ttl as f64).bits()));
                }
                Err(e) => {
                    eprintln!("Redis TTL error: {}", e);
                    js_promise_resolve(promise, f64::from_bits(JSValue::null().bits()));
                }
            }
        } else {
            eprintln!("Redis client not found: {}", client_id);
            js_promise_resolve(promise, f64::from_bits(JSValue::null().bits()));
        }

        promise as i64
    }
}

/// Close a Redis client connection
/// js_redis_quit(client: i64) -> Promise (i64)
#[no_mangle]
pub extern "C" fn js_redis_quit(client_handle: i64) -> i64 {
    let promise = unsafe { js_promise_new() };

    let client_id = client_handle as u64;
    let mut clients = REDIS_CLIENTS.lock().unwrap();

    clients.remove(&client_id);
    unsafe {
        js_promise_resolve(promise, f64::from_bits(JSValue::undefined().bits()));
    }

    promise as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redis_url() {
        // Basic test - would need a mock config object for full testing
    }
}
