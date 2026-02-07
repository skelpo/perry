use std::cell::RefCell;

/// Callback invoked when state changes: (state_id)
type RebuildFn = Box<dyn Fn(i64)>;

struct StateEntry {
    value: f64,
}

thread_local! {
    static STATES: RefCell<Vec<StateEntry>> = RefCell::new(Vec::new());
    static REBUILD_CB: RefCell<Option<RebuildFn>> = RefCell::new(None);
}

/// Create a new state cell with an initial value. Returns state handle (1-based).
pub fn state_create(initial: f64) -> i64 {
    STATES.with(|s| {
        let mut states = s.borrow_mut();
        states.push(StateEntry { value: initial });
        states.len() as i64 // 1-based handle
    })
}

/// Get the current value of a state cell.
pub fn state_get(handle: i64) -> f64 {
    STATES.with(|s| {
        let states = s.borrow();
        let idx = (handle - 1) as usize;
        if idx < states.len() {
            states[idx].value
        } else {
            f64::from_bits(0x7FFC_0000_0000_0001) // undefined
        }
    })
}

/// Set a new value on a state cell and trigger re-render.
pub fn state_set(handle: i64, value: f64) {
    STATES.with(|s| {
        let mut states = s.borrow_mut();
        let idx = (handle - 1) as usize;
        if idx < states.len() {
            states[idx].value = value;
        }
    });
    // Trigger rebuild
    REBUILD_CB.with(|cb| {
        if let Some(rebuild) = cb.borrow().as_ref() {
            rebuild(handle);
        }
    });
}

/// Register the rebuild callback (called by app_run to set up re-rendering).
pub fn set_rebuild_callback(cb: RebuildFn) {
    REBUILD_CB.with(|rc| {
        *rc.borrow_mut() = Some(cb);
    });
}
