//! FFI interop functions for calling between native code and JavaScript
//!
//! These functions are called from compiled native code to interact with
//! JavaScript modules loaded in the V8 runtime.

use crate::bridge::{native_to_v8, v8_to_native, get_js_handle, store_js_handle, make_js_handle_value, is_js_handle, get_handle_id};
use crate::{ensure_runtime_initialized, get_tokio_runtime, with_runtime, JsRuntimeState, JS_RUNTIME};
use deno_core::v8;
use std::ffi::CStr;
use std::path::PathBuf;

/// Convert a NaN-boxed f64 to a V8 value, returning None if the conversion fails
/// This is specifically for cases where we need to handle the error explicitly
fn nanbox_to_v8<'s>(scope: &mut v8::HandleScope<'s>, value: f64) -> Option<v8::Local<'s, v8::Value>> {
    // Check if it's a JS handle first
    if is_js_handle(value) {
        if let Some(handle_id) = get_handle_id(value) {
            return get_js_handle(scope, handle_id);
        }
        return None;
    }
    // Use the standard conversion for other values
    Some(native_to_v8(scope, value))
}

/// Initialize the JavaScript runtime
/// Must be called once before any other jsruntime functions
#[no_mangle]
pub extern "C" fn js_runtime_init() {
    // Force initialization of the Tokio runtime
    let _ = get_tokio_runtime();
    // Force initialization of the JS runtime on this thread
    ensure_runtime_initialized();

    // Register JS handle functions with perry-runtime so the unified functions can use them
    perry_runtime::js_set_handle_array_get(js_handle_array_get);
    perry_runtime::js_set_handle_array_length(js_handle_array_length);
    perry_runtime::js_set_handle_object_get_property(js_handle_object_get_property);
    perry_runtime::js_set_handle_to_string(js_handle_to_string);
}

/// Shutdown the JavaScript runtime and release resources
#[no_mangle]
pub extern "C" fn js_runtime_shutdown() {
    // The runtime will be cleaned up when the thread exits
    log::debug!("JS runtime shutdown requested");
}

/// Load a JavaScript module and return a handle to it
/// Returns a module handle (u64) that can be used with js_get_export and js_call_function
/// Returns 0 on failure
#[no_mangle]
pub unsafe extern "C" fn js_load_module(
    path_ptr: *const i8,
    path_len: usize,
) -> u64 {
    let path_slice = if path_ptr.is_null() {
        return 0;
    } else if path_len > 0 {
        std::slice::from_raw_parts(path_ptr as *const u8, path_len)
    } else {
        // Null-terminated C string
        CStr::from_ptr(path_ptr).to_bytes()
    };

    let path_str = match std::str::from_utf8(path_slice) {
        Ok(s) => s,
        Err(_) => return 0,
    };

    let path = PathBuf::from(path_str);
    let canonical = std::fs::canonicalize(&path).unwrap_or(path.clone());

    let specifier = match deno_core::ModuleSpecifier::from_file_path(&canonical) {
        Ok(s) => s,
        Err(_) => return 0,
    };

    let tokio_rt = get_tokio_runtime();
    let result = tokio_rt.block_on(async {
        JS_RUNTIME.with(|cell| {
            let mut opt = cell.borrow_mut();
            let state = match opt.as_mut() {
                Some(s) => s,
                None => return Err(()),
            };

            // Check if already loaded
            if let Some(&module_id) = state.loaded_modules.get(&canonical) {
                return Ok(module_id as u64);
            }

            // Use block_in_place to allow async operations
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    // Load the module
                    let module_id = match state.runtime.load_main_es_module(&specifier).await {
                        Ok(id) => id,
                        Err(e) => {
                            log::error!("Failed to load module: {}", e);
                            return Err(());
                        }
                    };

                    // Evaluate the module
                    let result = state.runtime.mod_evaluate(module_id);
                    if let Err(e) = state.runtime.run_event_loop(Default::default()).await {
                        log::error!("Event loop error: {}", e);
                        return Err(());
                    }
                    if let Err(e) = result.await {
                        log::error!("Module evaluation error: {}", e);
                        return Err(());
                    }

                    // Cache the module
                    state.loaded_modules.insert(canonical.clone(), module_id);

                    Ok(module_id as u64)
                })
            })
        })
    });

    result.unwrap_or(0)
}

