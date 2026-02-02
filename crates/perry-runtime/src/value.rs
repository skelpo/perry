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

/// BigInt pointer tag: 0x7FFA_XXXX_XXXX_XXXX (48 bits for bigint pointer)
const BIGINT_TAG: u64 = 0x7FFA_0000_0000_0000;

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
type JsHandleToStringFn = extern "C" fn(f64) -> *mut crate::string::StringHeader;

static JS_HANDLE_ARRAY_GET: AtomicPtr<()> = AtomicPtr::new(std::ptr::null_mut());
static JS_HANDLE_ARRAY_LENGTH: AtomicPtr<()> = AtomicPtr::new(std::ptr::null_mut());
static JS_HANDLE_OBJECT_GET_PROPERTY: AtomicPtr<()> = AtomicPtr::new(std::ptr::null_mut());
static JS_HANDLE_TO_STRING: AtomicPtr<()> = AtomicPtr::new(std::ptr::null_mut());

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

/// Set the JS handle to string conversion function (called by perry-jsruntime)
#[no_mangle]
pub extern "C" fn js_set_handle_to_string(func: JsHandleToStringFn) {
    JS_HANDLE_TO_STRING.store(func as *mut (), Ordering::SeqCst);
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

    /// Check if this is a BigInt pointer
    #[inline]
    pub fn is_bigint(&self) -> bool {
        (self.bits & !POINTER_MASK) == BIGINT_TAG
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

    /// Create a BigInt pointer value (uses BIGINT_TAG for type discrimination)
    #[inline]
    pub fn bigint_ptr(ptr: *mut crate::bigint::BigIntHeader) -> Self {
        debug_assert!((ptr as u64) <= POINTER_MASK, "Pointer too large for NaN-boxing");
        Self { bits: BIGINT_TAG | (ptr as u64 & POINTER_MASK) }
    }

    /// Get BigInt pointer (panics if not a BigInt)
    #[inline]
    pub fn as_bigint_ptr(&self) -> *const crate::bigint::BigIntHeader {
        debug_assert!(self.is_bigint(), "Value is not a BigInt");
        (self.bits & POINTER_MASK) as *const crate::bigint::BigIntHeader
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

/// Create a NaN-boxed BigInt pointer value from an i64 raw pointer.
/// Returns the value as f64 for storage in union-typed variables.
/// This uses BIGINT_TAG (0x7FFA) to distinguish from other pointer types.
#[no_mangle]
pub extern "C" fn js_nanbox_bigint(ptr: i64) -> f64 {
    let jsval = JSValue::bigint_ptr(ptr as *mut crate::bigint::BigIntHeader);
    f64::from_bits(jsval.bits())
}

/// Check if an f64 value (interpreted as NaN-boxed) represents a BigInt.
#[no_mangle]
pub extern "C" fn js_nanbox_is_bigint(value: f64) -> bool {
    let jsval = JSValue::from_bits(value.to_bits());
    jsval.is_bigint()
}

/// Extract a BigInt pointer from a NaN-boxed f64 value.
/// Returns the pointer as i64.
#[no_mangle]
pub extern "C" fn js_nanbox_get_bigint(value: f64) -> i64 {
    let jsval = JSValue::from_bits(value.to_bits());
    if jsval.is_bigint() {
        jsval.as_bigint_ptr() as i64
    } else {
        // Fallback: might be a raw bitcast pointer
        value.to_bits() as i64
    }
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

/// Extract a string pointer from an f64 value that may be either:
/// 1. A properly NaN-boxed string (with STRING_TAG)
/// 2. A raw pointer bitcast to f64 (for locally-created strings)
/// This unified function handles both cases for function parameters.
#[no_mangle]
pub extern "C" fn js_get_string_pointer_unified(value: f64) -> i64 {
    let bits = value.to_bits();
    let jsval = JSValue::from_bits(bits);

    // First check if it's a properly NaN-boxed string
    if jsval.is_string() {
        return jsval.as_string_ptr() as i64;
    }

    // Otherwise, assume it's a raw pointer bitcast to f64
    // This handles locally-created strings that were stored via bitcast
    // We can detect this by checking if the upper bits look like a valid pointer
    // (most pointers have the upper bits as 0 or small values)
    let upper_16 = bits >> 48;
    if upper_16 < 0x7FFC {
        // Looks like a raw pointer (not a tagged NaN-box value)
        return bits as i64;
    }

    // If it's some other tagged value, return 0
    0
}

/// Check if a NaN-boxed f64 value represents a string.
#[no_mangle]
pub extern "C" fn js_nanbox_is_string(value: f64) -> bool {
    let jsval = JSValue::from_bits(value.to_bits());
    jsval.is_string()
}

/// Convert a NaN-boxed f64 value to a string pointer.
/// Handles all value types: strings (extract pointer), numbers (convert), JS handles, etc.
#[no_mangle]
pub extern "C" fn js_jsvalue_to_string(value: f64) -> *mut crate::string::StringHeader {
    // Check for JS handle first - these come from the JS runtime (e.g., process.env values)
    if is_js_handle(value) {
        let func_ptr = JS_HANDLE_TO_STRING.load(Ordering::SeqCst);
        if !func_ptr.is_null() {
            let func: JsHandleToStringFn = unsafe { std::mem::transmute(func_ptr) };
            return func(value);
        }
        // Fallback if no handler registered
        return crate::string::js_string_from_bytes(b"[JS Handle]".as_ptr(), 11);
    }

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

/// Ensure a value is a native string pointer.
/// This is specifically for fetch headers where we need to handle:
/// 1. Raw string pointers (literal strings - f64 bits ARE the pointer)
/// 2. NaN-boxed strings (STRING_TAG)
/// 3. JS handle strings (from process.env)
/// Returns the string pointer as i64.
#[no_mangle]
pub extern "C" fn js_ensure_string_ptr(value: f64) -> i64 {
    let bits = value.to_bits();

    // Check for JS handle first - these need conversion
    if is_js_handle(value) {
        let func_ptr = JS_HANDLE_TO_STRING.load(Ordering::SeqCst);
        if !func_ptr.is_null() {
            let func: JsHandleToStringFn = unsafe { std::mem::transmute(func_ptr) };
            return func(value) as i64;
        }
        // Fallback - create a placeholder string
        return crate::string::js_string_from_bytes(b"[JS Handle]".as_ptr(), 11) as i64;
    }

    // Check for NaN-boxed string (STRING_TAG)
    if (bits & TAG_MASK) == STRING_TAG {
        let ptr = (bits & POINTER_MASK) as i64;
        if ptr != 0 {
            let str_header = ptr as *const crate::string::StringHeader;
            unsafe {
                let length = (*str_header).length;
                // Make a copy of the string to ensure we have a Perry-allocated string
                let data_ptr = (str_header as *const u8).add(std::mem::size_of::<crate::string::StringHeader>());
                let copy = crate::string::js_string_from_bytes(data_ptr, length);
                return copy as i64;
            }
        }
        return ptr;
    }

    // Otherwise, treat the f64 bits directly as a pointer (raw string literal)
    bits as i64
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

/// Check if a JavaScript value is truthy.
/// In JavaScript, the following values are falsy:
/// - false
/// - 0 (and -0)
/// - NaN
/// - "" (empty string)
/// - null
/// - undefined
/// Everything else is truthy.
/// Returns 1 if truthy, 0 if falsy.
#[no_mangle]
pub extern "C" fn js_is_truthy(value: f64) -> i32 {
    let bits = value.to_bits();

    // Check for special tagged values first
    if bits == TAG_UNDEFINED || bits == TAG_NULL || bits == TAG_FALSE {
        return 0;
    }

    // TAG_TRUE is truthy
    if bits == TAG_TRUE {
        return 1;
    }

    // Check for NaN-boxed string (empty string is falsy)
    if (bits & TAG_MASK) == STRING_TAG {
        let str_ptr = (bits & POINTER_MASK) as *const crate::string::StringHeader;
        if str_ptr.is_null() {
            return 0;
        }
        // Empty string is falsy
        let len = crate::string::js_string_length(str_ptr);
        if len == 0 {
            return 0;
        }
        return 1;
    }

    // Check for NaN-boxed pointer (objects/arrays are always truthy)
    if (bits & TAG_MASK) == POINTER_TAG {
        return 1;
    }

    // Check for JS handle (always truthy - they represent objects)
    if (bits & TAG_MASK) == JS_HANDLE_TAG {
        return 1;
    }

    // Check for int32 tag
    if (bits & TAG_MASK) == INT32_TAG {
        let int_val = (bits & INT32_MASK) as i32;
        return if int_val == 0 { 0 } else { 1 };
    }

    // Check for raw pointer bits (from bitcast of string literal)
    // In a 64-bit system, valid heap pointers are typically in the range
    // 0x0000_0000_0000_1000 to 0x0000_FFFF_FFFF_FFFF
    // This handles strings that were compiled as direct pointers, not NaN-boxed
    if bits > 0x1000 && bits < 0x0001_0000_0000_0000 {
        // This could be a raw string pointer - check if it's a valid string
        let str_ptr = bits as *const crate::string::StringHeader;
        // Try to read the string length - empty string is falsy
        let len = crate::string::js_string_length(str_ptr);
        if len == 0 {
            return 0;
        }
        return 1;
    }

    // Regular f64 number: 0.0, -0.0, and NaN are falsy
    if value == 0.0 || value.is_nan() {
        return 0;
    }

    // Everything else is truthy
    1
}

/// Dynamic string comparison that handles both NaN-boxed strings and raw pointer bitcasts.
/// This is needed when comparing a PropertyGet result (NaN-boxed) with a string literal (raw bitcast).
/// Returns 1 if equal, 0 if not.
#[no_mangle]
pub extern "C" fn js_dynamic_string_equals(a: f64, b: f64) -> i32 {
    // Extract string pointers from both values, handling both representations
    let a_ptr = extract_string_ptr(a);
    let b_ptr = extract_string_ptr(b);

    if a_ptr.is_null() && b_ptr.is_null() {
        return 1;
    }
    if a_ptr.is_null() || b_ptr.is_null() {
        return 0;
    }

    if crate::string::js_string_equals(a_ptr, b_ptr) {
        1
    } else {
        0
    }
}

/// Extract a string pointer from an f64 value that might be:
/// - NaN-boxed with STRING_TAG
/// - NaN-boxed with POINTER_TAG (for strings stored as generic pointers)
/// - Raw pointer bits (from bitcast)
fn extract_string_ptr(value: f64) -> *const crate::StringHeader {
    let bits = value.to_bits();
    let jsval = JSValue::from_bits(bits);

    // Check for STRING_TAG first (e.g., from PropertyGet)
    if jsval.is_string() {
        return jsval.as_string_ptr();
    }

    // Check for POINTER_TAG (generic pointer that might be a string)
    if jsval.is_pointer() {
        return jsval.as_pointer::<crate::StringHeader>();
    }

    // Assume raw pointer bits (from bitcast of string literal)
    // In a 64-bit system, valid heap pointers are typically in the range
    // 0x0000_0000_0000_0000 to 0x0000_7FFF_FFFF_FFFF
    // Check if it looks like a valid pointer (not NaN, not a small number)
    if bits > 0x1000 && bits < 0x0001_0000_0000_0000 {
        return bits as *const crate::StringHeader;
    }

    std::ptr::null()
}

/// Unified index access that handles strings, arrays, and JS handles.
/// This is called from compiled code when the value type is not known at compile time.
/// For strings, returns the character at the given index as a NaN-boxed string.
/// For arrays, returns the element at the given index.
#[no_mangle]
pub extern "C" fn js_dynamic_array_get(value: f64, index: i32) -> f64 {
    let bits = value.to_bits();
    let jsval = JSValue::from_bits(bits);

    // Check if this is a NaN-boxed string
    if jsval.is_string() {
        // String character access
        let str_ptr = jsval.as_string_ptr();
        if !str_ptr.is_null() && index >= 0 {
            let result_ptr = crate::string::js_string_char_at(str_ptr, index);
            if !result_ptr.is_null() {
                // NaN-box the result string pointer
                return f64::from_bits(STRING_TAG | (result_ptr as u64 & POINTER_MASK));
            }
        }
        // Return empty string for invalid index
        let empty = crate::string::js_string_from_bytes(std::ptr::null(), 0);
        return f64::from_bits(STRING_TAG | (empty as u64 & POINTER_MASK));
    }

    // Check if this is a JS handle
    if is_js_handle(value) {
        // Try to use the JS runtime function if it's been registered
        let func_ptr = JS_HANDLE_ARRAY_GET.load(Ordering::SeqCst);
        if !func_ptr.is_null() {
            let func: JsHandleArrayGetFn = unsafe { std::mem::transmute(func_ptr) };
            return func(value, index);
        }
        // JS runtime not available - return undefined
        return f64::from_bits(TAG_UNDEFINED);
    }

    // Not a JS handle - it's a native array pointer
    let ptr = js_nanbox_get_pointer(value);
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

/// Dynamic array find that handles both JS handle arrays and native arrays.
/// Takes the array as f64 (may be NaN-boxed or JS handle) and a callback closure.
/// Returns the found element as f64, or NaN (undefined) if not found.
#[no_mangle]
pub extern "C" fn js_dynamic_array_find(arr_value: f64, callback: *const crate::closure::ClosureHeader) -> f64 {
    // Check if callback is null
    if callback.is_null() {
        return f64::NAN;
    }

    // Check if this is a JS handle array
    if is_js_handle(arr_value) {
        // For JS handle arrays, iterate using dynamic access
        let length = js_dynamic_array_length(arr_value);
        for i in 0..length {
            let element = js_dynamic_array_get(arr_value, i);
            let result = unsafe { crate::closure::js_closure_call1(callback, element) };
            // Truthy check: non-zero value
            if result != 0.0 {
                return element;
            }
        }
        // Not found - return undefined (NaN)
        return f64::NAN;
    }

    // Not a JS handle - extract the native array pointer
    let ptr = js_nanbox_get_pointer(arr_value);
    if ptr == 0 {
        return f64::NAN;
    }

    // Use the native array find
    crate::array::js_array_find(ptr as *const crate::array::ArrayHeader, callback)
}

/// Dynamic array findIndex that handles both JS handle arrays and native arrays.
/// Takes the array as f64 (may be NaN-boxed or JS handle) and a callback closure.
/// Returns the index as f64 (-1.0 if not found).
#[no_mangle]
pub extern "C" fn js_dynamic_array_findIndex(arr_value: f64, callback: *const crate::closure::ClosureHeader) -> f64 {
    // Check if this is a JS handle array
    if is_js_handle(arr_value) {
        // For JS handle arrays, iterate using dynamic access
        let length = js_dynamic_array_length(arr_value);
        for i in 0..length {
            let element = js_dynamic_array_get(arr_value, i);
            let result = unsafe { crate::closure::js_closure_call1(callback, element) };
            // Truthy check: non-zero value
            if result != 0.0 {
                return i as f64;
            }
        }
        // Not found
        return -1.0;
    }

    // Not a JS handle - extract the native array pointer
    let ptr = js_nanbox_get_pointer(arr_value);
    if ptr == 0 {
        return -1.0;
    }

    // Use the native array findIndex and convert to f64
    crate::array::js_array_findIndex(ptr as *const crate::array::ArrayHeader, callback) as f64
}

/// Unified object property access that handles both JS handle objects and native objects.
/// Also handles strings for property access like `.length`.
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

    // Check if this is a NaN-boxed string - handle string properties like .length
    let bits = obj_value.to_bits();
    if (bits & TAG_MASK) == STRING_TAG {
        let str_ptr = (bits & POINTER_MASK) as *const crate::string::StringHeader;
        if !str_ptr.is_null() {
            // Get the property name
            let name_slice = if property_name_ptr.is_null() {
                return f64::from_bits(TAG_UNDEFINED);
            } else if property_name_len > 0 {
                std::slice::from_raw_parts(property_name_ptr as *const u8, property_name_len)
            } else {
                std::ffi::CStr::from_ptr(property_name_ptr).to_bytes()
            };

            // Handle string properties
            if name_slice == b"length" {
                let len = crate::string::js_string_length(str_ptr);
                return len as f64;
            }
            // Other string properties return undefined
            return f64::from_bits(TAG_UNDEFINED);
        }
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

    // Check the object type tag (first u32 field of both ObjectHeader and ErrorHeader)
    let object_type = *(ptr as *const u32);

    // Handle Error objects specially
    if object_type == crate::error::OBJECT_TYPE_ERROR {
        let error_ptr = ptr as *mut crate::error::ErrorHeader;
        match property_name {
            "message" => {
                let msg = crate::error::js_error_get_message(error_ptr);
                return js_nanbox_string(msg as i64);
            }
            "name" => {
                let name = crate::error::js_error_get_name(error_ptr);
                return js_nanbox_string(name as i64);
            }
            "stack" => {
                let stack = crate::error::js_error_get_stack(error_ptr);
                return js_nanbox_string(stack as i64);
            }
            _ => {
                // Error objects don't have other properties
                return f64::from_bits(TAG_UNDEFINED);
            }
        }
    }

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

/// Dynamic Object.keys() that handles both regular objects and Error objects.
/// Takes a raw pointer (extracted from NaN-boxed value) and returns array of keys.
#[no_mangle]
pub unsafe extern "C" fn js_dynamic_object_keys(ptr: i64) -> *mut crate::array::ArrayHeader {
    if ptr == 0 {
        return crate::array::js_array_alloc(0);
    }

    // Check the object type tag (first u32 field of both ObjectHeader and ErrorHeader)
    let object_type = *(ptr as *const u32);

    // Handle Error objects specially - they have fixed keys
    if object_type == crate::error::OBJECT_TYPE_ERROR {
        // Error objects have keys: "message", "name", "stack"
        let keys = crate::array::js_array_alloc(3);

        let msg_key = crate::string::js_string_from_bytes(b"message".as_ptr(), 7);
        crate::array::js_array_push(keys, JSValue::string_ptr(msg_key));

        let name_key = crate::string::js_string_from_bytes(b"name".as_ptr(), 4);
        crate::array::js_array_push(keys, JSValue::string_ptr(name_key));

        let stack_key = crate::string::js_string_from_bytes(b"stack".as_ptr(), 5);
        crate::array::js_array_push(keys, JSValue::string_ptr(stack_key));

        return keys;
    }

    // Regular object - delegate to js_object_keys
    crate::object::js_object_keys(ptr as *const crate::object::ObjectHeader)
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
