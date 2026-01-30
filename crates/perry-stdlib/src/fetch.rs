//! HTTP Fetch module (node-fetch compatible)
//!
//! Native implementation of the 'node-fetch' npm package using reqwest.
//! Provides fetch() function for making HTTP requests.

use perry_runtime::{
    js_array_alloc, js_array_push, js_object_alloc, js_object_set_field, js_object_set_keys,
    js_string_from_bytes, JSValue, StringHeader,
};
use std::collections::HashMap;
use std::sync::Mutex;

use crate::common::async_bridge::{queue_promise_resolution, spawn};

// Response handle storage
lazy_static::lazy_static! {
    static ref FETCH_RESPONSES: Mutex<HashMap<usize, FetchResponse>> = Mutex::new(HashMap::new());
    static ref NEXT_RESPONSE_ID: Mutex<usize> = Mutex::new(1);
}

struct FetchResponse {
    status: u16,
    status_text: String,
    headers: HashMap<String, String>,
    body: Vec<u8>,
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

/// Perform a GET request
/// fetch(url) -> Promise<Response>
#[no_mangle]
pub unsafe extern "C" fn js_fetch_get(url_ptr: *const StringHeader) -> *mut perry_runtime::Promise {
    let promise = perry_runtime::js_promise_new();
    let promise_ptr = promise as usize;

    let url = match string_from_header(url_ptr) {
        Some(u) => u,
        None => {
            let err_msg = "Invalid URL";
            let err_str = js_string_from_bytes(err_msg.as_ptr(), err_msg.len() as u32);
            let err_bits = JSValue::pointer(err_str as *const u8).bits();
            queue_promise_resolution(promise_ptr, false, err_bits);
            return promise;
        }
    };

    spawn(async move {
        let client = reqwest::Client::new();
        match client.get(&url).send().await {
            Ok(response) => {
                let status = response.status().as_u16();
                let status_text = response.status().canonical_reason().unwrap_or("").to_string();

                let mut headers = HashMap::new();
                for (key, value) in response.headers() {
                    if let Ok(v) = value.to_str() {
                        headers.insert(key.to_string(), v.to_string());
                    }
                }

                let body = response.bytes().await.unwrap_or_default().to_vec();

                // Store response
                let mut id_guard = NEXT_RESPONSE_ID.lock().unwrap();
                let response_id = *id_guard;
                *id_guard += 1;
                drop(id_guard);

                FETCH_RESPONSES.lock().unwrap().insert(response_id, FetchResponse {
                    status,
                    status_text,
                    headers,
                    body,
                });

                // Return response handle
                let result_bits = (response_id as f64).to_bits();
                queue_promise_resolution(promise_ptr, true, result_bits);
            }
            Err(e) => {
                let err_msg = format!("Fetch error: {}", e);
                let err_str = js_string_from_bytes(err_msg.as_ptr(), err_msg.len() as u32);
                let err_bits = JSValue::pointer(err_str as *const u8).bits();
                queue_promise_resolution(promise_ptr, false, err_bits);
            }
        }
    });

    promise
}

/// Perform a POST request with body
/// fetch(url, { method: 'POST', body: '...' }) -> Promise<Response>
#[no_mangle]
pub unsafe extern "C" fn js_fetch_post(
    url_ptr: *const StringHeader,
    body_ptr: *const StringHeader,
    content_type_ptr: *const StringHeader,
) -> *mut perry_runtime::Promise {
    let promise = perry_runtime::js_promise_new();
    let promise_ptr = promise as usize;

    let url = match string_from_header(url_ptr) {
        Some(u) => u,
        None => {
            let err_msg = "Invalid URL";
            let err_str = js_string_from_bytes(err_msg.as_ptr(), err_msg.len() as u32);
            let err_bits = JSValue::pointer(err_str as *const u8).bits();
            queue_promise_resolution(promise_ptr, false, err_bits);
            return promise;
        }
    };

    let body = string_from_header(body_ptr).unwrap_or_default();
    let content_type = string_from_header(content_type_ptr).unwrap_or_else(|| "application/json".to_string());

    spawn(async move {
        let client = reqwest::Client::new();
        match client
            .post(&url)
            .header("Content-Type", &content_type)
            .body(body)
            .send()
            .await
        {
            Ok(response) => {
                let status = response.status().as_u16();
                let status_text = response.status().canonical_reason().unwrap_or("").to_string();

                let mut headers = HashMap::new();
                for (key, value) in response.headers() {
                    if let Ok(v) = value.to_str() {
                        headers.insert(key.to_string(), v.to_string());
                    }
                }

                let body = response.bytes().await.unwrap_or_default().to_vec();

                // Store response
                let mut id_guard = NEXT_RESPONSE_ID.lock().unwrap();
                let response_id = *id_guard;
                *id_guard += 1;
                drop(id_guard);

                FETCH_RESPONSES.lock().unwrap().insert(response_id, FetchResponse {
                    status,
                    status_text,
                    headers,
                    body,
                });

                // Return response handle
                let result_bits = (response_id as f64).to_bits();
                queue_promise_resolution(promise_ptr, true, result_bits);
            }
            Err(e) => {
                let err_msg = format!("Fetch error: {}", e);
                let err_str = js_string_from_bytes(err_msg.as_ptr(), err_msg.len() as u32);
                let err_bits = JSValue::pointer(err_str as *const u8).bits();
                queue_promise_resolution(promise_ptr, false, err_bits);
            }
        }
    });

    promise
}

/// Perform a fetch request with full options (method, headers, body)
/// This is the most flexible fetch function
#[no_mangle]
pub unsafe extern "C" fn js_fetch_with_options(
    url_ptr: *const StringHeader,
    method_ptr: *const StringHeader,
    body_ptr: *const StringHeader,
    headers_json_ptr: *const StringHeader,
) -> *mut perry_runtime::Promise {
    let promise = perry_runtime::js_promise_new();
    let promise_ptr = promise as usize;

    let url = match string_from_header(url_ptr) {
        Some(u) => u,
        None => {
            let err_msg = "Invalid URL";
            let err_str = js_string_from_bytes(err_msg.as_ptr(), err_msg.len() as u32);
            let err_bits = JSValue::pointer(err_str as *const u8).bits();
            queue_promise_resolution(promise_ptr, false, err_bits);
            return promise;
        }
    };

    let method = string_from_header(method_ptr).unwrap_or_else(|| "GET".to_string());
    let body = string_from_header(body_ptr);
    let headers_json = string_from_header(headers_json_ptr).unwrap_or_else(|| "{}".to_string());

    // Parse headers from JSON
    let custom_headers: HashMap<String, String> = serde_json::from_str(&headers_json).unwrap_or_default();

    spawn(async move {
        let client = reqwest::Client::new();
        let mut request = match method.to_uppercase().as_str() {
            "POST" => client.post(&url),
            "PUT" => client.put(&url),
            "DELETE" => client.delete(&url),
            "PATCH" => client.patch(&url),
            "HEAD" => client.head(&url),
            _ => client.get(&url), // Default to GET
        };

        // Add custom headers
        for (key, value) in &custom_headers {
            request = request.header(key.as_str(), value.as_str());
        }

        // Add body if present
        if let Some(b) = body {
            request = request.body(b);
        }

        match request.send().await {
            Ok(response) => {
                let status = response.status().as_u16();
                let status_text = response.status().canonical_reason().unwrap_or("").to_string();

                let mut headers = HashMap::new();
                for (key, value) in response.headers() {
                    if let Ok(v) = value.to_str() {
                        headers.insert(key.to_string(), v.to_string());
                    }
                }

                let body = response.bytes().await.unwrap_or_default().to_vec();

                // Store response
                let mut id_guard = NEXT_RESPONSE_ID.lock().unwrap();
                let response_id = *id_guard;
                *id_guard += 1;
                drop(id_guard);

                FETCH_RESPONSES.lock().unwrap().insert(response_id, FetchResponse {
                    status,
                    status_text,
                    headers,
                    body,
                });

                // Return response handle
                let result_bits = (response_id as f64).to_bits();
                queue_promise_resolution(promise_ptr, true, result_bits);
            }
            Err(e) => {
                let err_msg = format!("Fetch error: {}", e);
                let err_str = js_string_from_bytes(err_msg.as_ptr(), err_msg.len() as u32);
                let err_bits = JSValue::pointer(err_str as *const u8).bits();
                queue_promise_resolution(promise_ptr, false, err_bits);
            }
        }
    });

    promise
}

/// Get response status code
/// response.status -> number
#[no_mangle]
pub extern "C" fn js_fetch_response_status(handle: i64) -> f64 {
    let response_id = handle as usize;
    let guard = FETCH_RESPONSES.lock().unwrap();
    match guard.get(&response_id) {
        Some(resp) => resp.status as f64,
        None => 0.0,
    }
}

/// Get response status text
/// response.statusText -> string
#[no_mangle]
pub extern "C" fn js_fetch_response_status_text(handle: i64) -> *mut StringHeader {
    let response_id = handle as usize;
    let guard = FETCH_RESPONSES.lock().unwrap();
    match guard.get(&response_id) {
        Some(resp) => {
            js_string_from_bytes(resp.status_text.as_ptr(), resp.status_text.len() as u32)
        }
        None => std::ptr::null_mut(),
    }
}

/// Check if response was successful (status 200-299)
/// response.ok -> boolean
#[no_mangle]
pub extern "C" fn js_fetch_response_ok(handle: i64) -> f64 {
    let response_id = handle as usize;
    let guard = FETCH_RESPONSES.lock().unwrap();
    match guard.get(&response_id) {
        Some(resp) => if resp.status >= 200 && resp.status < 300 { 1.0 } else { 0.0 },
        None => 0.0,
    }
}

/// Get response body as text
/// response.text() -> Promise<string>
#[no_mangle]
pub unsafe extern "C" fn js_fetch_response_text(handle: i64) -> *mut perry_runtime::Promise {
    let promise = perry_runtime::js_promise_new();
    let promise_ptr = promise as usize;
    let response_id = handle as usize;

    let body = {
        let guard = FETCH_RESPONSES.lock().unwrap();
        match guard.get(&response_id) {
            Some(resp) => resp.body.clone(),
            None => {
                let err_msg = "Invalid response handle";
                let err_str = js_string_from_bytes(err_msg.as_ptr(), err_msg.len() as u32);
                let err_bits = JSValue::pointer(err_str as *const u8).bits();
                queue_promise_resolution(promise_ptr, false, err_bits);
                return promise;
            }
        }
    };

    // Convert body to string
    let text = String::from_utf8_lossy(&body).to_string();
    let result_str = js_string_from_bytes(text.as_ptr(), text.len() as u32);
    let result_bits = JSValue::string_ptr(result_str).bits();
    queue_promise_resolution(promise_ptr, true, result_bits);

    promise
}

/// Convert serde_json::Value to JSValue
unsafe fn json_value_to_jsvalue(value: &serde_json::Value) -> JSValue {
    match value {
        serde_json::Value::Null => JSValue::null(),
        serde_json::Value::Bool(b) => JSValue::bool(*b),
        serde_json::Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                JSValue::number(f)
            } else if let Some(i) = n.as_i64() {
                JSValue::number(i as f64)
            } else {
                JSValue::number(0.0)
            }
        }
        serde_json::Value::String(s) => {
            let ptr = js_string_from_bytes(s.as_ptr(), s.len() as u32);
            JSValue::string_ptr(ptr)
        }
        serde_json::Value::Array(arr) => {
            let js_arr = js_array_alloc(arr.len() as u32);
            for item in arr {
                js_array_push(js_arr, json_value_to_jsvalue(item));
            }
            JSValue::object_ptr(js_arr as *mut u8)
        }
        serde_json::Value::Object(obj) => {
            let js_obj = js_object_alloc(0, obj.len() as u32);
            // Create keys array for property names
            let keys_arr = js_array_alloc(obj.len() as u32);
            for (idx, (key, val)) in obj.iter().enumerate() {
                // Add key to keys array
                let key_ptr = js_string_from_bytes(key.as_ptr(), key.len() as u32);
                js_array_push(keys_arr, JSValue::string_ptr(key_ptr));
                // Set field value
                js_object_set_field(js_obj, idx as u32, json_value_to_jsvalue(val));
            }
            // Associate keys with object
            js_object_set_keys(js_obj, keys_arr);
            JSValue::object_ptr(js_obj as *mut u8)
        }
    }
}

