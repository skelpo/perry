//! WebSocket module (ws compatible)
//!
//! Native implementation of the 'ws' npm package using tokio-tungstenite.
//! Provides WebSocket client and server functionality.

use perry_runtime::{js_string_from_bytes, JSValue, StringHeader, ClosureHeader, js_closure_call0, js_closure_call1};
use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::sync::Mutex;
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::common::async_bridge::{queue_promise_resolution, spawn};
use crate::common::{register_handle, get_handle_mut, Handle};

// WebSocket handle storage
lazy_static::lazy_static! {
    static ref WS_CONNECTIONS: Mutex<HashMap<usize, WsConnection>> = Mutex::new(HashMap::new());
    static ref NEXT_WS_ID: Mutex<usize> = Mutex::new(1);
    /// Per-client event listeners (for .on('message', cb) etc.)
    static ref WS_CLIENT_LISTENERS: Mutex<HashMap<usize, WsClientListeners>> = Mutex::new(HashMap::new());
    /// Pending WebSocket events to be processed on the main thread
    static ref WS_PENDING_EVENTS: Mutex<Vec<PendingWsEvent>> = Mutex::new(Vec::new());
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

/// Per-client event listeners
struct WsClientListeners {
    listeners: HashMap<String, Vec<i64>>,
}

/// WebSocketServer handle
pub struct WsServerHandle {
    /// Event name -> list of closure pointers (stored as i64 for Send + Sync)
    pub listeners: HashMap<String, Vec<i64>>,
    pub port: u16,
    pub is_listening: bool,
    /// Track connected client IDs for cleanup
    pub client_ids: Vec<usize>,
    /// Shutdown signal sender
    pub shutdown_tx: Option<mpsc::UnboundedSender<()>>,
}

/// Pending WebSocket event to be dispatched on the main thread
enum PendingWsEvent {
    /// Server received a new connection: (server_handle, client_ws_id)
    Connection(Handle, usize),
    /// Client received a message: (client_ws_id, message)
    Message(usize, String),
    /// Client connection closed: (client_ws_id, code, reason)
    Close(usize, u16, String),
    /// Error on client: (client_ws_id, error_message)
    Error(usize, String),
    /// Server error: (server_handle, error_message)
    ServerError(Handle, String),
    /// Server started listening: (server_handle)
    Listening(Handle),
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

                // Initialize client listeners
                WS_CLIENT_LISTENERS.lock().unwrap().insert(ws_id, WsClientListeners {
                    listeners: HashMap::new(),
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
                                // Check if there are 'message' listeners
                                let has_listeners = WS_CLIENT_LISTENERS.lock().unwrap()
                                    .get(&ws_id_recv)
                                    .map(|l| l.listeners.get("message").map(|v| !v.is_empty()).unwrap_or(false))
                                    .unwrap_or(false);

                                if has_listeners {
                                    WS_PENDING_EVENTS.lock().unwrap().push(
                                        PendingWsEvent::Message(ws_id_recv, text)
                                    );
                                } else {
                                    // Fall back to buffering
                                    if let Some(conn) = WS_CONNECTIONS.lock().unwrap().get_mut(&ws_id_recv) {
                                        conn.messages.push(text);
                                    }
                                }
                            }
                            Ok(Message::Close(frame)) => {
                                let (code, reason) = frame
                                    .map(|f| (f.code.into(), f.reason.to_string()))
                                    .unwrap_or((1000u16, String::new()));

                                if let Some(conn) = WS_CONNECTIONS.lock().unwrap().get_mut(&ws_id_recv) {
                                    conn.is_open = false;
                                }

                                // Queue close event
                                WS_PENDING_EVENTS.lock().unwrap().push(
                                    PendingWsEvent::Close(ws_id_recv, code, reason)
                                );
                                break;
                            }
                            Err(e) => {
                                if let Some(conn) = WS_CONNECTIONS.lock().unwrap().get_mut(&ws_id_recv) {
                                    conn.is_open = false;
                                }
                                WS_PENDING_EVENTS.lock().unwrap().push(
                                    PendingWsEvent::Error(ws_id_recv, format!("{}", e))
                                );
                                WS_PENDING_EVENTS.lock().unwrap().push(
                                    PendingWsEvent::Close(ws_id_recv, 1006, String::new())
                                );
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

/// Close the WebSocket connection or server
/// ws.close() / wss.close() -> void
/// Checks if handle is a server first, then falls back to client close
#[no_mangle]
pub unsafe extern "C" fn js_ws_close(handle: i64) {
    // Check if this is a server handle
    if get_handle_mut::<WsServerHandle>(handle).is_some() {
        js_ws_server_close(handle);
        return;
    }

    // Otherwise close client connection
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

// ============================================================================
// WebSocketServer (wss) implementation
// ============================================================================

/// Register an event listener on a WebSocket handle (server or client).
/// Unified function: checks handle type at runtime.
///
/// js_ws_on(handle, event_name_ptr, callback_ptr) -> handle
#[no_mangle]
pub unsafe extern "C" fn js_ws_on(
    handle: i64,
    event_name_ptr: *const StringHeader,
    callback_ptr: i64,
) -> i64 {
    let event_name = match string_from_header(event_name_ptr) {
        Some(name) => name,
        None => return handle,
    };

    if callback_ptr == 0 {
        return handle;
    }

    // Try server handle first
    if let Some(server) = get_handle_mut::<WsServerHandle>(handle) {
        server
            .listeners
            .entry(event_name)
            .or_insert_with(Vec::new)
            .push(callback_ptr);
        return handle;
    }

    // Otherwise treat as client ws_id
    let ws_id = handle as usize;
    let mut guard = WS_CLIENT_LISTENERS.lock().unwrap();
    let entry = guard.entry(ws_id).or_insert_with(|| WsClientListeners {
        listeners: HashMap::new(),
    });
    entry
        .listeners
        .entry(event_name)
        .or_insert_with(Vec::new)
        .push(callback_ptr);

    handle
}

/// Create a new WebSocketServer
/// new WebSocketServer({ port }) -> handle (synchronous, starts listening immediately)
#[no_mangle]
pub unsafe extern "C" fn js_ws_server_new(opts_f64: f64) -> Handle {
    // Extract port from options object
    let port = {
        let opts_bits = opts_f64.to_bits();
        // Check if it's a NaN-boxed pointer (object)
        let ptr_tag: u64 = 0x7FFD_0000_0000_0000;
        let mask: u64 = 0xFFFF_0000_0000_0000;
        if (opts_bits & mask) == ptr_tag {
            // Extract raw pointer
            let ptr = (opts_bits & 0x0000_FFFF_FFFF_FFFF) as *const perry_runtime::ObjectHeader;
            if !ptr.is_null() {
                // Get 'port' field
                let key = "port";
                let key_str = js_string_from_bytes(key.as_ptr(), key.len() as u32);
                let val = perry_runtime::js_object_get_field_by_name(ptr, key_str);
                let val_f64 = f64::from_bits(val.bits());
                if val_f64.is_finite() && val_f64 > 0.0 {
                    val_f64 as u16
                } else {
                    0
                }
            } else {
                0
            }
        } else if opts_f64.is_finite() && opts_f64 > 0.0 {
            // Maybe port was passed directly as a number
            opts_f64 as u16
        } else {
            0
        }
    };

    let (shutdown_tx, mut shutdown_rx) = mpsc::unbounded_channel::<()>();

    let server_handle = register_handle(WsServerHandle {
        listeners: HashMap::new(),
        port,
        is_listening: false,
        client_ids: Vec::new(),
        shutdown_tx: Some(shutdown_tx),
    });

    // Spawn the accept loop
    let handle_id = server_handle;
    spawn(async move {
        let addr = format!("0.0.0.0:{}", port);
        let listener = match tokio::net::TcpListener::bind(&addr).await {
            Ok(l) => l,
            Err(e) => {
                WS_PENDING_EVENTS.lock().unwrap().push(
                    PendingWsEvent::ServerError(handle_id, format!("WebSocketServer bind error: {}", e))
                );
                return;
            }
        };

        // Queue 'listening' event
        WS_PENDING_EVENTS.lock().unwrap().push(
            PendingWsEvent::Listening(handle_id)
        );

        // Mark as listening
        if let Some(server) = get_handle_mut::<WsServerHandle>(handle_id) {
            server.is_listening = true;
        }

        loop {
            tokio::select! {
                accept_result = listener.accept() => {
                    match accept_result {
                        Ok((tcp_stream, _addr)) => {
                            // Upgrade to WebSocket
                            match tokio_tungstenite::accept_async(tcp_stream).await {
                                Ok(ws_stream) => {
                                    let (mut write, mut read) = ws_stream.split();
                                    let (tx, mut rx) = mpsc::unbounded_channel::<WsCommand>();

                                    // Allocate client ID
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

                                    // Initialize client listeners
                                    WS_CLIENT_LISTENERS.lock().unwrap().insert(ws_id, WsClientListeners {
                                        listeners: HashMap::new(),
                                    });

                                    // Track client on server
                                    if let Some(server) = get_handle_mut::<WsServerHandle>(handle_id) {
                                        server.client_ids.push(ws_id);
                                    }

                                    // Queue 'connection' event
                                    WS_PENDING_EVENTS.lock().unwrap().push(
                                        PendingWsEvent::Connection(handle_id, ws_id)
                                    );

                                    // Spawn outgoing message handler
                                    let ws_id_send = ws_id;
                                    tokio::spawn(async move {
                                        while let Some(cmd) = rx.recv().await {
                                            match cmd {
                                                WsCommand::Send(msg) => {
                                                    if write.send(Message::Text(msg)).await.is_err() {
                                                        break;
                                                    }
                                                }
                                                WsCommand::Close => {
                                                    let _ = write.send(Message::Close(None)).await;
                                                    break;
                                                }
                                            }
                                        }
                                        if let Some(conn) = WS_CONNECTIONS.lock().unwrap().get_mut(&ws_id_send) {
                                            conn.is_open = false;
                                        }
                                    });

                                    // Spawn incoming message handler
                                    let ws_id_recv = ws_id;
                                    tokio::spawn(async move {
                                        while let Some(msg_result) = read.next().await {
                                            match msg_result {
                                                Ok(Message::Text(text)) => {
                                                    let has_listeners = WS_CLIENT_LISTENERS.lock().unwrap()
                                                        .get(&ws_id_recv)
                                                        .map(|l| l.listeners.get("message").map(|v| !v.is_empty()).unwrap_or(false))
                                                        .unwrap_or(false);

                                                    if has_listeners {
                                                        WS_PENDING_EVENTS.lock().unwrap().push(
                                                            PendingWsEvent::Message(ws_id_recv, text)
                                                        );
                                                    } else {
                                                        if let Some(conn) = WS_CONNECTIONS.lock().unwrap().get_mut(&ws_id_recv) {
                                                            conn.messages.push(text);
                                                        }
                                                    }
                                                }
                                                Ok(Message::Binary(data)) => {
                                                    // Convert binary to string representation for now
                                                    let text = String::from_utf8_lossy(&data).to_string();
                                                    let has_listeners = WS_CLIENT_LISTENERS.lock().unwrap()
                                                        .get(&ws_id_recv)
                                                        .map(|l| l.listeners.get("message").map(|v| !v.is_empty()).unwrap_or(false))
                                                        .unwrap_or(false);

                                                    if has_listeners {
                                                        WS_PENDING_EVENTS.lock().unwrap().push(
                                                            PendingWsEvent::Message(ws_id_recv, text)
                                                        );
                                                    } else {
                                                        if let Some(conn) = WS_CONNECTIONS.lock().unwrap().get_mut(&ws_id_recv) {
                                                            conn.messages.push(text);
                                                        }
                                                    }
                                                }
                                                Ok(Message::Close(frame)) => {
                                                    let (code, reason) = frame
                                                        .map(|f| (f.code.into(), f.reason.to_string()))
                                                        .unwrap_or((1000u16, String::new()));

                                                    if let Some(conn) = WS_CONNECTIONS.lock().unwrap().get_mut(&ws_id_recv) {
                                                        conn.is_open = false;
                                                    }
                                                    WS_PENDING_EVENTS.lock().unwrap().push(
                                                        PendingWsEvent::Close(ws_id_recv, code, reason)
                                                    );
                                                    break;
                                                }
                                                Err(e) => {
                                                    if let Some(conn) = WS_CONNECTIONS.lock().unwrap().get_mut(&ws_id_recv) {
                                                        conn.is_open = false;
                                                    }
                                                    WS_PENDING_EVENTS.lock().unwrap().push(
                                                        PendingWsEvent::Error(ws_id_recv, format!("{}", e))
                                                    );
                                                    WS_PENDING_EVENTS.lock().unwrap().push(
                                                        PendingWsEvent::Close(ws_id_recv, 1006, String::new())
                                                    );
                                                    break;
                                                }
                                                _ => {}
                                            }
                                        }
                                    });
                                }
                                Err(e) => {
                                    WS_PENDING_EVENTS.lock().unwrap().push(
                                        PendingWsEvent::ServerError(handle_id, format!("WebSocket accept error: {}", e))
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            WS_PENDING_EVENTS.lock().unwrap().push(
                                PendingWsEvent::ServerError(handle_id, format!("TCP accept error: {}", e))
                            );
                        }
                    }
                }
                _ = shutdown_rx.recv() => {
                    // Shutdown signal received
                    break;
                }
            }
        }
    });

    server_handle
}

/// Close the WebSocketServer and all its client connections
/// wss.close(callback?) -> void
#[no_mangle]
pub unsafe extern "C" fn js_ws_server_close(handle: i64) {
    if let Some(server) = get_handle_mut::<WsServerHandle>(handle) {
        server.is_listening = false;

        // Send shutdown signal
        if let Some(tx) = server.shutdown_tx.take() {
            let _ = tx.send(());
        }

        // Close all client connections
        let client_ids: Vec<usize> = server.client_ids.clone();
        for ws_id in client_ids {
            let guard = WS_CONNECTIONS.lock().unwrap();
            if let Some(conn) = guard.get(&ws_id) {
                let _ = conn.sender.send(WsCommand::Close);
            }
        }
    }
}

/// Process pending WebSocket events (called from js_stdlib_process_pending)
/// Drains the event queue and invokes closures on the main thread.
/// Returns number of events processed.
#[no_mangle]
pub unsafe extern "C" fn js_ws_process_pending() -> i32 {
    let events: Vec<PendingWsEvent> = {
        let mut guard = WS_PENDING_EVENTS.lock().unwrap();
        guard.drain(..).collect()
    };

    let count = events.len() as i32;

    for event in events {
        match event {
            PendingWsEvent::Connection(server_handle, client_ws_id) => {
                // Get 'connection' listeners from server
                let listeners: Vec<i64> = get_handle_mut::<WsServerHandle>(server_handle)
                    .and_then(|s| s.listeners.get("connection").cloned())
                    .unwrap_or_default();

                // NaN-box the client handle with POINTER_TAG so it's recognized as an object
                let client_handle_f64 = f64::from_bits(
                    0x7FFD_0000_0000_0000u64 | (client_ws_id as u64 & 0x0000_FFFF_FFFF_FFFF)
                );

                for cb in listeners {
                    if cb != 0 {
                        let closure = cb as *const ClosureHeader;
                        js_closure_call1(closure, client_handle_f64);
                    }
                }
            }
            PendingWsEvent::Message(ws_id, message) => {
                // Get 'message' listeners from client
                let listeners: Vec<i64> = {
                    let guard = WS_CLIENT_LISTENERS.lock().unwrap();
                    guard.get(&ws_id)
                        .and_then(|l| l.listeners.get("message").cloned())
                        .unwrap_or_default()
                };

                // Create string on main thread and NaN-box with STRING_TAG
                let msg_str = js_string_from_bytes(message.as_ptr(), message.len() as u32);
                let msg_f64 = f64::from_bits(
                    0x7FFF_0000_0000_0000u64 | (msg_str as u64 & 0x0000_FFFF_FFFF_FFFF)
                );

                for cb in listeners {
                    if cb != 0 {
                        let closure = cb as *const ClosureHeader;
                        js_closure_call1(closure, msg_f64);
                    }
                }
            }
            PendingWsEvent::Close(ws_id, _code, _reason) => {
                let listeners: Vec<i64> = {
                    let guard = WS_CLIENT_LISTENERS.lock().unwrap();
                    guard.get(&ws_id)
                        .and_then(|l| l.listeners.get("close").cloned())
                        .unwrap_or_default()
                };

                for cb in listeners {
                    if cb != 0 {
                        let closure = cb as *const ClosureHeader;
                        js_closure_call0(closure);
                    }
                }
            }
            PendingWsEvent::Error(ws_id, error_msg) => {
                let listeners: Vec<i64> = {
                    let guard = WS_CLIENT_LISTENERS.lock().unwrap();
                    guard.get(&ws_id)
                        .and_then(|l| l.listeners.get("error").cloned())
                        .unwrap_or_default()
                };

                let err_str = js_string_from_bytes(error_msg.as_ptr(), error_msg.len() as u32);
                let err_f64 = f64::from_bits(
                    0x7FFF_0000_0000_0000u64 | (err_str as u64 & 0x0000_FFFF_FFFF_FFFF)
                );

                for cb in listeners {
                    if cb != 0 {
                        let closure = cb as *const ClosureHeader;
                        js_closure_call1(closure, err_f64);
                    }
                }
            }
            PendingWsEvent::ServerError(server_handle, error_msg) => {
                let listeners: Vec<i64> = get_handle_mut::<WsServerHandle>(server_handle)
                    .and_then(|s| s.listeners.get("error").cloned())
                    .unwrap_or_default();

                let err_str = js_string_from_bytes(error_msg.as_ptr(), error_msg.len() as u32);
                let err_f64 = f64::from_bits(
                    0x7FFF_0000_0000_0000u64 | (err_str as u64 & 0x0000_FFFF_FFFF_FFFF)
                );

                for cb in listeners {
                    if cb != 0 {
                        let closure = cb as *const ClosureHeader;
                        js_closure_call1(closure, err_f64);
                    }
                }
            }
            PendingWsEvent::Listening(server_handle) => {
                let listeners: Vec<i64> = get_handle_mut::<WsServerHandle>(server_handle)
                    .and_then(|s| s.listeners.get("listening").cloned())
                    .unwrap_or_default();

                for cb in listeners {
                    if cb != 0 {
                        let closure = cb as *const ClosureHeader;
                        js_closure_call0(closure);
                    }
                }
            }
        }
    }

    count
}