/// Get an export from a loaded module
/// Returns the value as a NaN-boxed f64
#[no_mangle]
pub unsafe extern "C" fn js_get_export(
    module_handle: u64,
    export_name_ptr: *const i8,
    export_name_len: usize,
) -> f64 {
    let name_slice = if export_name_ptr.is_null() {
        return f64::from_bits(0x7FFC_0000_0000_0001); // undefined
    } else if export_name_len > 0 {
        std::slice::from_raw_parts(export_name_ptr as *const u8, export_name_len)
    } else {
        CStr::from_ptr(export_name_ptr).to_bytes()
    };

    let export_name = match std::str::from_utf8(name_slice) {
        Ok(s) => s,
        Err(_) => return f64::from_bits(0x7FFC_0000_0000_0001),
    };

    with_runtime(|state| {
        let module_id = module_handle as deno_core::ModuleId;
        let namespace = match state.runtime.get_module_namespace(module_id) {
            Ok(ns) => ns,
            Err(e) => {
                log::error!("Failed to get module namespace: {}", e);
                return f64::from_bits(0x7FFC_0000_0000_0001);
            }
        };

        let scope = &mut state.runtime.handle_scope();
        let namespace = v8::Local::new(scope, namespace);

        let key = match v8::String::new(scope, export_name) {
            Some(k) => k,
            None => return f64::from_bits(0x7FFC_0000_0000_0001),
        };

        let value = match namespace.get(scope, key.into()) {
            Some(v) => v,
            None => return f64::from_bits(0x7FFC_0000_0000_0001),
        };

        v8_to_native(scope, value)
    })
}

/// Call a JavaScript function with arguments
/// Returns the result as a NaN-boxed f64
#[no_mangle]
pub unsafe extern "C" fn js_call_function(
    module_handle: u64,
    func_name_ptr: *const i8,
    func_name_len: usize,
    args_ptr: *const f64,
    args_len: usize,
) -> f64 {
    let name_slice = if func_name_ptr.is_null() {
        return f64::from_bits(0x7FFC_0000_0000_0001); // undefined
    } else if func_name_len > 0 {
        std::slice::from_raw_parts(func_name_ptr as *const u8, func_name_len)
    } else {
        CStr::from_ptr(func_name_ptr).to_bytes()
    };

    let func_name = match std::str::from_utf8(name_slice) {
        Ok(s) => s,
        Err(_) => return f64::from_bits(0x7FFC_0000_0000_0001),
    };

    let args = if args_ptr.is_null() || args_len == 0 {
        Vec::new()
    } else {
        std::slice::from_raw_parts(args_ptr, args_len).to_vec()
    };

    with_runtime(|state| {
        let module_id = module_handle as deno_core::ModuleId;
        let namespace = match state.runtime.get_module_namespace(module_id) {
            Ok(ns) => ns,
            Err(e) => {
                log::error!("Failed to get module namespace: {}", e);
                return f64::from_bits(0x7FFC_0000_0000_0001);
            }
        };

        call_function_impl(state, namespace, func_name, &args)
    })
}

fn call_function_impl(
    state: &mut JsRuntimeState,
    namespace: v8::Global<v8::Object>,
    func_name: &str,
    args: &[f64],
) -> f64 {
    let scope = &mut state.runtime.handle_scope();
    let namespace = v8::Local::new(scope, namespace);

    // Get the function from the namespace
    let key = match v8::String::new(scope, func_name) {
        Some(k) => k,
        None => return f64::from_bits(0x7FFC_0000_0000_0001),
    };

    let func_val = match namespace.get(scope, key.into()) {
        Some(v) => v,
        None => {
            log::error!("Function '{}' not found in module", func_name);
            return f64::from_bits(0x7FFC_0000_0000_0001);
        }
    };

    if !func_val.is_function() {
        log::error!("'{}' is not a function", func_name);
        return f64::from_bits(0x7FFC_0000_0000_0001);
    }

    let func = v8::Local::<v8::Function>::try_from(func_val).unwrap();

    // Convert arguments from native to V8
    let v8_args: Vec<v8::Local<v8::Value>> = args
        .iter()
        .map(|&arg| native_to_v8(scope, arg))
        .collect();

    // Call the function
    let undefined = v8::undefined(scope);
    let result = match func.call(scope, undefined.into(), &v8_args) {
        Some(r) => r,
        None => {
            log::error!("Function call failed");
            return f64::from_bits(0x7FFC_0000_0000_0001);
        }
    };

    // Handle promises - for now just return the promise object
    // Proper async support would require more complex handling
    v8_to_native(scope, result)
}

