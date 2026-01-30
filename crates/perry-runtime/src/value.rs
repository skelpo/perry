//! JSValue representation using NaN-boxing
//!
//! NaN-boxing is a technique that encodes type information and values
//! in a 64-bit float. IEEE 754 double-precision floats have a specific
//! bit pattern for NaN (Not a Number), and we can use the unused bits
//! in the NaN payload to store pointers or small values.
//!
//! Layout (64 bits):
//! - Regular f64 values (including NaN) are stored directly
//! - Tagged values use a signaling NaN pattern: 0x7FF8... with tag in bits 48-50
//!
//! We use the top 16 bits for tagging:
//! - 0x7FF8 + tag: special values
//! - 0x7FF9: pointer
//! - 0x7FFA: int32
//! - 0x7FFB: reserved
//! - Other: regular f64

/// Tag markers - we use 0x7FFC prefix to distinguish from IEEE NaN (0x7FF8)
/// IEEE quiet NaN is 0x7FF8_0000_0000_0000, so we use 0x7FFC as our marker
const TAG_MARKER: u64 = 0x7FFC_0000_0000_0000;

/// Special singleton values
const TAG_UNDEFINED: u64 = 0x7FFC_0000_0000_0001;
const TAG_NULL: u64 = 0x7FFC_0000_0000_0002;
const TAG_FALSE: u64 = 0x7FFC_0000_0000_0003;
const TAG_TRUE: u64 = 0x7FFC_0000_0000_0004;

/// Pointer tag: 0x7FFD_XXXX_XXXX_XXXX (48 bits for pointer) - objects/arrays
const POINTER_TAG: u64 = 0x7FFD_0000_0000_0000;
const POINTER_MASK: u64 = 0x0000_FFFF_FFFF_FFFF;

/// Int32 tag: 0x7FFE_0000_XXXX_XXXX (32 bits for i32)
const INT32_TAG: u64 = 0x7FFE_0000_0000_0000;
const INT32_MASK: u64 = 0x0000_0000_FFFF_FFFF;

/// String pointer tag: 0x7FFF_XXXX_XXXX_XXXX (48 bits for string pointer)
const STRING_TAG: u64 = 0x7FFF_0000_0000_0000;

/// JS Handle tag: 0x7FFB_XXXX_XXXX_XXXX (48 bits for handle ID)
/// This is used by perry-jsruntime to reference V8 objects
const JS_HANDLE_TAG: u64 = 0x7FFB_0000_0000_0000;
const TAG_MASK: u64 = 0xFFFF_0000_0000_0000;

/// Function pointers for JS handle operations (set by perry-jsruntime)
/// These allow the unified functions to dispatch to JS runtime when needed
use std::sync::atomic::{AtomicPtr, Ordering};

type JsHandleArrayGetFn = extern "C" fn(f64, i32) -> f64;
type JsHandleArrayLengthFn = extern "C" fn(f64) -> i32;
type JsHandleObjectGetPropertyFn = extern "C" fn(f64, *const i8, usize) -> f64;

static JS_HANDLE_ARRAY_GET: AtomicPtr<()> = AtomicPtr::new(std::ptr::null_mut());
static JS_HANDLE_ARRAY_LENGTH: AtomicPtr<()> = AtomicPtr::new(std::ptr::null_mut());
static JS_HANDLE_OBJECT_GET_PROPERTY: AtomicPtr<()> = AtomicPtr::new(std::ptr::null_mut());

/// Set the JS handle array get function (called by perry-jsruntime)
#[no_mangle]
pub extern "C" fn js_set_handle_array_get(func: JsHandleArrayGetFn) {
    JS_HANDLE_ARRAY_GET.store(func as *mut (), Ordering::SeqCst);
}

/// Set the JS handle array length function (called by perry-jsruntime)
#[no_mangle]
pub extern "C" fn js_set_handle_array_length(func: JsHandleArrayLengthFn) {
    JS_HANDLE_ARRAY_LENGTH.store(func as *mut (), Ordering::SeqCst);
}

/// Set the JS handle object get property function (called by perry-jsruntime)
#[no_mangle]
pub extern "C" fn js_set_handle_object_get_property(func: JsHandleObjectGetPropertyFn) {
    JS_HANDLE_OBJECT_GET_PROPERTY.store(func as *mut (), Ordering::SeqCst);
}

/// Check if a NaN-boxed value is a JS handle
#[inline]
pub fn is_js_handle(value: f64) -> bool {
    let bits = value.to_bits();
    (bits & TAG_MASK) == JS_HANDLE_TAG
}

