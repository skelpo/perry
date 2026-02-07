use objc2::rc::Retained;
use objc2::MainThreadOnly;
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSBackingStoreType, NSWindow,
    NSWindowStyleMask,
};
use objc2_core_foundation::{CGPoint, CGSize, CGRect};
use objc2_foundation::{NSString, MainThreadMarker};

use std::cell::RefCell;

use crate::widgets;

thread_local! {
    static APPS: RefCell<Vec<AppEntry>> = RefCell::new(Vec::new());
}

struct AppEntry {
    window: Retained<NSWindow>,
    _root_widget: Option<i64>,
}

/// Create an app with title, width, height.
pub fn app_create(title_ptr: *const u8, width: f64, height: f64) -> i64 {
    let title = if title_ptr.is_null() {
        "Perry App"
    } else {
        unsafe {
            let len = libc::strlen(title_ptr as *const i8);
            std::str::from_utf8_unchecked(std::slice::from_raw_parts(title_ptr, len))
        }
    };

    let w = if width > 0.0 { width } else { 400.0 };
    let h = if height > 0.0 { height } else { 300.0 };

    let mtm = MainThreadMarker::new().expect("perry/ui must run on the main thread");

    unsafe {
        let style = NSWindowStyleMask::Titled
            | NSWindowStyleMask::Closable
            | NSWindowStyleMask::Miniaturizable
            | NSWindowStyleMask::Resizable;

        let frame = CGRect::new(CGPoint::new(200.0, 200.0), CGSize::new(w, h));

        let window = NSWindow::initWithContentRect_styleMask_backing_defer(
            NSWindow::alloc(mtm),
            frame,
            style,
            NSBackingStoreType::Buffered,
            false,
        );

        let ns_title = NSString::from_str(title);
        window.setTitle(&ns_title);

        APPS.with(|a| {
            let mut apps = a.borrow_mut();
            apps.push(AppEntry {
                window,
                _root_widget: None,
            });
            apps.len() as i64 // 1-based handle
        })
    }
}

/// Set the root widget (body) of the app.
pub fn app_set_body(app_handle: i64, root_handle: i64) {
    APPS.with(|a| {
        let mut apps = a.borrow_mut();
        let idx = (app_handle - 1) as usize;
        if idx < apps.len() {
            apps[idx]._root_widget = Some(root_handle);

            if let Some(view) = widgets::get_widget(root_handle) {
                apps[idx].window.setContentView(Some(&view));
            }
        }
    });
}

/// Run the application event loop (blocks).
pub fn app_run(_app_handle: i64) {
    let mtm = MainThreadMarker::new().expect("perry/ui must run on the main thread");

    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Regular);

    APPS.with(|a| {
        let apps = a.borrow();
        for entry in apps.iter() {
            entry.window.center();
            entry.window.makeKeyAndOrderFront(None);
        }
    });

    // Activate the app (bring to front)
    #[allow(deprecated)]
    app.activateIgnoringOtherApps(true);

    app.run();
}