/// Call a method on a JavaScript object
#[no_mangle]
pub unsafe extern "C" fn js_call_method(
    object_ptr: f64,
    method_name_ptr: *const i8,
    method_name_len: usize,
    args_ptr: *const f64,
    args_len: usize,
) -> f64 {
    let name_slice = if method_name_ptr.is_null() {
        return f64::from_bits(0x7FFC_0000_0000_0001);
    } else if method_name_len > 0 {
        std::slice::from_raw_parts(method_name_ptr as *const u8, method_name_len)
    } else {
        CStr::from_ptr(method_name_ptr).to_bytes()
    };

    let method_name = match std::str::from_utf8(name_slice) {
        Ok(s) => s,
        Err(_) => return f64::from_bits(0x7FFC_0000_0000_0001),
    };

    let args = if args_ptr.is_null() || args_len == 0 {
        Vec::new()
    } else {
        std::slice::from_raw_parts(args_ptr, args_len).to_vec()
    };

    with_runtime(|state| {
        let scope = &mut state.runtime.handle_scope();

        // Convert the object pointer to a V8 object
        let obj_val = native_to_v8(scope, object_ptr);
        if !obj_val.is_object() {
            log::error!("Value is not an object");
            return f64::from_bits(0x7FFC_0000_0000_0001);
        }

        let obj = obj_val.to_object(scope).unwrap();

        // Get the method
        let key = match v8::String::new(scope, method_name) {
            Some(k) => k,
            None => return f64::from_bits(0x7FFC_0000_0000_0001),
        };

        let method_val = match obj.get(scope, key.into()) {
            Some(v) => v,
            None => {
                log::error!("Method '{}' not found on object", method_name);
                return f64::from_bits(0x7FFC_0000_0000_0001);
            }
        };

        if !method_val.is_function() {
            log::error!("'{}' is not a function", method_name);
            return f64::from_bits(0x7FFC_0000_0000_0001);
        }

        let method = v8::Local::<v8::Function>::try_from(method_val).unwrap();

        // Convert arguments
        let v8_args: Vec<v8::Local<v8::Value>> = args
            .iter()
            .map(|&arg| native_to_v8(scope, arg))
            .collect();

        // Call with 'this' bound to the object
        let result = match method.call(scope, obj.into(), &v8_args) {
            Some(r) => r,
            None => {
                log::error!("Method call failed");
                return f64::from_bits(0x7FFC_0000_0000_0001);
            }
        };

        v8_to_native(scope, result)
    })
}

/// Call a JavaScript function value directly (for callback parameters)
/// func_value: NaN-boxed f64 containing a V8 function handle
/// args_ptr: pointer to array of f64 arguments
/// args_len: number of arguments
/// Returns the result as a NaN-boxed f64
#[no_mangle]
pub unsafe extern "C" fn js_call_value(
    func_value: f64,
    args_ptr: *const f64,
    args_len: usize,
) -> f64 {
    let args = if args_ptr.is_null() || args_len == 0 {
        Vec::new()
    } else {
        std::slice::from_raw_parts(args_ptr, args_len).to_vec()
    };

    with_runtime(|state| {
        let scope = &mut state.runtime.handle_scope();

        // Extract the function from the NaN-boxed value
        let func_local = match nanbox_to_v8(scope, func_value) {
            Some(v) => v,
            None => {
                log::error!("Failed to convert function value from NaN-boxed");
                return f64::from_bits(0x7FFC_0000_0000_0001); // undefined
            }
        };

        if !func_local.is_function() {
            log::error!("Value is not a function");
            return f64::from_bits(0x7FFC_0000_0000_0001);
        }

        let func = v8::Local::<v8::Function>::try_from(func_local).unwrap();

        // Convert arguments
        let v8_args: Vec<v8::Local<v8::Value>> = args
            .iter()
            .map(|&arg| native_to_v8(scope, arg))
            .collect();

        // Call with undefined as 'this'
        let undefined = v8::undefined(scope);
        let result = match func.call(scope, undefined.into(), &v8_args) {
            Some(r) => r,
            None => {
                log::error!("Function call failed");
                return f64::from_bits(0x7FFC_0000_0000_0001);
            }
        };

        v8_to_native(scope, result)
    })
}

