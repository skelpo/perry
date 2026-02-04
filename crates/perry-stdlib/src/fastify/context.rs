//! Request/Reply context objects
//!
//! Provides a unified context for both Fastify and Hono style request handling.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use perry_runtime::{js_string_from_bytes, StringHeader, JSValue};

use crate::common::{get_handle, get_handle_mut, register_handle, Handle};

/// Context ID counter
static CONTEXT_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Helper to extract string from StringHeader pointer
pub(crate) unsafe fn string_from_header(ptr: *const StringHeader) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    let len = (*ptr).length as usize;
    let data_ptr = (ptr as *const u8).add(std::mem::size_of::<StringHeader>());
    let bytes = std::slice::from_raw_parts(data_ptr, len);
    Some(String::from_utf8_lossy(bytes).to_string())
}

/// Helper to extract string from raw i64 pointer (NaN-boxed or raw)
pub(crate) unsafe fn string_from_nanboxed(value: i64) -> Option<String> {
    let ptr = perry_runtime::js_get_string_pointer_unified(f64::from_bits(value as u64));
    if ptr == 0 {
        return None;
    }
    string_from_header(ptr as *const StringHeader)
}

/// Unified context for both Fastify and Hono styles
pub struct FastifyContext {
    /// Unique context ID
    pub id: u64,
    /// Request ID (from underlying server)
    pub request_id: u64,
    /// HTTP method
    pub method: String,
    /// Request URL path
    pub url: String,
    /// Query string (without leading ?)
    pub query_string: String,
    /// Extracted route parameters
    pub params: HashMap<String, String>,
    /// Request headers
    pub headers: HashMap<String, String>,
    /// Request body (raw bytes)
    pub body: Option<Vec<u8>>,

    // Reply state
    /// Response status code
    pub status_code: u16,
    /// Response headers
    pub response_headers: Vec<(String, String)>,
    /// Whether response has been sent
    pub sent: bool,
    /// Response body (if built incrementally)
    pub response_body: Option<Vec<u8>>,
}

impl FastifyContext {
    /// Create a new context
    pub fn new(
        request_id: u64,
        method: String,
        url: String,
        headers: HashMap<String, String>,
        body: Option<Vec<u8>>,
        params: HashMap<String, String>,
    ) -> Self {
        // Parse query string from URL
        let (path, query_string) = match url.split_once('?') {
            Some((p, q)) => (p.to_string(), q.to_string()),
            None => (url.clone(), String::new()),
        };

        Self {
            id: CONTEXT_ID_COUNTER.fetch_add(1, Ordering::SeqCst),
            request_id,
            method,
            url: path,
            query_string,
            params,
            headers,
            body,
            status_code: 200,
            response_headers: Vec::new(),
            sent: false,
            response_body: None,
        }
    }

    /// Get a request header value
    pub fn get_header(&self, name: &str) -> Option<&str> {
        self.headers.get(&name.to_lowercase()).map(|s| s.as_str())
    }

    /// Get a route parameter value
    pub fn get_param(&self, name: &str) -> Option<&str> {
        self.params.get(name).map(|s| s.as_str())
    }

    /// Get a query parameter value
    pub fn get_query_param(&self, name: &str) -> Option<String> {
        for pair in self.query_string.split('&') {
            if let Some((key, value)) = pair.split_once('=') {
                if key == name {
                    return Some(urlencoding_decode(value));
                }
            }
        }
        None
    }

    /// Get all query parameters as a map
    pub fn get_query_params(&self) -> HashMap<String, String> {
        let mut params = HashMap::new();
        for pair in self.query_string.split('&') {
            if let Some((key, value)) = pair.split_once('=') {
                params.insert(key.to_string(), urlencoding_decode(value));
            }
        }
        params
    }

    /// Get body as string
    pub fn body_string(&self) -> Option<String> {
        self.body.as_ref().map(|b| String::from_utf8_lossy(b).to_string())
    }

    /// Set response status code
    pub fn set_status(&mut self, code: u16) {
        self.status_code = code;
    }

    /// Add a response header
    pub fn add_header(&mut self, name: &str, value: &str) {
        self.response_headers.push((name.to_string(), value.to_string()));
    }
}

