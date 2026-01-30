//! MySQL connection pool implementation

use perry_runtime::{js_array_get_jsvalue, js_array_length, js_promise_new, JSValue, Promise};
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
    params: JSValue,
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

    // Extract parameters from the JSValue array
    let param_values = extract_params_from_jsvalue(params);

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        use crate::common::get_handle;

        if let Some(wrapper) = get_handle::<MysqlPoolHandle>(pool_handle) {
            // Build the query with parameter bindings
            let mut query = sqlx::query(&sql);

            for param in &param_values {
                query = match param {
                    ParamValue::Null => query.bind(Option::<String>::None),
                    ParamValue::String(s) => query.bind(s.clone()),
                    ParamValue::Number(n) => query.bind(*n),
                    ParamValue::Int(i) => query.bind(*i),
                    ParamValue::Bool(b) => query.bind(*b),
                };
            }

            match query.fetch_all(&wrapper.pool).await {
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

/// Enum to hold different parameter value types
#[derive(Clone, Debug)]
enum ParamValue {
    Null,
    String(String),
    Number(f64),
    Int(i64),
    Bool(bool),
}

/// Extract parameter values from a JSValue array
unsafe fn extract_params_from_jsvalue(params: JSValue) -> Vec<ParamValue> {
    let mut result = Vec::new();

    let bits = params.bits();

    // Handle both NaN-boxed pointers and raw pointers
    let arr_ptr: *const perry_runtime::ArrayHeader = if params.is_pointer() {
        // NaN-boxed pointer (POINTER_TAG = 0x7FFD)
        params.as_pointer() as *const perry_runtime::ArrayHeader
    } else if bits != 0 && bits <= 0x0000_FFFF_FFFF_FFFF {
        // Raw pointer (not NaN-boxed) - the bits ARE the pointer
        // Check upper bits don't match any NaN-box tag (0x7FFC-0x7FFF)
        let upper = bits >> 48;
        if upper == 0 || (upper > 0 && upper < 0x7FF0) {
            bits as *const perry_runtime::ArrayHeader
        } else {
            return result;
        }
    } else {
        return result;
    };

    if arr_ptr.is_null() {
        return result;
    }

    let length = js_array_length(arr_ptr);

    for i in 0..length {
        let element_bits = js_array_get_jsvalue(arr_ptr, i);
        let element = JSValue::from_bits(element_bits);

        let param = if element.is_null() || element.is_undefined() {
            ParamValue::Null
        } else if element.is_string() {
            // Extract string value
            let str_ptr = element.as_string_ptr();
            if !str_ptr.is_null() {
                let len = (*str_ptr).length as usize;
                let data_ptr = (str_ptr as *const u8).add(std::mem::size_of::<perry_runtime::StringHeader>());
                let bytes = std::slice::from_raw_parts(data_ptr, len);
                ParamValue::String(String::from_utf8_lossy(bytes).to_string())
            } else {
                ParamValue::Null
            }
        } else if element.is_int32() {
            ParamValue::Int(element.as_int32() as i64)
        } else if element.is_bool() {
            ParamValue::Bool(element.as_bool())
        } else if element.is_number() {
            ParamValue::Number(element.to_number())
        } else {
            // Unknown type - try to treat as number
            ParamValue::Number(element.to_number())
        };

        result.push(param);
    }

    result
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
