//! PostgreSQL connection pool implementation

use perry_runtime::{js_promise_new, JSValue, Promise};
use sqlx::postgres::{PgPool, PgPoolOptions};
use sqlx::Row;

use crate::common::{register_handle, Handle};
use super::result::rows_to_pg_result;
use super::types::parse_pg_config;

/// Wrapper around PgPool that we can store in the handle registry
pub struct PgPoolHandle {
    pub pool: Option<PgPool>,
}

impl PgPoolHandle {
    pub fn new(pool: PgPool) -> Self {
        Self { pool: Some(pool) }
    }
}

/// new Pool(config) -> Promise<Pool>
///
/// Creates a new PostgreSQL connection pool with the given configuration.
///
/// # Safety
/// The config parameter must be a valid JSValue representing a config object.
#[no_mangle]
pub unsafe extern "C" fn js_pg_create_pool(config: JSValue) -> *mut Promise {
    let promise = js_promise_new();

    // Parse the config
    let pg_config = parse_pg_config(config);

    // Extract max connections if provided (default to 10)
    let max_conns = 10u32;

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let url = pg_config.to_url();

        match PgPoolOptions::new()
            .max_connections(max_conns)
            .connect(&url)
            .await
        {
            Ok(pool) => {
                let handle = register_handle(PgPoolHandle::new(pool));
                Ok(handle as u64)
            }
            Err(e) => Err(format!("Failed to create pool: {}", e)),
        }
    });

    promise
}

/// pool.query(sql) -> Promise<Result>
///
/// Executes a query on the pool.
#[no_mangle]
pub unsafe extern "C" fn js_pg_pool_query(
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

    // Determine command type from SQL
    let command = sql.trim().split_whitespace().next()
        .unwrap_or("SELECT").to_uppercase();

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        use crate::common::get_handle;

        if let Some(wrapper) = get_handle::<PgPoolHandle>(pool_handle) {
            if let Some(pool) = wrapper.pool.as_ref() {
                match sqlx::query(&sql).fetch_all(pool).await {
                    Ok(rows) => {
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
                Err("Pool already closed".to_string())
            }
        } else {
            Err("Invalid pool handle".to_string())
        }
    });

    promise
}

/// pool.end() -> Promise<void>
///
/// Closes all connections in the pool.
#[no_mangle]
pub unsafe extern "C" fn js_pg_pool_end(pool_handle: Handle) -> *mut Promise {
    let promise = js_promise_new();

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        use crate::common::take_handle;

        if let Some(mut wrapper) = take_handle::<PgPoolHandle>(pool_handle) {
            if let Some(pool) = wrapper.pool.take() {
                pool.close().await;
                Ok(JSValue::undefined().bits())
            } else {
                Err("Pool already closed".to_string())
            }
        } else {
            Err("Invalid pool handle".to_string())
        }
    });

    promise
}