/// Register a native function that can be called from JavaScript
#[no_mangle]
pub unsafe extern "C" fn js_register_native_function(
    name_ptr: *const i8,
    name_len: usize,
    func_ptr: *const u8,
    param_count: usize,
) {
    let name_slice = if name_ptr.is_null() {
        return;
    } else if name_len > 0 {
        std::slice::from_raw_parts(name_ptr as *const u8, name_len)
    } else {
        CStr::from_ptr(name_ptr).to_bytes()
    };

    let _func_name = match std::str::from_utf8(name_slice) {
        Ok(s) => s.to_string(),
        Err(_) => return,
    };

    // Store the function pointer and param count for later use
    log::debug!(
        "Registered native function at {:?} with {} params",
        func_ptr,
        param_count
    );

    // TODO: Implement proper native function registration
}

/// Get an element from a JavaScript array by index
/// array_handle: NaN-boxed value containing a JS handle to an array
/// index: The array index
/// Returns the element value as a NaN-boxed f64
#[no_mangle]
pub extern "C" fn js_handle_array_get(array_handle: f64, index: i32) -> f64 {
    with_runtime(|state| {
        let scope = &mut state.runtime.handle_scope();

        // Convert the handle to a V8 value
        let arr_val = native_to_v8(scope, array_handle);
        if !arr_val.is_array() {
            log::error!("Value is not an array");
            return f64::from_bits(0x7FFC_0000_0000_0001); // undefined
        }

        let arr = v8::Local::<v8::Array>::try_from(arr_val).unwrap();

        // Get the element
        let elem = match arr.get_index(scope, index as u32) {
            Some(v) => v,
            None => return f64::from_bits(0x7FFC_0000_0000_0001),
        };

        v8_to_native(scope, elem)
    })
}

/// Get the length of a JavaScript array
/// array_handle: NaN-boxed value containing a JS handle to an array
/// Returns the length as i32
#[no_mangle]
pub extern "C" fn js_handle_array_length(array_handle: f64) -> i32 {
    with_runtime(|state| {
        let scope = &mut state.runtime.handle_scope();

        // Convert the handle to a V8 value
        let arr_val = native_to_v8(scope, array_handle);
        if !arr_val.is_array() {
            log::error!("Value is not an array");
            return 0;
        }

        let arr = v8::Local::<v8::Array>::try_from(arr_val).unwrap();
        arr.length() as i32
    })
}

/// Get a property from a JavaScript object (for JS handle objects)
/// This is called by js_dynamic_object_get_property in perry-runtime when a JS handle is detected
/// object_ptr: NaN-boxed value containing a JS handle
/// Returns the property value as a NaN-boxed f64
#[no_mangle]
pub extern "C" fn js_handle_object_get_property(
    object_ptr: f64,
    property_name_ptr: *const i8,
    property_name_len: usize,
) -> f64 {
    let name_slice = if property_name_ptr.is_null() {
        return f64::from_bits(0x7FFC_0000_0000_0001); // undefined
    } else if property_name_len > 0 {
        unsafe { std::slice::from_raw_parts(property_name_ptr as *const u8, property_name_len) }
    } else {
        unsafe { CStr::from_ptr(property_name_ptr).to_bytes() }
    };

    let property_name = match std::str::from_utf8(name_slice) {
        Ok(s) => s,
        Err(_) => return f64::from_bits(0x7FFC_0000_0000_0001),
    };

    with_runtime(|state| {
        let scope = &mut state.runtime.handle_scope();

        // Convert the object pointer to a V8 object
        let obj_val = native_to_v8(scope, object_ptr);
        if !obj_val.is_object() {
            log::error!("Value is not an object");
            return f64::from_bits(0x7FFC_0000_0000_0001);
        }

        let obj = obj_val.to_object(scope).unwrap();

        // Get the property
        let key = match v8::String::new(scope, property_name) {
            Some(k) => k,
            None => return f64::from_bits(0x7FFC_0000_0000_0001),
        };

        let prop_val = match obj.get(scope, key.into()) {
            Some(v) => v,
            None => return f64::from_bits(0x7FFC_0000_0000_0001),
        };

        v8_to_native(scope, prop_val)
    })
}

