//! V8 JavaScript Runtime for Perry
//!
//! This crate provides V8 JavaScript runtime support for running npm modules
//! that cannot be natively compiled. It serves as a fallback when:
//! - A module is pure JavaScript (not TypeScript)
//! - A module uses dynamic features incompatible with AOT compilation
//!
//! The runtime is opt-in and requires explicit configuration.

mod bridge;
mod interop;
mod modules;
mod ops;

pub use bridge::{native_to_v8, v8_to_native, store_js_handle, get_js_handle, release_js_handle,
    is_js_handle, get_handle_id, make_js_handle_value};
pub use interop::{
    js_call_function, js_call_method, js_get_export, js_load_module, js_register_native_function,
    js_runtime_init, js_runtime_shutdown, js_get_property, js_set_property,
    js_new_instance, js_new_from_handle, js_create_callback,
};
// Re-export deno_core's ModuleLoader trait for external use
pub use deno_core::ModuleLoader;

use deno_core::{JsRuntime, RuntimeOptions};
use once_cell::sync::OnceCell;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::runtime::Runtime as TokioRuntime;

/// Global Tokio runtime for async operations
static TOKIO_RUNTIME: OnceCell<TokioRuntime> = OnceCell::new();

thread_local! {
    /// Thread-local V8 runtime instance
    /// JsRuntime is not Send, so it must be thread-local
    static JS_RUNTIME: RefCell<Option<JsRuntimeState>> = const { RefCell::new(None) };
}

/// State for the JS runtime
pub struct JsRuntimeState {
    pub runtime: JsRuntime,
    /// Map of loaded module paths to their V8 module IDs
    pub loaded_modules: HashMap<PathBuf, deno_core::ModuleId>,
    /// Whether the runtime has been initialized
    pub initialized: bool,
}

impl JsRuntimeState {
    fn new() -> Self {
        let runtime = JsRuntime::new(RuntimeOptions {
            module_loader: Some(std::rc::Rc::new(modules::NodeModuleLoader::new())),
            extensions: vec![ops::perry_ops::init_ops()],
            ..Default::default()
        });

        Self {
            runtime,
            loaded_modules: HashMap::new(),
            initialized: true,
        }
    }
}

/// Initialize the Tokio runtime for async operations
pub fn get_tokio_runtime() -> &'static TokioRuntime {
    TOKIO_RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Failed to create Tokio runtime")
    })
}

/// Initialize the JS runtime for the current thread
pub fn ensure_runtime_initialized() {
    JS_RUNTIME.with(|cell| {
        let mut opt = cell.borrow_mut();
        if opt.is_none() {
            *opt = Some(JsRuntimeState::new());
        }
    });
}

/// Execute a closure with the JS runtime
pub fn with_runtime<F, R>(f: F) -> R
where
    F: FnOnce(&mut JsRuntimeState) -> R,
{
    ensure_runtime_initialized();
    JS_RUNTIME.with(|cell| {
        let mut opt = cell.borrow_mut();
        let state = opt.as_mut().expect("Runtime should be initialized");
        f(state)
    })
}

/// Execute an async closure with the JS runtime
pub fn with_runtime_async<F, Fut, R>(f: F) -> R
where
    F: FnOnce(&mut JsRuntimeState) -> Fut,
    Fut: std::future::Future<Output = R>,
{
    let tokio_rt = get_tokio_runtime();
    tokio_rt.block_on(async {
        ensure_runtime_initialized();
        JS_RUNTIME.with(|cell| {
            let mut opt = cell.borrow_mut();
            let state = opt.as_mut().expect("Runtime should be initialized");
            // Note: This is not truly async-safe, but works for simple cases
            // For proper async support, we'd need a different architecture
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(f(state))
            })
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runtime_init() {
        js_runtime_init();
        // Should not panic on double init
        js_runtime_init();
    }
}