/// Simple URL decoding
fn urlencoding_decode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                result.push(byte as char);
            } else {
                result.push('%');
                result.push_str(&hex);
            }
        } else if c == '+' {
            result.push(' ');
        } else {
            result.push(c);
        }
    }

    result
}

// ============================================================================
// Request Methods FFI
// ============================================================================

/// Get request method
#[no_mangle]
pub unsafe extern "C" fn js_fastify_req_method(ctx_handle: Handle) -> *mut StringHeader {
    if let Some(ctx) = get_handle::<FastifyContext>(ctx_handle) {
        return js_string_from_bytes(ctx.method.as_ptr(), ctx.method.len() as u32);
    }
    std::ptr::null_mut()
}

/// Get request URL path
#[no_mangle]
pub unsafe extern "C" fn js_fastify_req_url(ctx_handle: Handle) -> *mut StringHeader {
    if let Some(ctx) = get_handle::<FastifyContext>(ctx_handle) {
        return js_string_from_bytes(ctx.url.as_ptr(), ctx.url.len() as u32);
    }
    std::ptr::null_mut()
}

/// Get all route params as JSON object
#[no_mangle]
pub unsafe extern "C" fn js_fastify_req_params(ctx_handle: Handle) -> *mut StringHeader {
    if let Some(ctx) = get_handle::<FastifyContext>(ctx_handle) {
        if let Ok(json) = serde_json::to_string(&ctx.params) {
            return js_string_from_bytes(json.as_ptr(), json.len() as u32);
        }
    }
    std::ptr::null_mut()
}

/// Get a single route param (Hono style)
#[no_mangle]
pub unsafe extern "C" fn js_fastify_req_param(ctx_handle: Handle, name: i64) -> *mut StringHeader {
    let name = match string_from_nanboxed(name) {
        Some(n) => n,
        None => return std::ptr::null_mut(),
    };

    if let Some(ctx) = get_handle::<FastifyContext>(ctx_handle) {
        if let Some(value) = ctx.params.get(&name) {
            return js_string_from_bytes(value.as_ptr(), value.len() as u32);
        }
    }
    std::ptr::null_mut()
}

/// Get all query params as JSON object
#[no_mangle]
pub unsafe extern "C" fn js_fastify_req_query(ctx_handle: Handle) -> *mut StringHeader {
    if let Some(ctx) = get_handle::<FastifyContext>(ctx_handle) {
        let params = ctx.get_query_params();
        if let Ok(json) = serde_json::to_string(&params) {
            return js_string_from_bytes(json.as_ptr(), json.len() as u32);
        }
    }
    std::ptr::null_mut()
}

/// Get raw request body as string
#[no_mangle]
pub unsafe extern "C" fn js_fastify_req_body(ctx_handle: Handle) -> *mut StringHeader {
    if let Some(ctx) = get_handle::<FastifyContext>(ctx_handle) {
        if let Some(body) = ctx.body_string() {
            return js_string_from_bytes(body.as_ptr(), body.len() as u32);
        }
    }
    std::ptr::null_mut()
}

/// Get parsed JSON body (returns NaN-boxed object or undefined)
#[no_mangle]
pub unsafe extern "C" fn js_fastify_req_json(ctx_handle: Handle) -> f64 {
    if let Some(ctx) = get_handle::<FastifyContext>(ctx_handle) {
        if let Some(body) = ctx.body_string() {
            // Parse JSON and create object
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&body) {
                return json_value_to_jsvalue(&value);
            }
        }
    }
    f64::from_bits(JSValue::undefined().bits())
}

/// Get all headers as JSON object
#[no_mangle]
pub unsafe extern "C" fn js_fastify_req_headers(ctx_handle: Handle) -> *mut StringHeader {
    if let Some(ctx) = get_handle::<FastifyContext>(ctx_handle) {
        if let Ok(json) = serde_json::to_string(&ctx.headers) {
            return js_string_from_bytes(json.as_ptr(), json.len() as u32);
        }
    }
    std::ptr::null_mut()
}

