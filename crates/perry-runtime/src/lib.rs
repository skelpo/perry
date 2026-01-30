//! Runtime Library for Perry
//!
//! Provides the runtime support needed by compiled TypeScript programs:
//! - JSValue representation (NaN-boxing)
//! - Object representation and allocation
//! - Array representation and operations
//! - Garbage collection integration
//! - Built-in object implementations
//! - Console and other global functions

pub mod value;
pub mod arena;
pub mod object;
pub mod array;
pub mod map;
pub mod set;
pub mod string;
pub mod bigint;
pub mod closure;
pub mod exception;
pub mod error;
pub mod promise;
pub mod timer;
pub mod builtins;
pub mod r#box;
pub mod process;
pub mod fs;
pub mod path;
pub mod math;
pub mod date;
pub mod url;
pub mod regex;
pub mod os;
pub mod buffer;
pub mod child_process;
pub mod net;
pub mod redis_client;

pub use value::JSValue;
pub use promise::Promise;
pub use object::ObjectHeader;
pub use array::ArrayHeader;
pub use map::MapHeader;
pub use set::SetHeader;
pub use string::StringHeader;
pub use bigint::BigIntHeader;
pub use closure::ClosureHeader;
pub use regex::RegExpHeader;
pub use buffer::BufferHeader;

// Re-export closure module for stdlib to use js_closure_call* functions
pub use closure::{js_closure_call0, js_closure_call1, js_closure_call2, js_closure_call3};

// Re-export commonly used FFI functions for stdlib
pub use array::{js_array_alloc, js_array_set, js_array_get, js_array_push, js_array_length, js_array_is_array, js_array_get_jsvalue};
pub use object::{js_object_alloc, js_object_set_field, js_object_get_field, js_object_set_keys, js_object_keys, js_object_values, js_object_entries, js_object_get_field_by_name, js_object_get_field_by_name_f64};
pub use string::js_string_from_bytes;
pub use promise::{js_promise_new, js_promise_resolve, js_promise_reject};
pub use bigint::js_bigint_from_string;
pub use value::js_nanbox_get_pointer;
pub use value::{js_set_handle_array_get, js_set_handle_array_length, js_set_handle_object_get_property};
