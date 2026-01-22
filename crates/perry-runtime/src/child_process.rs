//! Child Process module - provides process spawning capabilities

use std::process::{Command, Stdio};
use std::io::Read;

use crate::string::{js_string_from_bytes, StringHeader};
use crate::buffer::BufferHeader;
use crate::object::ObjectHeader;

/// Execute a command synchronously and return stdout as a buffer/string
/// Returns: Buffer containing stdout, or null on error
#[no_mangle]
pub extern "C" fn js_child_process_exec_sync(
    cmd_ptr: *const StringHeader,
    _options_ptr: *const ObjectHeader,
) -> *mut BufferHeader {
    if cmd_ptr.is_null() {
        return std::ptr::null_mut();
    }

    unsafe {
        let len = (*cmd_ptr).length as usize;
        let data_ptr = (cmd_ptr as *const u8).add(std::mem::size_of::<StringHeader>());
        let cmd_bytes = std::slice::from_raw_parts(data_ptr, len);

        let cmd_str = match std::str::from_utf8(cmd_bytes) {
            Ok(s) => s,
            Err(_) => return std::ptr::null_mut(),
        };

        // Execute the command using shell
        #[cfg(unix)]
        let output = Command::new("sh")
            .arg("-c")
            .arg(cmd_str)
            .output();

        #[cfg(windows)]
        let output = Command::new("cmd")
            .arg("/C")
            .arg(cmd_str)
            .output();

        match output {
            Ok(output) => {
                // Return stdout as a buffer
                let stdout = &output.stdout;
                let buf = crate::buffer::js_buffer_alloc(stdout.len() as i32, 0);
                if !buf.is_null() {
                    let buf_data = (buf as *mut u8).add(std::mem::size_of::<BufferHeader>());
                    std::ptr::copy_nonoverlapping(stdout.as_ptr(), buf_data, stdout.len());
                    (*buf).length = stdout.len() as u32;
                }
                buf
            }
            Err(_) => std::ptr::null_mut(),
        }
    }
}

/// Execute a command synchronously with more control (spawnSync)
/// Returns: Object with stdout, stderr, status, etc.
#[no_mangle]
pub extern "C" fn js_child_process_spawn_sync(
    cmd_ptr: *const StringHeader,
    args_ptr: *const crate::array::ArrayHeader,
    _options_ptr: *const ObjectHeader,
) -> *mut ObjectHeader {
    if cmd_ptr.is_null() {
        return std::ptr::null_mut();
    }

    unsafe {
        // Get command string
        let cmd_len = (*cmd_ptr).length as usize;
        let cmd_data = (cmd_ptr as *const u8).add(std::mem::size_of::<StringHeader>());
        let cmd_bytes = std::slice::from_raw_parts(cmd_data, cmd_len);

        let cmd_str = match std::str::from_utf8(cmd_bytes) {
            Ok(s) => s,
            Err(_) => return std::ptr::null_mut(),
        };

        // Build command
        let mut command = Command::new(cmd_str);

        // Add arguments if provided
        if !args_ptr.is_null() {
            let args_len = (*args_ptr).length as usize;
            let args_data = (args_ptr as *const u8).add(std::mem::size_of::<crate::array::ArrayHeader>()) as *const f64;

            for i in 0..args_len {
                let arg_ptr = (*args_data.add(i)).to_bits() as *const StringHeader;
                if !arg_ptr.is_null() {
                    let arg_len = (*arg_ptr).length as usize;
                    let arg_data = (arg_ptr as *const u8).add(std::mem::size_of::<StringHeader>());
                    let arg_bytes = std::slice::from_raw_parts(arg_data, arg_len);
                    if let Ok(arg_str) = std::str::from_utf8(arg_bytes) {
                        command.arg(arg_str);
                    }
                }
            }
        }

        // Execute the command
        match command.output() {
            Ok(output) => {
                // Create result object with stdout, stderr, status (3 fields)
                // Fields: 0=stdout, 1=stderr, 2=status
                let result = crate::object::js_object_alloc(0, 3);

                // Set stdout as buffer (field 0)
                let stdout_buf = crate::buffer::js_buffer_alloc(output.stdout.len() as i32, 0);
                if !stdout_buf.is_null() {
                    let buf_data = (stdout_buf as *mut u8).add(std::mem::size_of::<BufferHeader>());
                    std::ptr::copy_nonoverlapping(output.stdout.as_ptr(), buf_data, output.stdout.len());
                    (*stdout_buf).length = output.stdout.len() as u32;
                }
                crate::object::js_object_set_field_f64(
                    result,
                    0,
                    (stdout_buf as u64) as f64
                );

                // Set stderr as buffer (field 1)
                let stderr_buf = crate::buffer::js_buffer_alloc(output.stderr.len() as i32, 0);
                if !stderr_buf.is_null() {
                    let buf_data = (stderr_buf as *mut u8).add(std::mem::size_of::<BufferHeader>());
                    std::ptr::copy_nonoverlapping(output.stderr.as_ptr(), buf_data, output.stderr.len());
                    (*stderr_buf).length = output.stderr.len() as u32;
                }
                crate::object::js_object_set_field_f64(
                    result,
                    1,
                    (stderr_buf as u64) as f64
                );

                // Set status (field 2)
                let status = output.status.code().unwrap_or(-1) as f64;
                crate::object::js_object_set_field_f64(
                    result,
                    2,
                    status
                );

                result
            }
            Err(_) => std::ptr::null_mut(),
        }
    }
}

/// Spawn a process asynchronously
/// Note: This returns a simplified handle for now
/// Full async support would require integration with the async runtime
#[no_mangle]
pub extern "C" fn js_child_process_spawn(
    _cmd_ptr: *const StringHeader,
    _args_ptr: *const crate::array::ArrayHeader,
    _options_ptr: *const ObjectHeader,
) -> *mut ObjectHeader {
    // TODO: Implement async spawn with proper ChildProcess handle
    // For now, return null - async child processes need event loop integration
    std::ptr::null_mut()
}

/// Execute a command asynchronously with shell
/// Note: This returns a simplified handle for now
#[no_mangle]
pub extern "C" fn js_child_process_exec(
    _cmd_ptr: *const StringHeader,
    _options_ptr: *const ObjectHeader,
    _callback_ptr: *const crate::closure::ClosureHeader,
) -> *mut ObjectHeader {
    // TODO: Implement async exec with callback
    // For now, return null - async child processes need event loop integration
    std::ptr::null_mut()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exec_sync_echo() {
        let cmd = "echo hello";
        let cmd_ptr = js_string_from_bytes(cmd.as_ptr(), cmd.len() as u32);
        let result = js_child_process_exec_sync(cmd_ptr, std::ptr::null());

        assert!(!result.is_null());
        unsafe {
            assert!((*result).length > 0);
        }
    }
}
