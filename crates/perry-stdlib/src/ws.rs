//! WebSocket module (ws compatible)
//!
//! Native implementation of the 'ws' npm package using tokio-tungstenite.
//! Provides WebSocket client functionality.

use perry_runtime::{js_string_from_bytes, JSValue, StringHeader};
use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::sync::Mutex;
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::common::async_bridge::{queue_promise_resolution, spawn};

// WebSocket handle storage
lazy_static::lazy_static! {
    static ref WS_CONNECTIONS: Mutex<HashMap<usize, WsConnection>> = Mutex::new(HashMap::new());
    static ref NEXT_WS_ID: Mutex<usize> = Mutex::new(1);
}

struct WsConnection {
    sender: mpsc::UnboundedSender<WsCommand>,
    messages: Vec<String>,
    is_open: bool,
}

enum WsCommand {
    Send(String),
    Close,
}

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

/// Create a new WebSocket connection
/// new WebSocket(url) -> Promise<WebSocket>
#[no_mangle]
pub unsafe extern "C" fn js_ws_connect(url_ptr: *const StringHeader) -> *mut perry_runtime::Promise {
    let promise = perry_runtime::js_promise_new();
    let promise_ptr = promise as usize;

    let url = match string_from_header(url_ptr) {
        Some(u) => u,
        None => {
            let err_msg = "Invalid URL";
            let err_str = js_string_from_bytes(err_msg.as_ptr(), err_msg.len() as u32);
            let err_bits = JSValue::pointer(err_str as *const u8).bits();
            queue_promise_resolution(promise_ptr, false, err_bits);
            return promise;
        }
    };

    spawn(async move {
        match connect_async(&url).await {
            Ok((ws_stream, _response)) => {
                let (mut write, mut read) = ws_stream.split();

                // Create command channel
                let (tx, mut rx) = mpsc::unbounded_channel::<WsCommand>();

                // Allocate connection ID
                let mut id_guard = NEXT_WS_ID.lock().unwrap();
                let ws_id = *id_guard;
                *id_guard += 1;
                drop(id_guard);

                // Store connection
                WS_CONNECTIONS.lock().unwrap().insert(ws_id, WsConnection {
                    sender: tx,
                    messages: Vec::new(),
                    is_open: true,
                });

                // Spawn task to handle outgoing messages
                let ws_id_send = ws_id;
                tokio::spawn(async move {
                    while let Some(cmd) = rx.recv().await {
                        match cmd {
                            WsCommand::Send(msg) => {
                                if let Err(_) = write.send(Message::Text(msg)).await {
                                    break;
                                }
                            }
                            WsCommand::Close => {
                                let _ = write.send(Message::Close(None)).await;
                                break;
                            }
                        }
                    }

                    // Mark as closed
                    if let Some(conn) = WS_CONNECTIONS.lock().unwrap().get_mut(&ws_id_send) {
                        conn.is_open = false;
                    }
                });

                // Spawn task to handle incoming messages
                let ws_id_recv = ws_id;
                tokio::spawn(async move {
                    while let Some(msg_result) = read.next().await {
                        match msg_result {
                            Ok(Message::Text(text)) => {
                                if let Some(conn) = WS_CONNECTIONS.lock().unwrap().get_mut(&ws_id_recv) {
                                    conn.messages.push(text);
                                }
                            }
                            Ok(Message::Close(_)) => {
                                if let Some(conn) = WS_CONNECTIONS.lock().unwrap().get_mut(&ws_id_recv) {
                                    conn.is_open = false;
                                }
                                break;
                            }
                            Err(_) => {
                                if let Some(conn) = WS_CONNECTIONS.lock().unwrap().get_mut(&ws_id_recv) {
                                    conn.is_open = false;
                                }
                                break;
                            }
                            _ => {}
                        }
                    }
                });

                // Return WebSocket handle
                let result_bits = (ws_id as f64).to_bits();
                queue_promise_resolution(promise_ptr, true, result_bits);
            }
            Err(e) => {
                let err_msg = format!("WebSocket connection error: {}", e);
                let err_str = js_string_from_bytes(err_msg.as_ptr(), err_msg.len() as u32);
                let err_bits = JSValue::pointer(err_str as *const u8).bits();
                queue_promise_resolution(promise_ptr, false, err_bits);
            }
        }
    });

    promise
}

