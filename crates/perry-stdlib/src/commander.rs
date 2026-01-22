//! Commander implementation
//!
//! Native implementation of the commander npm package for CLI parsing.
//! Provides a fluent API for building command-line interfaces.

use perry_runtime::{js_string_from_bytes, StringHeader};
use std::collections::HashMap;

use crate::common::{get_handle_mut, register_handle, Handle};

/// CommanderHandle stores the command configuration and parsed values
pub struct CommanderHandle {
    name: String,
    description: String,
    version: String,
    options: Vec<CommandOption>,
    parsed_values: HashMap<String, String>,
    args: Vec<String>,
}

struct CommandOption {
    short: Option<char>,
    long: String,
    description: String,
    default_value: Option<String>,
    is_flag: bool, // true for boolean flags, false for value options
}

impl CommanderHandle {
    pub fn new() -> Self {
        CommanderHandle {
            name: String::new(),
            description: String::new(),
            version: String::new(),
            options: Vec::new(),
            parsed_values: HashMap::new(),
            args: Vec::new(),
        }
    }
}

/// Helper to extract string from StringHeader pointer
unsafe fn string_from_header(ptr: *const StringHeader) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    let len = (*ptr).length as usize;
    let data_ptr = (ptr as *const u8).add(std::mem::size_of::<StringHeader>());
    let bytes = std::slice::from_raw_parts(data_ptr, len);
    Some(String::from_utf8_lossy(bytes).to_string())
}

/// Create a new Command
#[no_mangle]
pub extern "C" fn js_commander_new() -> Handle {
    register_handle(CommanderHandle::new())
}

/// Command.name(name)
#[no_mangle]
pub unsafe extern "C" fn js_commander_name(handle: Handle, name_ptr: *const StringHeader) -> Handle {
    if let Some(name) = string_from_header(name_ptr) {
        if let Some(cmd) = get_handle_mut::<CommanderHandle>(handle) {
            cmd.name = name;
        }
    }
    handle
}

/// Command.description(desc)
#[no_mangle]
pub unsafe extern "C" fn js_commander_description(handle: Handle, desc_ptr: *const StringHeader) -> Handle {
    if let Some(desc) = string_from_header(desc_ptr) {
        if let Some(cmd) = get_handle_mut::<CommanderHandle>(handle) {
            cmd.description = desc;
        }
    }
    handle
}

/// Command.version(version)
#[no_mangle]
pub unsafe extern "C" fn js_commander_version(handle: Handle, version_ptr: *const StringHeader) -> Handle {
    if let Some(version) = string_from_header(version_ptr) {
        if let Some(cmd) = get_handle_mut::<CommanderHandle>(handle) {
            cmd.version = version;
        }
    }
    handle
}

/// Command.option(flags, description, defaultValue?)
/// flags format: "-s, --long <value>" or "-f, --flag" for boolean
#[no_mangle]
pub unsafe extern "C" fn js_commander_option(
    handle: Handle,
    flags_ptr: *const StringHeader,
    desc_ptr: *const StringHeader,
    default_ptr: *const StringHeader,
) -> Handle {
    let flags = match string_from_header(flags_ptr) {
        Some(f) => f,
        None => return handle,
    };
    let description = string_from_header(desc_ptr).unwrap_or_default();
    let default_value = string_from_header(default_ptr);

    // Parse flags: "-s, --long" or "-s, --long <value>"
    let is_flag = !flags.contains('<') && !flags.contains('[');

    let mut short: Option<char> = None;
    let mut long = String::new();

    for part in flags.split(',') {
        let part = part.trim();
        if part.starts_with("--") {
            // Long option
            long = part.trim_start_matches("--")
                .split_whitespace()
                .next()
                .unwrap_or("")
                .to_string();
        } else if part.starts_with('-') {
            // Short option
            short = part.chars().nth(1);
        }
    }

    if let Some(cmd) = get_handle_mut::<CommanderHandle>(handle) {
        cmd.options.push(CommandOption {
            short,
            long,
            description,
            default_value,
            is_flag,
        });
    }

    handle
}

