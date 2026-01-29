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
/// Returns the pointer as i64.
#[no_mangle]
pub extern "C" fn js_nanbox_get_pointer(value: f64) -> i64 {
    let bits = value.to_bits();
    let jsval = JSValue::from_bits(bits);

    // First check for properly NaN-boxed pointers (with POINTER_TAG)
    if jsval.is_pointer() {
        return jsval.as_pointer::<u8>() as i64;
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