/// Convert a JavaScript handle value to a native string
/// handle: NaN-boxed value containing a JS handle
/// Returns a pointer to a native StringHeader
#[no_mangle]
pub extern "C" fn js_handle_to_string(handle: f64) -> *mut perry_runtime::string::StringHeader {
    with_runtime(|state| {
        let scope = &mut state.runtime.handle_scope();

        // Convert the handle to a V8 value
        let v8_val = native_to_v8(scope, handle);

        // Convert to string
        let str_val = match v8_val.to_string(scope) {
            Some(s) => s,
            None => {
                // Return empty string on failure
                return perry_runtime::string::js_string_from_bytes(b"".as_ptr(), 0);
            }
        };

        // Get the UTF-8 bytes
        let len = str_val.utf8_length(scope);
        let mut buffer = vec![0u8; len];
        str_val.write_utf8(scope, &mut buffer, None, v8::WriteOptions::NO_NULL_TERMINATION);

        // Create a native string
        perry_runtime::string::js_string_from_bytes(buffer.as_ptr(), buffer.len() as u32)
    })
}

/// Set a property on a JavaScript object
/// object_ptr: NaN-boxed value containing a JS handle
/// value: NaN-boxed value to set
#[no_mangle]
pub unsafe extern "C" fn js_set_property(
    object_ptr: f64,
    property_name_ptr: *const i8,
    property_name_len: usize,
    value: f64,
) {
    let name_slice = if property_name_ptr.is_null() {
        return;
    } else if property_name_len > 0 {
        std::slice::from_raw_parts(property_name_ptr as *const u8, property_name_len)
    } else {
        CStr::from_ptr(property_name_ptr).to_bytes()
    };

    let property_name = match std::str::from_utf8(name_slice) {
        Ok(s) => s,
        Err(_) => return,
    };

    with_runtime(|state| {
        let scope = &mut state.runtime.handle_scope();

        // Convert the object pointer to a V8 object
        let obj_val = native_to_v8(scope, object_ptr);
        if !obj_val.is_object() {
            log::error!("Value is not an object");
            return;
        }

        let obj = obj_val.to_object(scope).unwrap();

        // Set the property
        let key = match v8::String::new(scope, property_name) {
            Some(k) => k,
            None => return,
        };

        let v8_value = native_to_v8(scope, value);
        obj.set(scope, key.into(), v8_value);
    })
}

