//! JSON Web Token module (jsonwebtoken compatible)
//!
//! Native implementation of the 'jsonwebtoken' npm package.
//! Provides JWT sign, verify, and decode functionality.

use perry_runtime::{js_string_from_bytes, StringHeader};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Helper to extract string from StringHeader pointer
unsafe fn string_from_header(ptr: *const StringHeader) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    let len = (*ptr).length as usize;
    let data_ptr = (ptr as *const u8).add(std::mem::size_of::<StringHeader>());
    let bytes = std::slice::from_raw_parts(data_ptr, len);
    std::str::from_utf8(bytes).ok().map(|s| s.to_string())
}

/// Generic claims structure that can hold any JSON
#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    #[serde(flatten)]
    data: HashMap<String, serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    exp: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    iat: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    nbf: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sub: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    iss: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    aud: Option<String>,
}

/// Sign a payload to create a JWT
/// jwt.sign(payload, secret) -> string
/// jwt.sign(payload, secret, options) -> string
#[no_mangle]
pub unsafe extern "C" fn js_jwt_sign(
    payload_ptr: *const StringHeader,
    secret_ptr: *const StringHeader,
    expires_in_secs: f64,
) -> *mut StringHeader {
    let payload_json = match string_from_header(payload_ptr) {
        Some(p) => p,
        None => return std::ptr::null_mut(),
    };

    let secret = match string_from_header(secret_ptr) {
        Some(s) => s,
        None => return std::ptr::null_mut(),
    };

    // Parse the payload JSON
    let mut claims: Claims = match serde_json::from_str(&payload_json) {
        Ok(c) => c,
        Err(_) => {
            // If it's not valid JSON, wrap it
            Claims {
                data: HashMap::new(),
                exp: None,
                iat: None,
                nbf: None,
                sub: None,
                iss: None,
                aud: None,
            }
        }
    };

    // Set expiration if provided
    if expires_in_secs > 0.0 {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        claims.exp = Some(now + expires_in_secs as u64);
        if claims.iat.is_none() {
            claims.iat = Some(now);
        }
    }

    // Create the token
    let header = Header::new(Algorithm::HS256);
    let key = EncodingKey::from_secret(secret.as_bytes());

    match encode(&header, &claims, &key) {
        Ok(token) => js_string_from_bytes(token.as_ptr(), token.len() as u32),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Verify and decode a JWT
/// jwt.verify(token, secret) -> object (payload)
#[no_mangle]
pub unsafe extern "C" fn js_jwt_verify(
    token_ptr: *const StringHeader,
    secret_ptr: *const StringHeader,
) -> *mut StringHeader {
    let token = match string_from_header(token_ptr) {
        Some(t) => t,
        None => return std::ptr::null_mut(),
    };

    let secret = match string_from_header(secret_ptr) {
        Some(s) => s,
        None => return std::ptr::null_mut(),
    };

    let key = DecodingKey::from_secret(secret.as_bytes());
    let validation = Validation::new(Algorithm::HS256);

    match decode::<Claims>(&token, &key, &validation) {
        Ok(token_data) => {
            // Return the claims as JSON
            let json = serde_json::to_string(&token_data.claims).unwrap_or_else(|_| "{}".to_string());
            js_string_from_bytes(json.as_ptr(), json.len() as u32)
        }
        Err(_) => std::ptr::null_mut(), // Invalid token
    }
}

/// Decode a JWT without verification (just parse the payload)
/// jwt.decode(token) -> object (payload)
#[no_mangle]
pub unsafe extern "C" fn js_jwt_decode(token_ptr: *const StringHeader) -> *mut StringHeader {
    let token = match string_from_header(token_ptr) {
        Some(t) => t,
        None => return std::ptr::null_mut(),
    };

    // Split the token into parts
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return std::ptr::null_mut();
    }

    // Decode the payload (second part)
    use base64::Engine;
    let engine = base64::engine::general_purpose::URL_SAFE_NO_PAD;

    match engine.decode(parts[1]) {
        Ok(payload_bytes) => {
            match String::from_utf8(payload_bytes) {
                Ok(payload_json) => {
                    // Validate it's valid JSON and return it
                    if serde_json::from_str::<serde_json::Value>(&payload_json).is_ok() {
                        js_string_from_bytes(payload_json.as_ptr(), payload_json.len() as u32)
                    } else {
                        std::ptr::null_mut()
                    }
                }
                Err(_) => std::ptr::null_mut(),
            }
        }
        Err(_) => std::ptr::null_mut(),
    }
}
