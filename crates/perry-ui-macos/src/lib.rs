pub mod app;
pub mod state;
pub mod widgets;

// =============================================================================
// FFI exports — these are the functions called from Cranelift-generated code
// =============================================================================

/// Create an app. title_ptr=raw string, width/height as f64.
/// Returns app handle (i64).
#[no_mangle]
pub extern "C" fn perry_ui_app_create(title_ptr: i64, width: f64, height: f64) -> i64 {
    app::app_create(title_ptr as *const u8, width, height)
}

/// Set the root widget of an app.
#[no_mangle]
pub extern "C" fn perry_ui_app_set_body(app_handle: i64, root_handle: i64) {
    app::app_set_body(app_handle, root_handle);
}

/// Run the app event loop (blocks until window closes).
#[no_mangle]
pub extern "C" fn perry_ui_app_run(app_handle: i64) {
    app::app_run(app_handle);
}

/// Create a Text label. text_ptr = raw string pointer. Returns widget handle.
#[no_mangle]
pub extern "C" fn perry_ui_text_create(text_ptr: i64) -> i64 {
    widgets::text::create(text_ptr as *const u8)
}

/// Create a Button. label_ptr = raw string, on_press = NaN-boxed closure.
/// Returns widget handle.
#[no_mangle]
pub extern "C" fn perry_ui_button_create(label_ptr: i64, on_press: f64) -> i64 {
    widgets::button::create(label_ptr as *const u8, on_press)
}

/// Create a VStack container. spacing = f64. Returns widget handle.
#[no_mangle]
pub extern "C" fn perry_ui_vstack_create(spacing: f64) -> i64 {
    widgets::vstack::create(spacing)
}

/// Create an HStack container. spacing = f64. Returns widget handle.
#[no_mangle]
pub extern "C" fn perry_ui_hstack_create(spacing: f64) -> i64 {
    widgets::hstack::create(spacing)
}

/// Add a child widget to a parent widget.
#[no_mangle]
pub extern "C" fn perry_ui_widget_add_child(parent_handle: i64, child_handle: i64) {
    widgets::add_child(parent_handle, child_handle);
}

/// Create a reactive state cell. initial = f64 value. Returns state handle.
#[no_mangle]
pub extern "C" fn perry_ui_state_create(initial: f64) -> i64 {
    state::state_create(initial)
}

/// Get the current value of a state cell.
#[no_mangle]
pub extern "C" fn perry_ui_state_get(state_handle: i64) -> f64 {
    state::state_get(state_handle)
}

/// Set a new value on a state cell.
#[no_mangle]
pub extern "C" fn perry_ui_state_set(state_handle: i64, value: f64) {
    state::state_set(state_handle, value);
}
