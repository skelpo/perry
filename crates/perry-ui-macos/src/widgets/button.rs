use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Sel};
use objc2::{define_class, msg_send, AnyThread, DefinedClass};
use objc2_app_kit::{NSButton, NSView};
use objc2_foundation::{NSObject, NSString, MainThreadMarker};
use std::cell::RefCell;
use std::collections::HashMap;

thread_local! {
    /// Map from button target object address to closure pointer (f64 NaN-boxed)
    static BUTTON_CALLBACKS: RefCell<HashMap<usize, f64>> = RefCell::new(HashMap::new());
}

extern "C" {
    fn js_closure_call0(closure: i64) -> f64;
}

/// Internal state for our button target
pub struct PerryButtonTargetIvars {
    callback_key: std::cell::Cell<usize>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[name = "PerryButtonTarget"]
    #[ivars = PerryButtonTargetIvars]
    pub struct PerryButtonTarget;

    impl PerryButtonTarget {
        #[unsafe(method(buttonPressed:))]
        fn button_pressed(&self, _sender: &AnyObject) {
            let key = self.ivars().callback_key.get();
            BUTTON_CALLBACKS.with(|cbs| {
                if let Some(&closure_f64) = cbs.borrow().get(&key) {
                    let closure_i64 = closure_f64.to_bits() as i64;
                    unsafe {
                        js_closure_call0(closure_i64);
                    }
                }
            });
        }
    }
);

impl PerryButtonTarget {
    fn new() -> Retained<Self> {
        let this = Self::alloc().set_ivars(PerryButtonTargetIvars {
            callback_key: std::cell::Cell::new(0),
        });
        unsafe { msg_send![super(this), init] }
    }
}

/// Create an NSButton with a label and closure callback.
/// `label_ptr` is a raw string pointer, `on_press` is a NaN-boxed closure pointer.
pub fn create(label_ptr: *const u8, on_press: f64) -> i64 {
    let label = if label_ptr.is_null() {
        ""
    } else {
        unsafe {
            let len = libc::strlen(label_ptr as *const i8);
            std::str::from_utf8_unchecked(std::slice::from_raw_parts(label_ptr, len))
        }
    };

    let mtm = MainThreadMarker::new().expect("perry/ui must run on the main thread");
    let ns_string = NSString::from_str(label);

    unsafe {
        let button = NSButton::buttonWithTitle_target_action(
            &ns_string,
            None,
            None,
            mtm,
        );

        // Create our target object and wire it up
        let target = PerryButtonTarget::new();
        let target_addr = Retained::as_ptr(&target) as usize;
        target.ivars().callback_key.set(target_addr);

        // Store the closure callback
        BUTTON_CALLBACKS.with(|cbs| {
            cbs.borrow_mut().insert(target_addr, on_press);
        });

        // Set target and action
        let sel = Sel::register(c"buttonPressed:");
        button.setTarget(Some(&target));
        button.setAction(Some(sel));

        // Prevent target from being deallocated (leak the Retained reference)
        std::mem::forget(target);

        let view: Retained<NSView> = Retained::cast_unchecked(button);
        super::register_widget(view)
    }
}
