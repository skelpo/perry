//! MySQL connection implementation

use std::time::Duration;

use perry_runtime::{js_promise_new, JSValue, Promise};
use sqlx::mysql::MySqlConnection;
use sqlx::Connection;

use crate::common::{register_handle, get_handle_mut, Handle};
use super::pool::MysqlPoolConnectionHandle;
use super::result::RawQueryResult;
use super::types::parse_mysql_config;

/// Default timeout for connecting to the database (in seconds)
const DEFAULT_CONNECT_TIMEOUT_SECS: u64 = 10;
/// Default timeout for overall query operation (in seconds)
const DEFAULT_QUERY_TIMEOUT_SECS: u64 = 30;

/// Wrapper around MySqlConnection that we can store in the handle registry
pub struct MysqlConnectionHandle {
    pub connection: Option<MySqlConnection>,
}

impl MysqlConnectionHandle {
    pub fn new(conn: MySqlConnection) -> Self {
        Self {
            connection: Some(conn),
        }
    }

    pub fn take(&mut self) -> Option<MySqlConnection> {
        self.connection.take()
    }
}

/// mysql.createConnection(config) -> Promise<Connection>
///
/// Creates a new MySQL connection with the given configuration.
/// Returns a Promise that resolves to a connection handle.
///
/// # Safety
/// The config parameter must be a valid JSValue representing a config object.
#[no_mangle]
pub unsafe extern "C" fn js_mysql2_create_connection(config: JSValue) -> *mut Promise {
    let promise = js_promise_new();

    // Parse the config
    let mysql_config = parse_mysql_config(config);

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        use tokio::time::timeout;

        let url = mysql_config.to_url();

        // Wrap connection in a timeout to prevent indefinite hangs
        match timeout(Duration::from_secs(DEFAULT_CONNECT_TIMEOUT_SECS), MySqlConnection::connect(&url)).await {
            Ok(Ok(conn)) => {
                let handle = register_handle(MysqlConnectionHandle::new(conn));
                // NaN-box the handle with POINTER_TAG so it can be properly extracted later
                let nanboxed = perry_runtime::js_nanbox_pointer(handle as i64);
                Ok(nanboxed.to_bits())
            }
            Ok(Err(e)) => Err(format!("Failed to connect: {}", e)),
            Err(_) => Err(format!(
                "Connection timed out after {} seconds (MySQL server may be unavailable)",
                DEFAULT_CONNECT_TIMEOUT_SECS
            )),
        }
    });

    promise
}

/// connection.end() -> Promise<void>
///
/// Closes the MySQL connection.
#[no_mangle]
pub unsafe extern "C" fn js_mysql2_connection_end(conn_handle: Handle) -> *mut Promise {
    let promise = js_promise_new();

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        use crate::common::take_handle;
        use tokio::time::timeout;

        if let Some(mut wrapper) = take_handle::<MysqlConnectionHandle>(conn_handle) {
            if let Some(conn) = wrapper.take() {
                match timeout(Duration::from_secs(DEFAULT_CONNECT_TIMEOUT_SECS), conn.close()).await {
                    Ok(Ok(())) => Ok(JSValue::undefined().bits()),
                    Ok(Err(e)) => Err(format!("Failed to close connection: {}", e)),
                    Err(_) => {
                        // Connection close timed out, but we've already taken it so just return
                        Ok(JSValue::undefined().bits())
                    }
                }
            } else {
                Err("Connection already closed".to_string())
            }
        } else {
            Err("Invalid connection handle".to_string())
        }
    });

    promise
}

/// connection.query(sql) -> Promise<[rows, fields]>
///
/// Executes a query and returns the results.
/// This function handles both regular connections (MysqlConnectionHandle)
/// and pool connections (MysqlPoolConnectionHandle).
#[no_mangle]
pub unsafe extern "C" fn js_mysql2_connection_query(
    conn_handle: Handle,
    sql_ptr: *const u8,
) -> *mut Promise {
    let promise = js_promise_new();

    // Extract the SQL string
    let sql = if sql_ptr.is_null() {
        String::new()
    } else {
        let header = sql_ptr as *const perry_runtime::StringHeader;
        let len = (*header).length as usize;
        let data_ptr = sql_ptr.add(std::mem::size_of::<perry_runtime::StringHeader>());
        let bytes = std::slice::from_raw_parts(data_ptr, len);
        String::from_utf8_lossy(bytes).to_string()
    };

    // Use spawn_for_promise_deferred to safely create JSValues on the main thread
    crate::common::spawn_for_promise_deferred(
        promise as *mut u8,
        async move {
            use tokio::time::timeout;

            // First try as a regular connection
            if let Some(wrapper) = get_handle_mut::<MysqlConnectionHandle>(conn_handle) {
                if let Some(conn) = wrapper.connection.as_mut() {
                    // Wrap the query in a timeout to prevent indefinite hangs
                    let query_future = sqlx::query(&sql).fetch_all(conn);
                    match timeout(Duration::from_secs(DEFAULT_QUERY_TIMEOUT_SECS), query_future).await {
                        Ok(Ok(rows)) => {
                            // Extract raw data on worker thread (no JSValue allocation)
                            let raw_result = RawQueryResult::from_mysql_rows(rows);
                            return Ok(raw_result);
                        }
                        Ok(Err(e)) => return Err(format!("Query failed: {}", e)),
                        Err(_) => return Err(format!(
                            "Query timed out after {} seconds (MySQL server may be unavailable)",
                            DEFAULT_QUERY_TIMEOUT_SECS
                        )),
                    }
                } else {
                    return Err("Connection already closed".to_string());
                }
            }

            // Then try as a pool connection
            if let Some(wrapper) = get_handle_mut::<MysqlPoolConnectionHandle>(conn_handle) {
                if let Some(ref mut conn) = wrapper.connection {
                    // Wrap the query in a timeout to prevent indefinite hangs
                    let query_future = sqlx::query(&sql).fetch_all(&mut **conn);
                    match timeout(Duration::from_secs(DEFAULT_QUERY_TIMEOUT_SECS), query_future).await {
                        Ok(Ok(rows)) => {
                            // Extract raw data on worker thread (no JSValue allocation)
                            let raw_result = RawQueryResult::from_mysql_rows(rows);
                            return Ok(raw_result);
                        }
                        Ok(Err(e)) => return Err(format!("Query failed: {}", e)),
                        Err(_) => return Err(format!(
                            "Query timed out after {} seconds (MySQL server may be unavailable)",
                            DEFAULT_QUERY_TIMEOUT_SECS
                        )),
                    }
                } else {
                    return Err("Connection has been released".to_string());
                }
            }

            Err("Invalid connection handle".to_string())
        },
        // Converter runs on main thread - safe to create JSValues here
        |raw_result: RawQueryResult| {
            raw_result.to_jsvalue().bits()
        },
    );

    promise
}