/// A JavaScript value using NaN-boxing representation
#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct JSValue {
    bits: u64,
}

impl JSValue {
    /// Create undefined value
    #[inline]
    pub const fn undefined() -> Self {
        Self { bits: TAG_UNDEFINED }
    }

    /// Create null value
    #[inline]
    pub const fn null() -> Self {
        Self { bits: TAG_NULL }
    }

    /// Create a boolean value
    #[inline]
    pub const fn bool(value: bool) -> Self {
        Self { bits: if value { TAG_TRUE } else { TAG_FALSE } }
    }

    /// Create an f64 number value
    #[inline]
    pub fn number(value: f64) -> Self {
        // Just reinterpret the bits - f64 values are stored directly
        Self { bits: value.to_bits() }
    }

    /// Create an i32 value (stored in payload, faster than f64 for integers)
    #[inline]
    pub const fn int32(value: i32) -> Self {
        Self { bits: INT32_TAG | ((value as u32) as u64) }
    }

    /// Create a pointer value (for heap-allocated objects)
    #[inline]
    pub fn pointer(ptr: *const u8) -> Self {
        debug_assert!((ptr as u64) <= POINTER_MASK, "Pointer too large for NaN-boxing");
        Self { bits: POINTER_TAG | (ptr as u64 & POINTER_MASK) }
    }

    /// Check if this is a number (not a tagged value)
    #[inline]
    pub fn is_number(&self) -> bool {
        // A value is a number if upper 16 bits are not in our tagged range 0x7FFC-0x7FFF
        // This allows IEEE NaN (0x7FF8), negative numbers, and all other f64 through
        let upper = self.bits >> 48;
        upper < 0x7FFC || upper > 0x7FFF
    }

    /// Check if this is undefined
    #[inline]
    pub fn is_undefined(&self) -> bool {
        self.bits == TAG_UNDEFINED
    }

    /// Check if this is null
    #[inline]
    pub fn is_null(&self) -> bool {
        self.bits == TAG_NULL
    }

    /// Check if this is a boolean
    #[inline]
    pub fn is_bool(&self) -> bool {
        self.bits == TAG_TRUE || self.bits == TAG_FALSE
    }

    /// Check if this is an int32
    #[inline]
    pub fn is_int32(&self) -> bool {
        (self.bits & !INT32_MASK) == INT32_TAG
    }

    /// Check if this is a pointer (object or array)
    #[inline]
    pub fn is_pointer(&self) -> bool {
        (self.bits & !POINTER_MASK) == POINTER_TAG
    }

    /// Check if this is a string pointer
    #[inline]
    pub fn is_string(&self) -> bool {
        (self.bits & !POINTER_MASK) == STRING_TAG
    }

    /// Get as f64 (panics if not a number)
    #[inline]
    pub fn as_number(&self) -> f64 {
        debug_assert!(self.is_number(), "Value is not a number");
        f64::from_bits(self.bits)
    }

    /// Get as bool (panics if not a boolean)
    #[inline]
    pub fn as_bool(&self) -> bool {
        debug_assert!(self.is_bool(), "Value is not a boolean");
        self.bits == TAG_TRUE
    }

    /// Get as i32 (panics if not an int32)
    #[inline]
    pub fn as_int32(&self) -> i32 {
        debug_assert!(self.is_int32(), "Value is not an int32");
        (self.bits & INT32_MASK) as i32
    }

    /// Get as pointer (panics if not a pointer)
    #[inline]
    pub fn as_pointer<T>(&self) -> *const T {
        debug_assert!(self.is_pointer(), "Value is not a pointer");
        (self.bits & POINTER_MASK) as *const T
    }

    /// Convert to f64, coercing if necessary
    pub fn to_number(&self) -> f64 {
        if self.is_number() {
            self.as_number()
        } else if self.is_int32() {
            self.as_int32() as f64
        } else if self.is_bool() {
            if self.as_bool() { 1.0 } else { 0.0 }
        } else if self.is_null() {
            0.0
        } else if self.is_undefined() {
            f64::NAN
        } else {
            // Pointer types would need object-specific conversion
            f64::NAN
        }
    }

    /// Convert to boolean (JS truthiness)
    pub fn to_bool(&self) -> bool {
        if self.is_bool() {
            self.as_bool()
        } else if self.is_number() {
            let n = self.as_number();
            n != 0.0 && !n.is_nan()
        } else if self.is_int32() {
            self.as_int32() != 0
        } else if self.is_null() || self.is_undefined() {
            false
        } else {
            // Pointers (objects) are truthy
            true
        }
    }

