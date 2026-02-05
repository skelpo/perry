//! Handle-based method dispatch for perry-stdlib
//!
//! When native modules (Fastify, ioredis, etc.) use handle-based objects,
//! and those handles are passed to functions as generic parameters,
//! the codegen can't statically determine the type. This module provides
//! runtime dispatch by checking the handle type in the registry.

use super::handle::*;

/// Dispatch a method call on a handle-based object.
/// Called from perry-runtime's js_native_call_method when it detects a handle
/// (pointer value < 0x100000, indicating an integer handle, not a real heap pointer).
#[no_mangle]
pub unsafe extern "C" fn js_handle_method_dispatch(
    handle: i64,
    method_name_ptr: *const u8,
    method_name_len: usize,
    args_ptr: *const f64,
    args_len: usize,
) -> f64 {
    let method_name = if method_name_ptr.is_null() || method_name_len == 0 {
        ""
    } else {
        std::str::from_utf8(std::slice::from_raw_parts(method_name_ptr, method_name_len))
            .unwrap_or("")
    };

    let args: &[f64] = if args_len > 0 && !args_ptr.is_null() {
        std::slice::from_raw_parts(args_ptr, args_len)
    } else {
        &[]
    };

    // Try Fastify app dispatch
    if with_handle::<crate::fastify::FastifyApp, bool, _>(handle, |_| true).unwrap_or(false) {
        return dispatch_fastify_app(handle, method_name, args);
    }

    // Try Fastify context dispatch (request/reply)
    if with_handle::<crate::fastify::FastifyContext, bool, _>(handle, |_| true).unwrap_or(false) {
        return dispatch_fastify_context(handle, method_name, args);
    }

    // Unknown handle type - return undefined
    f64::from_bits(0x7FF8_0000_0000_0001)
}

/// Dispatch method calls on Fastify app handles
unsafe fn dispatch_fastify_app(handle: i64, method: &str, args: &[f64]) -> f64 {
    match method {
        "get" if args.len() >= 2 => {
            let path = args[0].to_bits() as i64;
            let handler = args[1].to_bits() as i64;
            let result = crate::fastify::js_fastify_get(handle, path, handler);
            if result { 1.0 } else { 0.0 }
        }
        "post" if args.len() >= 2 => {
            let path = args[0].to_bits() as i64;
            let handler = args[1].to_bits() as i64;
            let result = crate::fastify::js_fastify_post(handle, path, handler);
            if result { 1.0 } else { 0.0 }
        }
        "put" if args.len() >= 2 => {
            let path = args[0].to_bits() as i64;
            let handler = args[1].to_bits() as i64;
            let result = crate::fastify::js_fastify_put(handle, path, handler);
            if result { 1.0 } else { 0.0 }
        }
        "delete" if args.len() >= 2 => {
            let path = args[0].to_bits() as i64;
            let handler = args[1].to_bits() as i64;
            let result = crate::fastify::js_fastify_delete(handle, path, handler);
            if result { 1.0 } else { 0.0 }
        }
        "patch" if args.len() >= 2 => {
            let path = args[0].to_bits() as i64;
            let handler = args[1].to_bits() as i64;
            let result = crate::fastify::js_fastify_patch(handle, path, handler);
            if result { 1.0 } else { 0.0 }
        }
        "head" if args.len() >= 2 => {
            let path = args[0].to_bits() as i64;
            let handler = args[1].to_bits() as i64;
            let result = crate::fastify::js_fastify_head(handle, path, handler);
            if result { 1.0 } else { 0.0 }
        }
        "options" if args.len() >= 2 => {
            let path = args[0].to_bits() as i64;
            let handler = args[1].to_bits() as i64;
            let result = crate::fastify::js_fastify_options(handle, path, handler);
            if result { 1.0 } else { 0.0 }
        }
        "all" if args.len() >= 2 => {
            let path = args[0].to_bits() as i64;
            let handler = args[1].to_bits() as i64;
            let result = crate::fastify::js_fastify_all(handle, path, handler);
            if result { 1.0 } else { 0.0 }
        }
        "addHook" if args.len() >= 2 => {
            let hook_name = args[0].to_bits() as i64;
            let handler = args[1].to_bits() as i64;
            let result = crate::fastify::js_fastify_add_hook(handle, hook_name, handler);
            if result { 1.0 } else { 0.0 }
        }
        "setErrorHandler" if args.len() >= 1 => {
            let handler = args[0].to_bits() as i64;
            let result = crate::fastify::js_fastify_set_error_handler(handle, handler);
            if result { 1.0 } else { 0.0 }
        }
        "register" if args.len() >= 1 => {
            let plugin = args[0].to_bits() as i64;
            let opts = if args.len() >= 2 { args[1] } else { f64::from_bits(0x7FF8_0000_0000_0001) };
            let result = crate::fastify::js_fastify_register(handle, plugin, opts);
            if result { 1.0 } else { 0.0 }
        }
        "listen" if args.len() >= 1 => {
            let callback = if args.len() >= 2 { args[1].to_bits() as i64 } else { 0 };
            crate::fastify::js_fastify_listen(handle, args[0], callback);
            f64::from_bits(0x7FF8_0000_0000_0001) // undefined (void)
        }
        _ => {
            // Unknown method - return undefined
            f64::from_bits(0x7FF8_0000_0000_0001)
        }
    }
}