/// Create a new instance of a JavaScript class
/// module_handle: Handle to the loaded module
/// class_name: Name of the class to instantiate
/// args: Array of NaN-boxed f64 arguments
/// Returns a JS handle to the new instance
#[no_mangle]
pub unsafe extern "C" fn js_new_instance(
    module_handle: u64,
    class_name_ptr: *const i8,
    class_name_len: usize,
    args_ptr: *const f64,
    args_len: usize,
) -> f64 {
    let name_slice = if class_name_ptr.is_null() {
        return f64::from_bits(0x7FFC_0000_0000_0001); // undefined
    } else if class_name_len > 0 {
        std::slice::from_raw_parts(class_name_ptr as *const u8, class_name_len)
    } else {
        CStr::from_ptr(class_name_ptr).to_bytes()
    };

    let class_name = match std::str::from_utf8(name_slice) {
        Ok(s) => s,
        Err(_) => return f64::from_bits(0x7FFC_0000_0000_0001),
    };

    let args = if args_ptr.is_null() || args_len == 0 {
        Vec::new()
    } else {
        std::slice::from_raw_parts(args_ptr, args_len).to_vec()
    };

    with_runtime(|state| {
        let module_id = module_handle as deno_core::ModuleId;
        let namespace = match state.runtime.get_module_namespace(module_id) {
            Ok(ns) => ns,
            Err(e) => {
                log::error!("Failed to get module namespace: {}", e);
                return f64::from_bits(0x7FFC_0000_0000_0001);
            }
        };

        let scope = &mut state.runtime.handle_scope();
        let namespace = v8::Local::new(scope, namespace);

        // Get the class constructor from the namespace
        let key = match v8::String::new(scope, class_name) {
            Some(k) => k,
            None => return f64::from_bits(0x7FFC_0000_0000_0001),
        };

        let constructor_val = match namespace.get(scope, key.into()) {
            Some(v) => v,
            None => {
                log::error!("Class '{}' not found in module", class_name);
                return f64::from_bits(0x7FFC_0000_0000_0001);
            }
        };

        if !constructor_val.is_function() {
            log::error!("'{}' is not a constructor", class_name);
            return f64::from_bits(0x7FFC_0000_0000_0001);
        }

        let constructor = v8::Local::<v8::Function>::try_from(constructor_val).unwrap();

        // Convert arguments from native to V8
        let v8_args: Vec<v8::Local<v8::Value>> = args
            .iter()
            .map(|&arg| native_to_v8(scope, arg))
            .collect();

        // Call the constructor with 'new'
        let result = match constructor.new_instance(scope, &v8_args) {
            Some(r) => r,
            None => {
                log::error!("Constructor call failed");
                return f64::from_bits(0x7FFC_0000_0000_0001);
            }
        };

        v8_to_native(scope, result.into())
    })
}

/// Create a new instance using a JS handle to a constructor function
/// constructor_handle: NaN-boxed value containing a JS handle to a constructor
/// args: Array of NaN-boxed f64 arguments
/// Returns a JS handle to the new instance
#[no_mangle]
pub unsafe extern "C" fn js_new_from_handle(
    constructor_handle: f64,
    args_ptr: *const f64,
    args_len: usize,
) -> f64 {
    let args = if args_ptr.is_null() || args_len == 0 {
        Vec::new()
    } else {
        std::slice::from_raw_parts(args_ptr, args_len).to_vec()
    };

    with_runtime(|state| {
        let scope = &mut state.runtime.handle_scope();

        // Get the constructor from the handle
        let constructor_val = native_to_v8(scope, constructor_handle);
        if !constructor_val.is_function() {
            log::error!("Value is not a constructor");
            return f64::from_bits(0x7FFC_0000_0000_0001);
        }

        let constructor = v8::Local::<v8::Function>::try_from(constructor_val).unwrap();

        // Convert arguments from native to V8
        let v8_args: Vec<v8::Local<v8::Value>> = args
            .iter()
            .map(|&arg| native_to_v8(scope, arg))
            .collect();

        // Call the constructor with 'new'
        let result = match constructor.new_instance(scope, &v8_args) {
            Some(r) => r,
            None => {
                log::error!("Constructor call failed");
                return f64::from_bits(0x7FFC_0000_0000_0001);
            }
        };

        v8_to_native(scope, result.into())
    })
}

// Storage for native callback function pointers and their closure environments
thread_local! {
    static NATIVE_CALLBACKS: std::cell::RefCell<std::collections::HashMap<u64, (i64, i64)>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
    static NEXT_CALLBACK_ID: std::cell::Cell<u64> = const { std::cell::Cell::new(1) };
}