    /// Raw bits access (for debugging)
    #[inline]
    pub fn bits(&self) -> u64 {
        self.bits
    }

    /// Create from raw bits
    #[inline]
    pub fn from_bits(bits: u64) -> Self {
        Self { bits }
    }

    /// Create a string pointer value (uses STRING_TAG for type discrimination)
    #[inline]
    pub fn string_ptr(ptr: *mut crate::string::StringHeader) -> Self {
        debug_assert!((ptr as u64) <= POINTER_MASK, "Pointer too large for NaN-boxing");
        Self { bits: STRING_TAG | (ptr as u64 & POINTER_MASK) }
    }

    /// Get string pointer (panics if not a string)
    #[inline]
    pub fn as_string_ptr(&self) -> *const crate::string::StringHeader {
        debug_assert!(self.is_string(), "Value is not a string");
        (self.bits & POINTER_MASK) as *const crate::string::StringHeader
    }

    /// Create an object pointer value
    #[inline]
    pub fn object_ptr(ptr: *mut u8) -> Self {
        Self::pointer(ptr)
    }

    /// Create an array pointer value
    #[inline]
    pub fn array_ptr(ptr: *mut crate::array::ArrayHeader) -> Self {
        Self::pointer(ptr as *const u8)
    }
}

impl std::fmt::Debug for JSValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_undefined() {
            write!(f, "undefined")
        } else if self.is_null() {
            write!(f, "null")
        } else if self.is_bool() {
            write!(f, "{}", self.as_bool())
        } else if self.is_number() {
            write!(f, "{}", self.as_number())
        } else if self.is_int32() {
            write!(f, "{}i", self.as_int32())
        } else if self.is_pointer() {
            write!(f, "<ptr {:p}>", self.as_pointer::<u8>())
        } else {
            write!(f, "<unknown 0x{:016x}>", self.bits)
        }
    }
}

impl Default for JSValue {
    fn default() -> Self {
        Self::undefined()
    }
}

// FFI functions for creating NaN-boxed values from raw pointers

/// Create a NaN-boxed pointer value from an i64 raw pointer.
/// Returns the value as f64 for storage in union-typed variables.
#[no_mangle]
pub extern "C" fn js_nanbox_pointer(ptr: i64) -> f64 {
    let jsval = JSValue::pointer(ptr as *const u8);
    f64::from_bits(jsval.bits())
}

/// Create a NaN-boxed string pointer value from an i64 raw pointer.
/// Returns the value as f64 for storage in union-typed variables.
/// This uses STRING_TAG (0x7FFF) to distinguish from object pointers.
#[no_mangle]
pub extern "C" fn js_nanbox_string(ptr: i64) -> f64 {
    let jsval = JSValue::string_ptr(ptr as *mut crate::string::StringHeader);
    f64::from_bits(jsval.bits())
}

/// Check if an f64 value (interpreted as NaN-boxed) represents a pointer.
#[no_mangle]
pub extern "C" fn js_nanbox_is_pointer(value: f64) -> bool {
    let jsval = JSValue::from_bits(value.to_bits());
    jsval.is_pointer()
}

/// Extract a pointer from a NaN-boxed f64 value.
/// Also handles raw pointer bits (bitcast from i64) for backward compatibility.
/// Handles both POINTER_TAG and STRING_TAG.
/// Returns the pointer as i64.
#[no_mangle]
pub extern "C" fn js_nanbox_get_pointer(value: f64) -> i64 {
    let bits = value.to_bits();
    let jsval = JSValue::from_bits(bits);

    // First check for properly NaN-boxed pointers (with POINTER_TAG)
    if jsval.is_pointer() {
        return jsval.as_pointer::<u8>() as i64;
    }

    // Also check for string pointers (with STRING_TAG)
    if jsval.is_string() {
        return jsval.as_string_ptr() as i64;
    }

    // Check for raw pointer bits (from bitcast, not NaN-boxed)
    // Raw pointers on 64-bit systems typically:
    // - Have non-zero value
    // - Are in the range of valid heap addresses (positive, < 2^47)
    // - Are NOT valid normal f64 numbers (their bit pattern as f64 gives tiny denormalized numbers)
    //
    // The key insight: if the bits represent a valid pointer-sized integer
    // that doesn't match any NaN-box tag and is in valid pointer range,
    // it's likely a raw pointer that was bitcast to f64.
    if bits != 0 && bits <= POINTER_MASK {
        // Check if the upper bits don't match any NaN-box tag
        let upper = bits >> 48;
        // Our NaN-box tags use 0x7FFC-0x7FFF
        // Valid f64 NaN uses 0x7FF8
        // If upper bits are 0 or a valid pointer address prefix, treat as raw pointer
        if upper == 0 || (upper > 0 && upper < 0x7FF0) {
            return bits as i64;
        }
    }

    0
}