/// Get response body as JSON (parses and returns proper JS object)
/// response.json() -> Promise<object>
#[no_mangle]
pub unsafe extern "C" fn js_fetch_response_json(handle: i64) -> *mut perry_runtime::Promise {
    let promise = perry_runtime::js_promise_new();
    let promise_ptr = promise as usize;
    let response_id = handle as usize;

    let body = {
        let guard = FETCH_RESPONSES.lock().unwrap();
        match guard.get(&response_id) {
            Some(resp) => resp.body.clone(),
            None => {
                let err_msg = "Invalid response handle";
                let err_str = js_string_from_bytes(err_msg.as_ptr(), err_msg.len() as u32);
                let err_bits = JSValue::pointer(err_str as *const u8).bits();
                queue_promise_resolution(promise_ptr, false, err_bits);
                return promise;
            }
        }
    };

    // Convert body to string and parse as JSON
    let text = String::from_utf8_lossy(&body).to_string();
    match serde_json::from_str::<serde_json::Value>(&text) {
        Ok(json_value) => {
            let js_value = json_value_to_jsvalue(&json_value);
            queue_promise_resolution(promise_ptr, true, js_value.bits());
        }
        Err(e) => {
            let err_msg = format!("JSON parse error: {}", e);
            let err_str = js_string_from_bytes(err_msg.as_ptr(), err_msg.len() as u32);
            let err_bits = JSValue::pointer(err_str as *const u8).bits();
            queue_promise_resolution(promise_ptr, false, err_bits);
        }
    }

    promise
}