/// Create a V8 function that wraps a native callback
/// func_ptr: Pointer to the native function to call
/// closure_env: Pointer to the closure environment (or 0 for no environment)
/// param_count: Number of parameters the callback expects
/// Returns a JS handle to the V8 function
#[no_mangle]
pub unsafe extern "C" fn js_create_callback(
    func_ptr: i64,
    closure_env: i64,
    param_count: i64,
) -> f64 {
    // Store the callback info
    let callback_id = NEXT_CALLBACK_ID.with(|id| {
        let current = id.get();
        id.set(current + 1);
        current
    });

    NATIVE_CALLBACKS.with(|callbacks| {
        callbacks.borrow_mut().insert(callback_id, (func_ptr, closure_env));
    });

    with_runtime(|state| {
        let scope = &mut state.runtime.handle_scope();

        // Create external data to store the callback ID and param count
        let data_array = v8::Array::new(scope, 2);
        let id_val = v8::Number::new(scope, callback_id as f64);
        let count_val = v8::Number::new(scope, param_count as f64);
        data_array.set_index(scope, 0, id_val.into());
        data_array.set_index(scope, 1, count_val.into());

        // Create the callback function
        let callback_fn = v8::Function::builder(native_callback_trampoline)
            .data(data_array.into())
            .build(scope);

        match callback_fn {
            Some(func) => {
                let handle_id = store_js_handle(scope, func.into());
                make_js_handle_value(handle_id)
            }
            None => {
                log::error!("Failed to create callback function");
                f64::from_bits(0x7FFC_0000_0000_0001)
            }
        }
    })
}

/// Trampoline function that V8 calls when a native callback is invoked
fn native_callback_trampoline(
    scope: &mut v8::HandleScope,
    args: v8::FunctionCallbackArguments,
    mut retval: v8::ReturnValue,
) {
    // Get the callback ID and param count from the data
    let data = args.data();
    if !data.is_array() {
        retval.set(v8::undefined(scope).into());
        return;
    }

    let data_array = v8::Local::<v8::Array>::try_from(data).unwrap();
    let callback_id = data_array.get_index(scope, 0)
        .and_then(|v| v.number_value(scope))
        .unwrap_or(0.0) as u64;
    let _param_count = data_array.get_index(scope, 1)
        .and_then(|v| v.number_value(scope))
        .unwrap_or(0.0) as i64;

    // Get the function pointer and closure environment
    let (func_ptr, closure_env) = NATIVE_CALLBACKS.with(|callbacks| {
        callbacks.borrow().get(&callback_id).copied().unwrap_or((0, 0))
    });

    if func_ptr == 0 {
        log::error!("Native callback not found: {}", callback_id);
        retval.set(v8::undefined(scope).into());
        return;
    }

    // Convert arguments to native format
    let arg_count = args.length();
    let mut native_args: Vec<f64> = Vec::with_capacity(arg_count as usize);
    for i in 0..arg_count {
        let arg = args.get(i);
        native_args.push(v8_to_native(scope, arg));
    }

    // Call the native function
    // Function signature: fn(closure_env: i64, args_ptr: *const f64, args_len: i64) -> f64
    type CallbackFn = extern "C" fn(i64, *const f64, i64) -> f64;
    let callback: CallbackFn = unsafe { std::mem::transmute(func_ptr as *const ()) };
    let result = callback(closure_env, native_args.as_ptr(), native_args.len() as i64);

    // Convert result back to V8
    let v8_result = native_to_v8(scope, result);
    retval.set(v8_result);
}

/// Check if a module path should be loaded via the JS runtime
/// Returns 1 if it should use JS runtime, 0 if it should be compiled natively
#[no_mangle]
pub unsafe extern "C" fn js_should_use_runtime(
    path_ptr: *const i8,
    path_len: usize,
) -> i32 {
    let path_slice = if path_ptr.is_null() {
        return 0;
    } else if path_len > 0 {
        std::slice::from_raw_parts(path_ptr as *const u8, path_len)
    } else {
        CStr::from_ptr(path_ptr).to_bytes()
    };

    let path_str = match std::str::from_utf8(path_slice) {
        Ok(s) => s,
        Err(_) => return 0,
    };

    // Check if this is a .js file (not .ts/.tsx)
    if path_str.ends_with(".js") || path_str.ends_with(".mjs") || path_str.ends_with(".cjs") {
        return 1;
    }

    // Check if this is in node_modules and not TypeScript
    if path_str.contains("node_modules") {
        let path = PathBuf::from(path_str);

        // If it's a directory reference, check for TypeScript files
        if path.is_dir() {
            let has_ts = path.join("index.ts").exists()
                || path.join("index.tsx").exists()
                || path.join("src/index.ts").exists();

            if !has_ts {
                return 1;
            }
        }
    }

    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runtime_init() {
        js_runtime_init();
        // Should not panic
    }
}