/// Extract a string pointer from a NaN-boxed f64 value.
/// Returns the pointer as i64.
#[no_mangle]
pub extern "C" fn js_nanbox_get_string_pointer(value: f64) -> i64 {
    let jsval = JSValue::from_bits(value.to_bits());
    if jsval.is_string() {
        jsval.as_string_ptr() as i64
    } else {
        0
    }
}

/// Check if a NaN-boxed f64 value represents a string.
#[no_mangle]
pub extern "C" fn js_nanbox_is_string(value: f64) -> bool {
    let jsval = JSValue::from_bits(value.to_bits());
    jsval.is_string()
}

/// Convert a NaN-boxed f64 value to a string pointer.
/// Handles all value types: strings (extract pointer), numbers (convert), etc.
#[no_mangle]
pub extern "C" fn js_jsvalue_to_string(value: f64) -> *mut crate::string::StringHeader {
    let jsval = JSValue::from_bits(value.to_bits());

    if jsval.is_string() {
        // Already a string - extract and return the pointer
        jsval.as_string_ptr() as *mut crate::string::StringHeader
    } else if jsval.is_undefined() {
        crate::string::js_string_from_bytes(b"undefined".as_ptr(), 9)
    } else if jsval.is_null() {
        crate::string::js_string_from_bytes(b"null".as_ptr(), 4)
    } else if jsval.is_bool() {
        if jsval.as_bool() {
            crate::string::js_string_from_bytes(b"true".as_ptr(), 4)
        } else {
            crate::string::js_string_from_bytes(b"false".as_ptr(), 5)
        }
    } else if jsval.is_int32() {
        // Convert int32 to string
        let n = jsval.as_int32();
        let s = n.to_string();
        crate::string::js_string_from_bytes(s.as_ptr(), s.len() as u32)
    } else if jsval.is_pointer() {
        // Object/array - return "[object Object]" for now
        crate::string::js_string_from_bytes(b"[object Object]".as_ptr(), 15)
    } else {
        // Regular number - use js_number_to_string
        crate::string::js_number_to_string(value)
    }
}

/// Compare two NaN-boxed f64 values for equality.
/// Handles string comparison by comparing actual string contents.
/// Returns 1 if equal, 0 if not.
#[no_mangle]
pub extern "C" fn js_jsvalue_equals(a: f64, b: f64) -> i32 {
    let a_val = JSValue::from_bits(a.to_bits());
    let b_val = JSValue::from_bits(b.to_bits());

    // If both are strings, compare their contents
    if a_val.is_string() && b_val.is_string() {
        let a_str = a_val.as_string_ptr();
        let b_str = b_val.as_string_ptr();
        if crate::string::js_string_equals(a_str, b_str) {
            return 1;
        }
        return 0;
    }

    // Otherwise, compare bits directly (works for numbers, null, undefined, etc.)
    if a.to_bits() == b.to_bits() {
        1
    } else {
        0
    }
}

/// Unified array element access that handles both JS handle arrays and native arrays.
/// This is called from compiled code when the array type is not known at compile time.
#[no_mangle]
pub extern "C" fn js_dynamic_array_get(arr_value: f64, index: i32) -> f64 {
    // Check if this is a JS handle
    if is_js_handle(arr_value) {
        // Try to use the JS runtime function if it's been registered
        let func_ptr = JS_HANDLE_ARRAY_GET.load(Ordering::SeqCst);
        if !func_ptr.is_null() {
            let func: JsHandleArrayGetFn = unsafe { std::mem::transmute(func_ptr) };
            return func(arr_value, index);
        }
        // JS runtime not available - return undefined
        return f64::from_bits(TAG_UNDEFINED);
    }

    // Not a JS handle - it's a native array pointer
    let ptr = js_nanbox_get_pointer(arr_value);
    if ptr == 0 {
        // Invalid pointer - return undefined
        return f64::from_bits(TAG_UNDEFINED);
    }

    // Call the native array get function
    let result_bits = crate::array::js_array_get_jsvalue(ptr as *const crate::array::ArrayHeader, index as u32);
    f64::from_bits(result_bits)
}