/// Simple fetch that returns text directly (convenience function)
/// fetchText(url) -> Promise<string>
#[no_mangle]
pub unsafe extern "C" fn js_fetch_text(url_ptr: *const StringHeader) -> *mut perry_runtime::Promise {
    let promise = perry_runtime::js_promise_new();
    let promise_ptr = promise as usize;

    let url = match string_from_header(url_ptr) {
        Some(u) => u,
        None => {
            let err_msg = "Invalid URL";
            let err_str = js_string_from_bytes(err_msg.as_ptr(), err_msg.len() as u32);
            let err_bits = JSValue::pointer(err_str as *const u8).bits();
            queue_promise_resolution(promise_ptr, false, err_bits);
            return promise;
        }
    };

    spawn(async move {
        let client = reqwest::Client::new();
        match client.get(&url).send().await {
            Ok(response) => {
                match response.text().await {
                    Ok(text) => {
                        let result_str = js_string_from_bytes(text.as_ptr(), text.len() as u32);
                        let result_bits = JSValue::pointer(result_str as *const u8).bits();
                        queue_promise_resolution(promise_ptr, true, result_bits);
                    }
                    Err(e) => {
                        let err_msg = format!("Read error: {}", e);
                        let err_str = js_string_from_bytes(err_msg.as_ptr(), err_msg.len() as u32);
                        let err_bits = JSValue::pointer(err_str as *const u8).bits();
                        queue_promise_resolution(promise_ptr, false, err_bits);
                    }
                }
            }
            Err(e) => {
                let err_msg = format!("Fetch error: {}", e);
                let err_str = js_string_from_bytes(err_msg.as_ptr(), err_msg.len() as u32);
                let err_bits = JSValue::pointer(err_str as *const u8).bits();
                queue_promise_resolution(promise_ptr, false, err_bits);
            }
        }
    });

    promise
}