/// Get a single header value
#[no_mangle]
pub unsafe extern "C" fn js_fastify_req_header(ctx_handle: Handle, name: i64) -> *mut StringHeader {
    let name = match string_from_nanboxed(name) {
        Some(n) => n.to_lowercase(),
        None => return std::ptr::null_mut(),
    };

    if let Some(ctx) = get_handle::<FastifyContext>(ctx_handle) {
        if let Some(value) = ctx.headers.get(&name) {
            return js_string_from_bytes(value.as_ptr(), value.len() as u32);
        }
    }
    std::ptr::null_mut()
}

// ============================================================================
// Reply Methods FFI (Fastify style)
// ============================================================================

/// Set response status code (chainable, returns handle)
#[no_mangle]
pub unsafe extern "C" fn js_fastify_reply_status(ctx_handle: Handle, code: f64) -> Handle {
    if let Some(ctx) = get_handle_mut::<FastifyContext>(ctx_handle) {
        ctx.status_code = code as u16;
    }
    ctx_handle
}

/// Set a response header (chainable, returns handle)
#[no_mangle]
pub unsafe extern "C" fn js_fastify_reply_header(ctx_handle: Handle, name: i64, value: i64) -> Handle {
    let name = match string_from_nanboxed(name) {
        Some(n) => n,
        None => return ctx_handle,
    };
    let value = match string_from_nanboxed(value) {
        Some(v) => v,
        None => return ctx_handle,
    };

    if let Some(ctx) = get_handle_mut::<FastifyContext>(ctx_handle) {
        ctx.response_headers.push((name, value));
    }
    ctx_handle
}

/// Send response (Fastify style)
/// Returns true if sent, false otherwise
#[no_mangle]
pub unsafe extern "C" fn js_fastify_reply_send(ctx_handle: Handle, data: f64) -> bool {
    if let Some(ctx) = get_handle_mut::<FastifyContext>(ctx_handle) {
        if ctx.sent {
            return false;
        }

        // Convert data to response body
        let body = jsvalue_to_response_body(data);
        ctx.response_body = Some(body);
        ctx.sent = true;
        return true;
    }
    false
}

// ============================================================================
// Context Methods FFI (Hono style)
// ============================================================================

/// Send JSON response (Hono style)
/// Returns a response marker value
#[no_mangle]
pub unsafe extern "C" fn js_fastify_ctx_json(ctx_handle: Handle, data: f64, status: f64) -> f64 {
    if let Some(ctx) = get_handle_mut::<FastifyContext>(ctx_handle) {
        if status > 0.0 {
            ctx.status_code = status as u16;
        }

        // Add content-type header
        ctx.response_headers.push(("content-type".to_string(), "application/json; charset=utf-8".to_string()));

        // Convert data to JSON string
        let body = jsvalue_to_json_string(data);
        ctx.response_body = Some(body.into_bytes());
        ctx.sent = true;
    }

    // Return undefined (response is implicit)
    f64::from_bits(JSValue::undefined().bits())
}

/// Send text response (Hono style)
#[no_mangle]
pub unsafe extern "C" fn js_fastify_ctx_text(ctx_handle: Handle, text: i64, status: f64) -> f64 {
    let text = string_from_nanboxed(text).unwrap_or_default();

    if let Some(ctx) = get_handle_mut::<FastifyContext>(ctx_handle) {
        if status > 0.0 {
            ctx.status_code = status as u16;
        }

        ctx.response_headers.push(("content-type".to_string(), "text/plain; charset=utf-8".to_string()));
        ctx.response_body = Some(text.into_bytes());
        ctx.sent = true;
    }

    f64::from_bits(JSValue::undefined().bits())
}

/// Send HTML response (Hono style)
#[no_mangle]
pub unsafe extern "C" fn js_fastify_ctx_html(ctx_handle: Handle, html: i64, status: f64) -> f64 {
    let html = string_from_nanboxed(html).unwrap_or_default();

    if let Some(ctx) = get_handle_mut::<FastifyContext>(ctx_handle) {
        if status > 0.0 {
            ctx.status_code = status as u16;
        }

        ctx.response_headers.push(("content-type".to_string(), "text/html; charset=utf-8".to_string()));
        ctx.response_body = Some(html.into_bytes());
        ctx.sent = true;
    }

    f64::from_bits(JSValue::undefined().bits())
}