/// Unified array length access that handles both JS handle arrays and native arrays.
#[no_mangle]
pub extern "C" fn js_dynamic_array_length(arr_value: f64) -> i32 {
    // Check if this is a JS handle
    if is_js_handle(arr_value) {
        // Try to use the JS runtime function if it's been registered
        let func_ptr = JS_HANDLE_ARRAY_LENGTH.load(Ordering::SeqCst);
        if !func_ptr.is_null() {
            let func: JsHandleArrayLengthFn = unsafe { std::mem::transmute(func_ptr) };
            return func(arr_value);
        }
        // JS runtime not available - return 0
        return 0;
    }

    // Not a JS handle - extract the pointer
    let ptr = js_nanbox_get_pointer(arr_value);
    if ptr == 0 {
        return 0;
    }

    crate::array::js_array_length(ptr as *const crate::array::ArrayHeader) as i32
}

/// Unified object property access that handles both JS handle objects and native objects.
#[no_mangle]
pub unsafe extern "C" fn js_dynamic_object_get_property(
    obj_value: f64,
    property_name_ptr: *const i8,
    property_name_len: usize,
) -> f64 {
    // Check if this is a JS handle
    if is_js_handle(obj_value) {
        // Try to use the JS runtime function if it's been registered
        let func_ptr = JS_HANDLE_OBJECT_GET_PROPERTY.load(Ordering::SeqCst);
        if !func_ptr.is_null() {
            let func: JsHandleObjectGetPropertyFn = unsafe { std::mem::transmute(func_ptr) };
            return func(obj_value, property_name_ptr, property_name_len);
        }
        // JS runtime not available - return undefined
        return f64::from_bits(TAG_UNDEFINED);
    }

    // Not a JS handle - it's a native object pointer
    let ptr = js_nanbox_get_pointer(obj_value);
    if ptr == 0 {
        // Invalid pointer - return undefined
        return f64::from_bits(TAG_UNDEFINED);
    }

    // Get the key string
    let name_slice = if property_name_ptr.is_null() {
        return f64::from_bits(TAG_UNDEFINED);
    } else if property_name_len > 0 {
        std::slice::from_raw_parts(property_name_ptr as *const u8, property_name_len)
    } else {
        // Null-terminated C string
        std::ffi::CStr::from_ptr(property_name_ptr).to_bytes()
    };

    let property_name = match std::str::from_utf8(name_slice) {
        Ok(s) => s,
        Err(_) => return f64::from_bits(TAG_UNDEFINED),
    };

    // Create a Perry string for the key
    let key_ptr = crate::string::js_string_from_bytes(
        property_name.as_ptr(),
        property_name.len() as u32,
    );

    // Call native object property access
    crate::object::js_object_get_field_by_name_f64(
        ptr as *const crate::object::ObjectHeader,
        key_ptr,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_undefined() {
        let v = JSValue::undefined();
        assert!(v.is_undefined());
        assert!(!v.is_null());
        assert!(!v.is_number());
    }

    #[test]
    fn test_null() {
        let v = JSValue::null();
        assert!(v.is_null());
        assert!(!v.is_undefined());
    }

    #[test]
    fn test_bool() {
        let t = JSValue::bool(true);
        let f = JSValue::bool(false);
        assert!(t.is_bool());
        assert!(f.is_bool());
        assert!(t.as_bool());
        assert!(!f.as_bool());
    }

    #[test]
    fn test_number() {
        let v = JSValue::number(42.5);
        assert!(v.is_number());
        assert_eq!(v.as_number(), 42.5);

        let zero = JSValue::number(0.0);
        assert!(zero.is_number());
        assert_eq!(zero.as_number(), 0.0);

        let neg = JSValue::number(-123.456);
        assert!(neg.is_number());
        assert_eq!(neg.as_number(), -123.456);
    }

    #[test]
    fn test_int32() {
        let v = JSValue::int32(42);
        assert!(v.is_int32());
        assert_eq!(v.as_int32(), 42);

        let neg = JSValue::int32(-100);
        assert!(neg.is_int32());
        assert_eq!(neg.as_int32(), -100);
    }

    #[test]
    fn test_truthiness() {
        assert!(!JSValue::undefined().to_bool());
        assert!(!JSValue::null().to_bool());
        assert!(!JSValue::bool(false).to_bool());
        assert!(JSValue::bool(true).to_bool());
        assert!(!JSValue::number(0.0).to_bool());
        assert!(JSValue::number(1.0).to_bool());
        assert!(JSValue::number(-1.0).to_bool());
        assert!(!JSValue::number(f64::NAN).to_bool());
    }
}
