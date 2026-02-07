use objc2::rc::Retained;
use objc2_app_kit::{NSTextField, NSView};
use objc2_foundation::{NSString, MainThreadMarker};

use super::register_widget;

/// Create an NSTextField configured as a non-editable label.
pub fn create(text_ptr: *const u8) -> i64 {
    let text = if text_ptr.is_null() {
        ""
    } else {
        unsafe {
            let len = libc::strlen(text_ptr as *const i8);
            std::str::from_utf8_unchecked(std::slice::from_raw_parts(text_ptr, len))
        }
    };

    let mtm = MainThreadMarker::new().expect("perry/ui must run on the main thread");
    let ns_string = NSString::from_str(text);

    let label = NSTextField::labelWithString(&ns_string, mtm);
    let view: Retained<NSView> = unsafe { Retained::cast_unchecked(label) };
    register_widget(view)
}
