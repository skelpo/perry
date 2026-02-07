pub mod text;
pub mod button;
pub mod vstack;
pub mod hstack;

use objc2::rc::Retained;
use objc2::runtime::AnyClass;
use objc2_app_kit::{NSView, NSStackView};
use objc2_foundation::NSObjectProtocol;
use std::cell::RefCell;

thread_local! {
    /// Map from widget handle (1-based) to NSView
    static WIDGETS: RefCell<Vec<Retained<NSView>>> = RefCell::new(Vec::new());
}

/// Store an NSView and return its handle (1-based i64).
pub fn register_widget(view: Retained<NSView>) -> i64 {
    WIDGETS.with(|w| {
        let mut widgets = w.borrow_mut();
        widgets.push(view);
        widgets.len() as i64
    })
}

/// Retrieve the NSView for a given handle.
pub fn get_widget(handle: i64) -> Option<Retained<NSView>> {
    WIDGETS.with(|w| {
        let widgets = w.borrow();
        let idx = (handle - 1) as usize;
        widgets.get(idx).cloned()
    })
}

/// Add a child view to a parent view.
/// If the parent is an NSStackView, uses addArrangedSubview for proper layout.
pub fn add_child(parent_handle: i64, child_handle: i64) {
    if let (Some(parent), Some(child)) = (get_widget(parent_handle), get_widget(child_handle)) {
        // Check if parent is an NSStackView
        let is_stack = if let Some(cls) = AnyClass::get(c"NSStackView") {
            parent.isKindOfClass(cls)
        } else {
            false
        };

        if is_stack {
            // Safety: we verified the type with isKindOfClass
            let stack: &NSStackView = unsafe { &*(Retained::as_ptr(&parent) as *const NSStackView) };
            stack.addArrangedSubview(&child);
        } else {
            parent.addSubview(&child);
        }
    }
}