/// Send redirect response (Hono style)
#[no_mangle]
pub unsafe extern "C" fn js_fastify_ctx_redirect(ctx_handle: Handle, url: i64, status: f64) -> f64 {
    let url = string_from_nanboxed(url).unwrap_or_default();

    if let Some(ctx) = get_handle_mut::<FastifyContext>(ctx_handle) {
        ctx.status_code = if status > 0.0 { status as u16 } else { 302 };
        ctx.response_headers.push(("location".to_string(), url));
        ctx.response_body = Some(Vec::new());
        ctx.sent = true;
    }

    f64::from_bits(JSValue::undefined().bits())
}

// ============================================================================
// Helper functions
// ============================================================================

/// Convert a JSValue to a response body (bytes)
unsafe fn jsvalue_to_response_body(value: f64) -> Vec<u8> {
    let jsv = JSValue::from_bits(value.to_bits());

    // Check if it's a string
    if jsv.is_string() {
        if let Some(s) = extract_jsvalue_string(value) {
            return s.into_bytes();
        }
    }

    // Otherwise serialize as JSON
    jsvalue_to_json_string(value).into_bytes()
}

/// Convert a JSValue to a JSON string
unsafe fn jsvalue_to_json_string(value: f64) -> String {
    let jsv = JSValue::from_bits(value.to_bits());

    if jsv.is_undefined() {
        return "null".to_string();
    }
    if jsv.is_null() {
        return "null".to_string();
    }
    if jsv.is_bool() {
        return if jsv.as_bool() { "true".to_string() } else { "false".to_string() };
    }
    if jsv.is_number() {
        return format!("{}", value);
    }
    if jsv.is_string() {
        if let Some(s) = extract_jsvalue_string(value) {
            // Escape string for JSON
            return serde_json::to_string(&s).unwrap_or_else(|_| format!("\"{}\"", s));
        }
    }

    // For objects/arrays, use runtime's JSON stringify
    let str_ptr = perry_runtime::js_jsvalue_to_string(value);
    if !str_ptr.is_null() {
        if let Some(s) = string_from_header(str_ptr) {
            return s;
        }
    }

    "null".to_string()
}

/// Extract string from JSValue
unsafe fn extract_jsvalue_string(value: f64) -> Option<String> {
    let ptr = perry_runtime::js_get_string_pointer_unified(value);
    if ptr == 0 {
        return None;
    }
    string_from_header(ptr as *const StringHeader)
}

/// Convert serde_json::Value to JSValue (as f64)
unsafe fn json_value_to_jsvalue(value: &serde_json::Value) -> f64 {
    match value {
        serde_json::Value::Null => f64::from_bits(JSValue::null().bits()),
        serde_json::Value::Bool(b) => f64::from_bits(JSValue::bool(*b).bits()),
        serde_json::Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                f
            } else {
                0.0
            }
        }
        serde_json::Value::String(s) => {
            let ptr = js_string_from_bytes(s.as_ptr(), s.len() as u32);
            f64::from_bits(JSValue::string_ptr(ptr).bits())
        }
        serde_json::Value::Array(arr) => {
            let js_arr = perry_runtime::js_array_alloc(arr.len() as u32);
            for item in arr {
                let js_item = json_value_to_jsvalue(item);
                perry_runtime::js_array_push_f64(js_arr, js_item);
            }
            f64::from_bits(JSValue::pointer(js_arr as *const u8).bits())
        }
        serde_json::Value::Object(obj) => {
            let js_obj = perry_runtime::js_object_alloc(0, obj.len() as u32);
            for (key, val) in obj {
                let js_key = js_string_from_bytes(key.as_ptr(), key.len() as u32);
                let js_val = json_value_to_jsvalue(val);
                perry_runtime::js_object_set_field_by_name(js_obj, js_key, js_val);
            }
            f64::from_bits(JSValue::pointer(js_obj as *const u8).bits())
        }
    }
}
