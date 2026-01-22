//! Bcrypt password hashing module
//!
//! Native implementation of the 'bcrypt' npm package using the Rust bcrypt crate.
//! Provides secure password hashing and verification.

use perry_runtime::{js_string_from_bytes, JSValue, StringHeader};

use crate::common::async_bridge::{queue_promise_resolution, spawn};

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

/// Hash a password with the given cost factor
/// bcrypt.hash(password, saltRounds) -> Promise<string>
#[no_mangle]
pub unsafe extern "C" fn js_bcrypt_hash(
    password_ptr: *const StringHeader,
    salt_rounds: f64,
) -> *mut perry_runtime::Promise {
    let promise = perry_runtime::js_promise_new();
    let promise_ptr = promise as usize;

    let password = match string_from_header(password_ptr) {
        Some(s) => s,
        None => {
            let err_msg = "Password is null or invalid UTF-8";
            let err_str = js_string_from_bytes(err_msg.as_ptr(), err_msg.len() as u32);
            let err_bits = JSValue::pointer(err_str as *const u8).bits();
            queue_promise_resolution(promise_ptr, false, err_bits);
            return promise;
        }
    };

    let cost = salt_rounds as u32;

    // Spawn async task for hashing (bcrypt is CPU-intensive)
    spawn(async move {
        let result = tokio::task::spawn_blocking(move || {
            bcrypt::hash(password, cost)
        }).await;

        match result {
            Ok(Ok(hash)) => {
                let hash_str = js_string_from_bytes(hash.as_ptr(), hash.len() as u32);
                let result_bits = JSValue::pointer(hash_str as *const u8).bits();
                queue_promise_resolution(promise_ptr, true, result_bits);
            }
            Ok(Err(e)) => {
                let err_msg = format!("Bcrypt error: {}", e);
                let err_str = js_string_from_bytes(err_msg.as_ptr(), err_msg.len() as u32);
                let err_bits = JSValue::pointer(err_str as *const u8).bits();
                queue_promise_resolution(promise_ptr, false, err_bits);
            }
            Err(e) => {
                let err_msg = format!("Task error: {}", e);
                let err_str = js_string_from_bytes(err_msg.as_ptr(), err_msg.len() as u32);
                let err_bits = JSValue::pointer(err_str as *const u8).bits();
                queue_promise_resolution(promise_ptr, false, err_bits);
            }
        }
    });

    promise
}

/// Compare a password with a hash
/// bcrypt.compare(password, hash) -> Promise<boolean>
#[no_mangle]
pub unsafe extern "C" fn js_bcrypt_compare(
    password_ptr: *const StringHeader,
    hash_ptr: *const StringHeader,
) -> *mut perry_runtime::Promise {
    let promise = perry_runtime::js_promise_new();
    let promise_ptr = promise as usize;

    let password = match string_from_header(password_ptr) {
        Some(s) => s,
        None => {
            let err_msg = "Password is null or invalid UTF-8";
            let err_str = js_string_from_bytes(err_msg.as_ptr(), err_msg.len() as u32);
            let err_bits = JSValue::pointer(err_str as *const u8).bits();
            queue_promise_resolution(promise_ptr, false, err_bits);
            return promise;
        }
    };

    let hash = match string_from_header(hash_ptr) {
        Some(s) => s,
        None => {
            let err_msg = "Hash is null or invalid UTF-8";
            let err_str = js_string_from_bytes(err_msg.as_ptr(), err_msg.len() as u32);
            let err_bits = JSValue::pointer(err_str as *const u8).bits();
            queue_promise_resolution(promise_ptr, false, err_bits);
            return promise;
        }
    };

    // Spawn async task for verification (bcrypt is CPU-intensive)
    spawn(async move {
        let result = tokio::task::spawn_blocking(move || {
            bcrypt::verify(password, &hash)
        }).await;

        match result {
            Ok(Ok(matches)) => {
                // Return boolean as f64 (1.0 for true, 0.0 for false)
                let result_bits = if matches { 1.0f64.to_bits() } else { 0.0f64.to_bits() };
                queue_promise_resolution(promise_ptr, true, result_bits);
            }
            Ok(Err(e)) => {
                let err_msg = format!("Bcrypt verify error: {}", e);
                let err_str = js_string_from_bytes(err_msg.as_ptr(), err_msg.len() as u32);
                let err_bits = JSValue::pointer(err_str as *const u8).bits();
                queue_promise_resolution(promise_ptr, false, err_bits);
            }
            Err(e) => {
                let err_msg = format!("Task error: {}", e);
                let err_str = js_string_from_bytes(err_msg.as_ptr(), err_msg.len() as u32);
                let err_bits = JSValue::pointer(err_str as *const u8).bits();
                queue_promise_resolution(promise_ptr, false, err_bits);
            }
        }
    });

    promise
}

