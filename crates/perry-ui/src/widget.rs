/// Opaque handle to a UI widget (used as i64 in FFI)
pub type WidgetHandle = i64;

/// Widget type discriminant
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetKind {
    Text = 0,
    Button = 1,
    VStack = 2,
    HStack = 3,
    Spacer = 4,
}
