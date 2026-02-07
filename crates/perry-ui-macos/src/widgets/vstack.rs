use objc2::rc::Retained;
use objc2_app_kit::{NSStackView, NSView, NSUserInterfaceLayoutOrientation, NSLayoutAttribute};
use objc2_foundation::MainThreadMarker;

/// Create an NSStackView with vertical orientation.
pub fn create(spacing: f64) -> i64 {
    let mtm = MainThreadMarker::new().expect("perry/ui must run on the main thread");
    let stack = NSStackView::new(mtm);
    stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
    stack.setSpacing(spacing);
    stack.setAlignment(NSLayoutAttribute::CenterX);
    let view: Retained<NSView> = unsafe { Retained::cast_unchecked(stack) };
    super::register_widget(view)
}