/// Generate a salt with the given cost factor
/// bcrypt.genSalt(rounds) -> Promise<string>
#[no_mangle]
pub unsafe extern "C" fn js_bcrypt_gen_salt(
    rounds: f64,
) -> *mut perry_runtime::Promise {
    let promise = perry_runtime::js_promise_new();
    let promise_ptr = promise as usize;
    let cost = rounds as u32;

    // Spawn async task
    spawn(async move {
        let result = tokio::task::spawn_blocking(move || {
            // Generate a random salt with the given cost
            // The bcrypt crate doesn't expose salt generation directly,
            // so we generate a dummy hash and extract the salt prefix
            let dummy = bcrypt::hash("", cost);
            match dummy {
                Ok(h) => {
                    // bcrypt hash format: $2b$XX$<22-char-salt><31-char-hash>
                    // We return the full salt portion including the prefix
                    if h.len() >= 29 {
                        Ok(h[..29].to_string())
                    } else {
                        Err("Invalid hash format".to_string())
                    }
                }
                Err(e) => Err(format!("{}", e))
            }
        }).await;

        match result {
            Ok(Ok(salt)) => {
                let salt_str = js_string_from_bytes(salt.as_ptr(), salt.len() as u32);
                let result_bits = JSValue::pointer(salt_str as *const u8).bits();
                queue_promise_resolution(promise_ptr, true, result_bits);
            }
            Ok(Err(e)) => {
                let err_str = js_string_from_bytes(e.as_ptr(), e.len() as u32);
                let err_bits = JSValue::pointer(err_str as *const u8).bits();
                queue_promise_resolution(promise_ptr, false, err_bits);
            }
            Err(e) => {
                let err_msg = format!("Task error: {}", e);
                let err_str = js_string_from_bytes(err_msg.as_ptr(), err_msg.len() as u32);
                let err_bits = JSValue::pointer(err_str as *const u8).bits();
                queue_promise_resolution(promise_ptr, false, err_bits);
            }
        }
    });

    promise
}

/// Hash a password synchronously
/// bcrypt.hashSync(password, saltRounds) -> string
#[no_mangle]
pub unsafe extern "C" fn js_bcrypt_hash_sync(
    password_ptr: *const StringHeader,
    salt_rounds: f64,
) -> *mut StringHeader {
    let password = match string_from_header(password_ptr) {
        Some(s) => s,
        None => return std::ptr::null_mut(),
    };

    let cost = salt_rounds as u32;

    match bcrypt::hash(password, cost) {
        Ok(hash) => js_string_from_bytes(hash.as_ptr(), hash.len() as u32),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Compare a password with a hash synchronously
/// bcrypt.compareSync(password, hash) -> boolean
#[no_mangle]
pub unsafe extern "C" fn js_bcrypt_compare_sync(
    password_ptr: *const StringHeader,
    hash_ptr: *const StringHeader,
) -> f64 {
    let password = match string_from_header(password_ptr) {
        Some(s) => s,
        None => return 0.0,
    };

    let hash = match string_from_header(hash_ptr) {
        Some(s) => s,
        None => return 0.0,
    };

    match bcrypt::verify(password, &hash) {
        Ok(true) => 1.0,
        _ => 0.0,
    }
}