/// Dispatch method calls on Fastify context handles (request/reply)
unsafe fn dispatch_fastify_context(handle: i64, method: &str, args: &[f64]) -> f64 {
    use perry_runtime::JSValue;

    match method {
        // Reply methods
        "send" if args.len() >= 1 => {
            let result = crate::fastify::js_fastify_reply_send(handle, args[0]);
            if result { 1.0 } else { 0.0 }
        }
        "status" if args.len() >= 1 => {
            let result = crate::fastify::js_fastify_reply_status(handle, args[0]);
            // Return the handle as NaN-boxed pointer for chaining (reply.status(200).send(...))
            f64::from_bits(0x7FFD_0000_0000_0000 | (result as u64 & 0x0000_FFFF_FFFF_FFFF))
        }
        "header" if args.len() >= 2 => {
            let name = args[0].to_bits() as i64;
            let value = args[1].to_bits() as i64;
            let result = crate::fastify::js_fastify_reply_header(handle, name, value);
            // Return the handle for chaining
            f64::from_bits(0x7FFD_0000_0000_0000 | (result as u64 & 0x0000_FFFF_FFFF_FFFF))
        }
        // Request methods
        "method" => {
            let ptr = crate::fastify::js_fastify_req_method(handle);
            JSValue::string_ptr(ptr).bits() as f64
        }
        "url" => {
            let ptr = crate::fastify::js_fastify_req_url(handle);
            JSValue::string_ptr(ptr).bits() as f64
        }
        "body" => {
            let ptr = crate::fastify::js_fastify_req_body(handle);
            JSValue::string_ptr(ptr).bits() as f64
        }
        "json" => {
            crate::fastify::js_fastify_req_json(handle)
        }
        "params" => {
            let ptr = crate::fastify::js_fastify_req_params(handle);
            JSValue::string_ptr(ptr).bits() as f64
        }
        "headers" => {
            let ptr = crate::fastify::js_fastify_req_headers(handle);
            JSValue::string_ptr(ptr).bits() as f64
        }
        _ => {
            // Unknown method - return undefined
            f64::from_bits(0x7FF8_0000_0000_0001)
        }
    }
}

/// Initialize the handle method dispatch system.
/// This registers our dispatch function with perry-runtime's js_native_call_method.
/// Must be called before any user code runs.
#[no_mangle]
pub unsafe extern "C" fn js_stdlib_init_dispatch() {
    extern "C" {
        fn js_register_handle_method_dispatch(
            f: unsafe extern "C" fn(i64, *const u8, usize, *const f64, usize) -> f64,
        );
    }
    js_register_handle_method_dispatch(js_handle_method_dispatch);
}
