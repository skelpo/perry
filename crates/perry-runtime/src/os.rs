//! OS module - provides operating system related utility functions

use crate::string::{js_string_from_bytes, StringHeader};
use crate::array::ArrayHeader;
use crate::object::ObjectHeader;
use std::sync::OnceLock;
use std::time::Instant;

/// Process start time for uptime calculation
static PROCESS_START: OnceLock<Instant> = OnceLock::new();

fn get_process_start() -> &'static Instant {
    PROCESS_START.get_or_init(Instant::now)
}

/// Get the operating system platform
/// Returns: "darwin", "linux", "win32", "freebsd", etc.
#[no_mangle]
pub extern "C" fn js_os_platform() -> *mut StringHeader {
    #[cfg(target_os = "macos")]
    let platform = "darwin";
    #[cfg(target_os = "linux")]
    let platform = "linux";
    #[cfg(target_os = "windows")]
    let platform = "win32";
    #[cfg(target_os = "freebsd")]
    let platform = "freebsd";
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows", target_os = "freebsd")))]
    let platform = "unknown";

    let bytes = platform.as_bytes();
    js_string_from_bytes(bytes.as_ptr(), bytes.len() as u32)
}

/// Get the operating system CPU architecture
/// Returns: "x64", "arm64", "ia32", etc.
#[no_mangle]
pub extern "C" fn js_os_arch() -> *mut StringHeader {
    #[cfg(target_arch = "x86_64")]
    let arch = "x64";
    #[cfg(target_arch = "aarch64")]
    let arch = "arm64";
    #[cfg(target_arch = "x86")]
    let arch = "ia32";
    #[cfg(target_arch = "arm")]
    let arch = "arm";
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64", target_arch = "x86", target_arch = "arm")))]
    let arch = "unknown";

    let bytes = arch.as_bytes();
    js_string_from_bytes(bytes.as_ptr(), bytes.len() as u32)
}

/// Get the hostname of the operating system
#[no_mangle]
pub extern "C" fn js_os_hostname() -> *mut StringHeader {
    match hostname::get() {
        Ok(hostname) => {
            let hostname_str = hostname.to_string_lossy();
            let bytes = hostname_str.as_bytes();
            js_string_from_bytes(bytes.as_ptr(), bytes.len() as u32)
        }
        Err(_) => {
            let default = "localhost";
            js_string_from_bytes(default.as_ptr(), default.len() as u32)
        }
    }
}

/// Get the home directory for the current user
#[no_mangle]
pub extern "C" fn js_os_homedir() -> *mut StringHeader {
    match dirs::home_dir() {
        Some(path) => {
            let path_str = path.to_string_lossy();
            let bytes = path_str.as_bytes();
            js_string_from_bytes(bytes.as_ptr(), bytes.len() as u32)
        }
        None => {
            // Fallback
            #[cfg(unix)]
            let fallback = "/home";
            #[cfg(windows)]
            let fallback = "C:\\Users";
            #[cfg(not(any(unix, windows)))]
            let fallback = "/";
            js_string_from_bytes(fallback.as_ptr(), fallback.len() as u32)
        }
    }
}

/// Get the operating system's default directory for temporary files
#[no_mangle]
pub extern "C" fn js_os_tmpdir() -> *mut StringHeader {
    let tmp = std::env::temp_dir();
    let tmp_str = tmp.to_string_lossy();
    let bytes = tmp_str.as_bytes();
    js_string_from_bytes(bytes.as_ptr(), bytes.len() as u32)
}

/// Get the total amount of system memory in bytes
#[no_mangle]
pub extern "C" fn js_os_totalmem() -> f64 {
    use sysinfo::System;
    let sys = System::new_all();
    sys.total_memory() as f64
}

/// Get the amount of free system memory in bytes
#[no_mangle]
pub extern "C" fn js_os_freemem() -> f64 {
    use sysinfo::System;
    let sys = System::new_all();
    sys.free_memory() as f64
}

/// Get the system uptime in seconds
#[no_mangle]
pub extern "C" fn js_os_uptime() -> f64 {
    use sysinfo::System;
    System::uptime() as f64
}

