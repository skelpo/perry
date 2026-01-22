//! MySQL connection implementation

use perry_runtime::{js_promise_new, JSValue, Promise};
use sqlx::mysql::MySqlConnection;
use sqlx::{Connection, Row};

use crate::common::{register_handle, Handle};
use super::result::rows_to_result_tuple;
use super::types::{parse_mysql_config, MySqlConfig};

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
        let url = mysql_config.to_url();

        match MySqlConnection::connect(&url).await {
            Ok(conn) => {
                let handle = register_handle(MysqlConnectionHandle::new(conn));
                // Return the handle as bits
                Ok(handle as u64)
            }
            Err(e) => Err(format!("Failed to connect: {}", e)),
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

        if let Some(mut wrapper) = take_handle::<MysqlConnectionHandle>(conn_handle) {
            if let Some(conn) = wrapper.take() {
                match conn.close().await {
                    Ok(()) => Ok(JSValue::undefined().bits()),
                    Err(e) => Err(format!("Failed to close connection: {}", e)),
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

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        use crate::common::get_handle_mut;

        if let Some(wrapper) = get_handle_mut::<MysqlConnectionHandle>(conn_handle) {
            if let Some(conn) = wrapper.connection.as_mut() {
                match sqlx::query(&sql).fetch_all(conn).await {
                    Ok(rows) => {
                        // Get column info from first row (if any)
                        let columns: Vec<_> = if !rows.is_empty() {
                            rows[0].columns().to_vec()
                        } else {
                            Vec::new()
                        };

                        let result = rows_to_result_tuple(rows, &columns);
                        Ok(result.bits())
                    }
                    Err(e) => Err(format!("Query failed: {}", e)),
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

        if let Some(wrapper) = get_handle_mut::<MysqlConnectionHandle>(conn_handle) {
            if let Some(conn) = wrapper.connection.as_mut() {
                match sqlx::query("BEGIN").execute(conn).await {
                    Ok(_) => Ok(JSValue::undefined().bits()),
                    Err(e) => Err(format!("Failed to begin transaction: {}", e)),
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

        if let Some(wrapper) = get_handle_mut::<MysqlConnectionHandle>(conn_handle) {
            if let Some(conn) = wrapper.connection.as_mut() {
                match sqlx::query("COMMIT").execute(conn).await {
                    Ok(_) => Ok(JSValue::undefined().bits()),
                    Err(e) => Err(format!("Failed to commit transaction: {}", e)),
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

        if let Some(wrapper) = get_handle_mut::<MysqlConnectionHandle>(conn_handle) {
            if let Some(conn) = wrapper.connection.as_mut() {
                match sqlx::query("ROLLBACK").execute(conn).await {
                    Ok(_) => Ok(JSValue::undefined().bits()),
                    Err(e) => Err(format!("Failed to rollback transaction: {}", e)),
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