/// Send a message through the WebSocket
/// ws.send(message) -> void
#[no_mangle]
pub unsafe extern "C" fn js_ws_send(handle: i64, message_ptr: *const StringHeader) {
    let ws_id = handle as usize;
    let message = match string_from_header(message_ptr) {
        Some(m) => m,
        None => return,
    };

    let guard = WS_CONNECTIONS.lock().unwrap();
    if let Some(conn) = guard.get(&ws_id) {
        let _ = conn.sender.send(WsCommand::Send(message));
    }
}

/// Close the WebSocket connection
/// ws.close() -> void
#[no_mangle]
pub extern "C" fn js_ws_close(handle: i64) {
    let ws_id = handle as usize;

    let guard = WS_CONNECTIONS.lock().unwrap();
    if let Some(conn) = guard.get(&ws_id) {
        let _ = conn.sender.send(WsCommand::Close);
    }
}

/// Check if WebSocket is open
/// ws.readyState === WebSocket.OPEN
#[no_mangle]
pub extern "C" fn js_ws_is_open(handle: i64) -> f64 {
    let ws_id = handle as usize;

    let guard = WS_CONNECTIONS.lock().unwrap();
    match guard.get(&ws_id) {
        Some(conn) => if conn.is_open { 1.0 } else { 0.0 },
        None => 0.0,
    }
}

/// Get the number of pending messages
/// Returns the count of received messages waiting to be read
#[no_mangle]
pub extern "C" fn js_ws_message_count(handle: i64) -> f64 {
    let ws_id = handle as usize;

    let guard = WS_CONNECTIONS.lock().unwrap();
    match guard.get(&ws_id) {
        Some(conn) => conn.messages.len() as f64,
        None => 0.0,
    }
}

/// Get the next message from the queue
/// Returns null if no messages available
#[no_mangle]
pub extern "C" fn js_ws_receive(handle: i64) -> *mut StringHeader {
    let ws_id = handle as usize;

    let mut guard = WS_CONNECTIONS.lock().unwrap();
    match guard.get_mut(&ws_id) {
        Some(conn) => {
            if conn.messages.is_empty() {
                std::ptr::null_mut()
            } else {
                let msg = conn.messages.remove(0);
                js_string_from_bytes(msg.as_ptr(), msg.len() as u32)
            }
        }
        None => std::ptr::null_mut(),
    }
}

/// Wait for a message (blocking with timeout)
/// ws.waitForMessage(timeoutMs) -> Promise<string | null>
#[no_mangle]
pub unsafe extern "C" fn js_ws_wait_for_message(handle: i64, timeout_ms: f64) -> *mut perry_runtime::Promise {
    let promise = perry_runtime::js_promise_new();
    let promise_ptr = promise as usize;
    let ws_id = handle as usize;
    let timeout = std::time::Duration::from_millis(timeout_ms as u64);

    spawn(async move {
        let start = std::time::Instant::now();

        loop {
            // Check for messages
            {
                let mut guard = WS_CONNECTIONS.lock().unwrap();
                if let Some(conn) = guard.get_mut(&ws_id) {
                    if !conn.messages.is_empty() {
                        let msg = conn.messages.remove(0);
                        let result_str = js_string_from_bytes(msg.as_ptr(), msg.len() as u32);
                        let result_bits = JSValue::pointer(result_str as *const u8).bits();
                        queue_promise_resolution(promise_ptr, true, result_bits);
                        return;
                    }

                    if !conn.is_open {
                        // Connection closed
                        let result_bits = JSValue::null().bits();
                        queue_promise_resolution(promise_ptr, true, result_bits);
                        return;
                    }
                } else {
                    // Invalid handle
                    let result_bits = JSValue::null().bits();
                    queue_promise_resolution(promise_ptr, true, result_bits);
                    return;
                }
            }

            // Check timeout
            if start.elapsed() >= timeout {
                let result_bits = JSValue::null().bits();
                queue_promise_resolution(promise_ptr, true, result_bits);
                return;
            }

            // Wait a bit before checking again
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    });

    promise
}