/// Command.parse()
/// Parse command line arguments (from std::env::args)
#[no_mangle]
pub extern "C" fn js_commander_parse(handle: Handle) -> Handle {
    let args: Vec<String> = std::env::args().collect();

    if let Some(cmd) = get_handle_mut::<CommanderHandle>(handle) {
        // Initialize with default values
        for opt in &cmd.options {
            if let Some(ref default) = opt.default_value {
                cmd.parsed_values.insert(opt.long.clone(), default.clone());
            } else if opt.is_flag {
                cmd.parsed_values.insert(opt.long.clone(), "false".to_string());
            }
        }

        // Parse arguments
        let mut i = 1; // Skip program name
        let mut positional_args = Vec::new();

        while i < args.len() {
            let arg = &args[i];

            if arg.starts_with("--") {
                // Long option
                let opt_name = arg.trim_start_matches("--");

                // Check for --option=value format
                if let Some(eq_pos) = opt_name.find('=') {
                    let name = &opt_name[..eq_pos];
                    let value = &opt_name[eq_pos + 1..];
                    cmd.parsed_values.insert(name.to_string(), value.to_string());
                } else if let Some(opt) = cmd.options.iter().find(|o| o.long == opt_name) {
                    if opt.is_flag {
                        cmd.parsed_values.insert(opt_name.to_string(), "true".to_string());
                    } else if i + 1 < args.len() {
                        i += 1;
                        cmd.parsed_values.insert(opt_name.to_string(), args[i].clone());
                    }
                } else {
                    // Unknown option, treat as flag
                    cmd.parsed_values.insert(opt_name.to_string(), "true".to_string());
                }
            } else if arg.starts_with('-') && arg.len() == 2 {
                // Short option
                let short_char = arg.chars().nth(1).unwrap();

                if let Some(opt) = cmd.options.iter().find(|o| o.short == Some(short_char)) {
                    if opt.is_flag {
                        cmd.parsed_values.insert(opt.long.clone(), "true".to_string());
                    } else if i + 1 < args.len() {
                        i += 1;
                        cmd.parsed_values.insert(opt.long.clone(), args[i].clone());
                    }
                }
            } else {
                // Positional argument
                positional_args.push(arg.clone());
            }

            i += 1;
        }

        cmd.args = positional_args;
    }

    handle
}

/// Command.opts()
/// Returns the parsed options as a handle (for now, access via getOption)
#[no_mangle]
pub extern "C" fn js_commander_opts(handle: Handle) -> Handle {
    handle // Return self, options are accessed via getOption
}

/// Get a specific option value as string
#[no_mangle]
pub unsafe extern "C" fn js_commander_get_option(handle: Handle, name_ptr: *const StringHeader) -> *const StringHeader {
    let name = match string_from_header(name_ptr) {
        Some(n) => n,
        None => return std::ptr::null(),
    };

    if let Some(cmd) = get_handle_mut::<CommanderHandle>(handle) {
        if let Some(value) = cmd.parsed_values.get(&name) {
            return js_string_from_bytes(value.as_ptr(), value.len() as u32);
        }
    }

    std::ptr::null()
}

/// Get a specific option value as number
#[no_mangle]
pub unsafe extern "C" fn js_commander_get_option_number(handle: Handle, name_ptr: *const StringHeader) -> f64 {
    let name = match string_from_header(name_ptr) {
        Some(n) => n,
        None => return f64::NAN,
    };

    if let Some(cmd) = get_handle_mut::<CommanderHandle>(handle) {
        if let Some(value) = cmd.parsed_values.get(&name) {
            return value.parse::<f64>().unwrap_or(f64::NAN);
        }
    }

    f64::NAN
}

/// Get a specific option value as boolean
#[no_mangle]
pub unsafe extern "C" fn js_commander_get_option_bool(handle: Handle, name_ptr: *const StringHeader) -> bool {
    let name = match string_from_header(name_ptr) {
        Some(n) => n,
        None => return false,
    };

    if let Some(cmd) = get_handle_mut::<CommanderHandle>(handle) {
        if let Some(value) = cmd.parsed_values.get(&name) {
            return value == "true" || value == "1";
        }
    }

    false
}

/// Get positional arguments count
#[no_mangle]
pub extern "C" fn js_commander_args_count(handle: Handle) -> f64 {
    if let Some(cmd) = get_handle_mut::<CommanderHandle>(handle) {
        return cmd.args.len() as f64;
    }
    0.0
}

/// Get positional argument at index
#[no_mangle]
pub extern "C" fn js_commander_get_arg(handle: Handle, index: f64) -> *const StringHeader {
    let idx = index as usize;

    if let Some(cmd) = get_handle_mut::<CommanderHandle>(handle) {
        if idx < cmd.args.len() {
            let arg = &cmd.args[idx];
            return unsafe { js_string_from_bytes(arg.as_ptr(), arg.len() as u32) };
        }
    }

    std::ptr::null()
}