/// connection.execute(sql, params) -> Promise<[rows, fields]>
///
/// Executes a prepared statement with parameters.
#[no_mangle]
pub unsafe extern "C" fn js_mysql2_connection_execute(
    conn_handle: Handle,
    sql_ptr: *const u8,
    _params: JSValue, // TODO: Parse parameters array
) -> *mut Promise {
    // For now, just call query without params
    // TODO: Implement parameter binding
    js_mysql2_connection_query(conn_handle, sql_ptr)
}

/// connection.beginTransaction() -> Promise<void>
#[no_mangle]
pub unsafe extern "C" fn js_mysql2_connection_begin_transaction(conn_handle: Handle) -> *mut Promise {
    let promise = js_promise_new();

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        use crate::common::get_handle_mut;
        use tokio::time::timeout;

        if let Some(wrapper) = get_handle_mut::<MysqlConnectionHandle>(conn_handle) {
            if let Some(conn) = wrapper.connection.as_mut() {
                let query_future = sqlx::query("BEGIN").execute(conn);
                match timeout(Duration::from_secs(DEFAULT_QUERY_TIMEOUT_SECS), query_future).await {
                    Ok(Ok(_)) => Ok(JSValue::undefined().bits()),
                    Ok(Err(e)) => Err(format!("Failed to begin transaction: {}", e)),
                    Err(_) => Err(format!(
                        "Begin transaction timed out after {} seconds",
                        DEFAULT_QUERY_TIMEOUT_SECS
                    )),
                }
            } else {
                Err("Connection already closed".to_string())
            }
        } else {
            Err("Invalid connection handle".to_string())
        }
    });

    promise
}

/// connection.commit() -> Promise<void>
#[no_mangle]
pub unsafe extern "C" fn js_mysql2_connection_commit(conn_handle: Handle) -> *mut Promise {
    let promise = js_promise_new();

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        use crate::common::get_handle_mut;
        use tokio::time::timeout;

        if let Some(wrapper) = get_handle_mut::<MysqlConnectionHandle>(conn_handle) {
            if let Some(conn) = wrapper.connection.as_mut() {
                let query_future = sqlx::query("COMMIT").execute(conn);
                match timeout(Duration::from_secs(DEFAULT_QUERY_TIMEOUT_SECS), query_future).await {
                    Ok(Ok(_)) => Ok(JSValue::undefined().bits()),
                    Ok(Err(e)) => Err(format!("Failed to commit transaction: {}", e)),
                    Err(_) => Err(format!(
                        "Commit timed out after {} seconds",
                        DEFAULT_QUERY_TIMEOUT_SECS
                    )),
                }
            } else {
                Err("Connection already closed".to_string())
            }
        } else {
            Err("Invalid connection handle".to_string())
        }
    });

    promise
}

/// connection.rollback() -> Promise<void>
#[no_mangle]
pub unsafe extern "C" fn js_mysql2_connection_rollback(conn_handle: Handle) -> *mut Promise {
    let promise = js_promise_new();

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        use crate::common::get_handle_mut;
        use tokio::time::timeout;

        if let Some(wrapper) = get_handle_mut::<MysqlConnectionHandle>(conn_handle) {
            if let Some(conn) = wrapper.connection.as_mut() {
                let query_future = sqlx::query("ROLLBACK").execute(conn);
                match timeout(Duration::from_secs(DEFAULT_QUERY_TIMEOUT_SECS), query_future).await {
                    Ok(Ok(_)) => Ok(JSValue::undefined().bits()),
                    Ok(Err(e)) => Err(format!("Failed to rollback transaction: {}", e)),
                    Err(_) => Err(format!(
                        "Rollback timed out after {} seconds",
                        DEFAULT_QUERY_TIMEOUT_SECS
                    )),
                }
            } else {
                Err("Connection already closed".to_string())
            }
        } else {
            Err("Invalid connection handle".to_string())
        }
    });

    promise
}