/// Get the process uptime in seconds (time since process started)
#[no_mangle]
pub extern "C" fn js_process_uptime() -> f64 {
    get_process_start().elapsed().as_secs_f64()
}

/// Get the current working directory
#[no_mangle]
pub extern "C" fn js_process_cwd() -> *mut StringHeader {
    let cwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| String::new());
    let bytes = cwd.as_bytes();
    js_string_from_bytes(bytes.as_ptr(), bytes.len() as u32)
}

/// Get the operating system name
/// Returns: "Darwin", "Linux", "Windows_NT", etc.
#[no_mangle]
pub extern "C" fn js_os_type() -> *mut StringHeader {
    #[cfg(target_os = "macos")]
    let os_type = "Darwin";
    #[cfg(target_os = "linux")]
    let os_type = "Linux";
    #[cfg(target_os = "windows")]
    let os_type = "Windows_NT";
    #[cfg(target_os = "freebsd")]
    let os_type = "FreeBSD";
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows", target_os = "freebsd")))]
    let os_type = "Unknown";

    let bytes = os_type.as_bytes();
    js_string_from_bytes(bytes.as_ptr(), bytes.len() as u32)
}

/// Get the operating system release
#[no_mangle]
pub extern "C" fn js_os_release() -> *mut StringHeader {
    use sysinfo::System;
    let release = System::os_version().unwrap_or_else(|| "unknown".to_string());
    let bytes = release.as_bytes();
    js_string_from_bytes(bytes.as_ptr(), bytes.len() as u32)
}

/// Get the end-of-line marker for the current operating system
#[no_mangle]
pub extern "C" fn js_os_eol() -> *mut StringHeader {
    #[cfg(windows)]
    let eol = "\r\n";
    #[cfg(not(windows))]
    let eol = "\n";

    let bytes = eol.as_bytes();
    js_string_from_bytes(bytes.as_ptr(), bytes.len() as u32)
}

/// Get information about CPUs
/// Returns an array of CPU info objects
/// TODO: Implement properly when dynamic object properties are supported
#[no_mangle]
pub extern "C" fn js_os_cpus() -> *mut ArrayHeader {
    // Return empty array for now - dynamic object properties need different API
    crate::array::js_array_alloc(0)
}

/// Get network interfaces information
/// Returns an object with interface names as keys
/// TODO: Implement properly when dynamic object properties are supported
#[no_mangle]
pub extern "C" fn js_os_network_interfaces() -> *mut ObjectHeader {
    // Return empty object for now - dynamic object properties need different API
    crate::object::js_object_alloc(0, 0)
}

/// Get information about the current user
/// Returns an object with username, uid, gid, shell, homedir
/// TODO: Implement properly when dynamic object properties are supported
#[no_mangle]
pub extern "C" fn js_os_user_info() -> *mut ObjectHeader {
    // Return empty object for now - dynamic object properties need different API
    crate::object::js_object_alloc(0, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_os_platform() {
        let platform = js_os_platform();
        assert!(!platform.is_null());
    }

    #[test]
    fn test_os_arch() {
        let arch = js_os_arch();
        assert!(!arch.is_null());
    }

    #[test]
    fn test_os_hostname() {
        let hostname = js_os_hostname();
        assert!(!hostname.is_null());
    }

    #[test]
    fn test_os_homedir() {
        let homedir = js_os_homedir();
        assert!(!homedir.is_null());
    }

    #[test]
    fn test_os_tmpdir() {
        let tmpdir = js_os_tmpdir();
        assert!(!tmpdir.is_null());
    }

    #[test]
    fn test_os_totalmem() {
        let mem = js_os_totalmem();
        assert!(mem > 0.0);
    }

    #[test]
    fn test_os_freemem() {
        let mem = js_os_freemem();
        assert!(mem > 0.0);
    }

    #[test]
    fn test_os_uptime() {
        let uptime = js_os_uptime();
        assert!(uptime >= 0.0);
    }

    #[test]
    fn test_os_type() {
        let os_type = js_os_type();
        assert!(!os_type.is_null());
    }

    #[test]
    fn test_os_release() {
        let release = js_os_release();
        assert!(!release.is_null());
    }

    #[test]
    fn test_os_eol() {
        let eol = js_os_eol();
        assert!(!eol.is_null());
    }
}
