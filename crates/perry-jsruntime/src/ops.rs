//! Custom Deno ops for Perry runtime integration
//!
//! These ops allow JavaScript code to call back into native Perry code.

use deno_core::{extension, op2};

#[op2]
#[string]
fn op_perry_log(#[string] message: String) -> String {
    log::info!("[JS] {}", message);
    message
}

#[op2]
#[serde]
fn op_perry_call_native(
    #[string] func_name: String,
    #[serde] args: Vec<serde_json::Value>,
) -> serde_json::Value {
    log::debug!("Native call: {} with {} args", func_name, args.len());
    // TODO: Look up function in registry and call it
    serde_json::Value::Null
}

extension!(
    perry_ops,
    ops = [
        op_perry_log,
        op_perry_call_native,
    ],
);
