//! MySQL connection pool implementation

use perry_runtime::{js_promise_new, JSValue, Promise};
use sqlx::mysql::{MySqlPool, MySqlPoolOptions};
use sqlx::Row;

use crate::common::{register_handle, Handle};
use super::result::rows_to_result_tuple;
use super::types::parse_mysql_config;

/// Wrapper around MySqlPool
pub struct MysqlPoolHandle {
    pub pool: MySqlPool,
}

impl MysqlPoolHandle {
    pub fn new(pool: MySqlPool) -> Self {
        Self { pool }
    }
}

/// mysql.createPool(config) -> Pool
///
/// Creates a new connection pool. The pool connects lazily, so this
/// returns synchronously.
///
/// # Safety
/// The config parameter must be a valid JSValue representing a config object.
#[no_mangle]
pub unsafe extern "C" fn js_mysql2_create_pool(config: JSValue) -> Handle {
    let mysql_config = parse_mysql_config(config);
    let url = mysql_config.to_url();

    // Create pool with lazy connection using the tokio runtime context
    // We need to enter the runtime context for connect_lazy to work
    let _guard = crate::common::runtime().enter();

    let pool = MySqlPoolOptions::new()
        .max_connections(10)
        .connect_lazy(&url);

    match pool {
        Ok(pool) => register_handle(MysqlPoolHandle::new(pool)),
        Err(_) => 0, // Return invalid handle on error
    }
}

/// pool.end() -> Promise<void>
///
/// Closes all connections in the pool.
#[no_mangle]
pub unsafe extern "C" fn js_mysql2_pool_end(pool_handle: Handle) -> *mut Promise {
    let promise = js_promise_new();

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        use crate::common::take_handle;

        if let Some(wrapper) = take_handle::<MysqlPoolHandle>(pool_handle) {
            wrapper.pool.close().await;
            Ok(JSValue::undefined().bits())
        } else {
            Err("Invalid pool handle".to_string())
        }
    });

    promise
}

/// pool.query(sql) -> Promise<[rows, fields]>
///
/// Executes a query using a connection from the pool.
#[no_mangle]
pub unsafe extern "C" fn js_mysql2_pool_query(
    pool_handle: Handle,
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
        use crate::common::get_handle;

        if let Some(wrapper) = get_handle::<MysqlPoolHandle>(pool_handle) {
            match sqlx::query(&sql).fetch_all(&wrapper.pool).await {
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
            Err("Invalid pool handle".to_string())
        }
    });

    promise
}

/// pool.execute(sql, params) -> Promise<[rows, fields]>
///
/// Executes a prepared statement with parameters using a connection from the pool.
#[no_mangle]
pub unsafe extern "C" fn js_mysql2_pool_execute(
    pool_handle: Handle,
    sql_ptr: *const u8,
    _params: JSValue, // TODO: Parse parameters array
) -> *mut Promise {
    // For now, just call query without params
    // TODO: Implement parameter binding
    js_mysql2_pool_query(pool_handle, sql_ptr)
}

/// pool.getConnection() -> Promise<PoolConnection>
///
/// Gets a connection from the pool.
#[no_mangle]
pub unsafe extern "C" fn js_mysql2_pool_get_connection(pool_handle: Handle) -> *mut Promise {
    let promise = js_promise_new();

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        use crate::common::get_handle;

        if let Some(_wrapper) = get_handle::<MysqlPoolHandle>(pool_handle) {
            // TODO: Implement proper PoolConnection with release()
            // For now, return an error since we can't properly implement this
            // without more infrastructure
            Err("pool.getConnection() not yet implemented - use pool.query() instead".to_string())
        } else {
            Err("Invalid pool handle".to_string())
        }
    });

    promise
}

/// poolConnection.release()
///
/// Returns a connection to the pool.
#[no_mangle]
pub unsafe extern "C" fn js_mysql2_pool_connection_release(_conn_handle: Handle) {
    // TODO: Implement when PoolConnection is implemented
    // For now, this is a no-op
}
