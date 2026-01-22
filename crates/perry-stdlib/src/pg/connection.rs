//! PostgreSQL connection implementation

use perry_runtime::{js_promise_new, JSValue, Promise};
use sqlx::postgres::PgConnection;
use sqlx::{Connection, Row};

use crate::common::{register_handle, Handle};
use super::result::rows_to_pg_result;
use super::types::{parse_pg_config, PgConfig};

/// Wrapper around PgConnection that we can store in the handle registry
pub struct PgConnectionHandle {
    pub connection: Option<PgConnection>,
}

impl PgConnectionHandle {
    pub fn new(conn: PgConnection) -> Self {
        Self {
            connection: Some(conn),
        }
    }

    pub fn take(&mut self) -> Option<PgConnection> {
        self.connection.take()
    }
}

/// pg.connect(config) -> Promise<Client>
///
/// Creates a new PostgreSQL connection with the given configuration.
/// Returns a Promise that resolves to a client handle.
///
/// # Safety
/// The config parameter must be a valid JSValue representing a config object.
#[no_mangle]
pub unsafe extern "C" fn js_pg_connect(config: JSValue) -> *mut Promise {
    let promise = js_promise_new();

    // Parse the config
    let pg_config = parse_pg_config(config);

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let url = pg_config.to_url();

        match PgConnection::connect(&url).await {
            Ok(conn) => {
                let handle = register_handle(PgConnectionHandle::new(conn));
                // Return the handle as bits
                Ok(handle as u64)
            }
            Err(e) => Err(format!("Failed to connect: {}", e)),
        }
    });

    promise
}

/// client.end() -> Promise<void>
///
/// Closes the PostgreSQL connection.
#[no_mangle]
pub unsafe extern "C" fn js_pg_client_end(client_handle: Handle) -> *mut Promise {
    let promise = js_promise_new();

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        use crate::common::take_handle;

        if let Some(mut wrapper) = take_handle::<PgConnectionHandle>(client_handle) {
            if let Some(conn) = wrapper.take() {
                match conn.close().await {
                    Ok(()) => Ok(JSValue::undefined().bits()),
                    Err(e) => Err(format!("Failed to close connection: {}", e)),
                }
            } else {
                Err("Connection already closed".to_string())
            }
        } else {
            Err("Invalid client handle".to_string())
        }
    });

    promise
}

/// client.query(sql) -> Promise<Result>
///
/// Executes a query and returns the results.
#[no_mangle]
pub unsafe extern "C" fn js_pg_client_query(
    client_handle: Handle,
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

    // Determine command type from SQL
    let command = sql.trim().split_whitespace().next()
        .unwrap_or("SELECT").to_uppercase();

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        use crate::common::get_handle_mut;

        if let Some(wrapper) = get_handle_mut::<PgConnectionHandle>(client_handle) {
            if let Some(conn) = wrapper.connection.as_mut() {
                match sqlx::query(&sql).fetch_all(conn).await {
                    Ok(rows) => {
                        // Get column info from first row (if any)
                        let columns: Vec<_> = if !rows.is_empty() {
                            rows[0].columns().to_vec()
                        } else {
                            Vec::new()
                        };

                        let result = rows_to_pg_result(rows, &columns, &command);
                        Ok(result.bits())
                    }
                    Err(e) => Err(format!("Query failed: {}", e)),
                }
            } else {
                Err("Connection already closed".to_string())
            }
        } else {
            Err("Invalid client handle".to_string())
        }
    });

    promise
}

/// client.query(sql, params) -> Promise<Result>
///
/// Executes a parameterized query.
#[no_mangle]
pub unsafe extern "C" fn js_pg_client_query_params(
    client_handle: Handle,
    sql_ptr: *const u8,
    _params: JSValue, // TODO: Parse parameters array
) -> *mut Promise {
    // For now, just call query without params
    // TODO: Implement parameter binding (PostgreSQL uses $1, $2, etc.)
    js_pg_client_query(client_handle, sql_ptr)
}
