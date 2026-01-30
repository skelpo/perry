//! Query result handling for mysql2

use perry_runtime::{js_array_alloc, js_array_push, js_object_alloc, js_object_set_field, js_object_set_keys, js_string_from_bytes, JSValue};
use sqlx::mysql::{MySqlColumn, MySqlRow};

use super::types::{column_to_field_packet, row_to_js_object};

/// Convert query results to the mysql2 format: [rows, fields]
///
/// Returns a JSValue representing a 2-element array where:
/// - index 0: Array of row objects (RowDataPacket[])
/// - index 1: Array of field metadata objects (FieldPacket[])
pub fn rows_to_result_tuple(rows: Vec<MySqlRow>, columns: &[MySqlColumn]) -> JSValue {
    // Create the result tuple [rows, fields]
    let mut result_array = js_array_alloc(2);

    // Create rows array
    let mut rows_array = js_array_alloc(rows.len() as u32);
    for row in rows.iter() {
        let row_obj = row_to_js_object(row);
        rows_array = js_array_push(rows_array, JSValue::object_ptr(row_obj as *mut u8));
    }
    let rows_jsval = JSValue::array_ptr(rows_array);
    result_array = js_array_push(result_array, rows_jsval);

    // Create fields array
    let mut fields_array = js_array_alloc(columns.len() as u32);
    for col in columns.iter() {
        let field_obj = column_to_field_packet(col);
        fields_array = js_array_push(fields_array, JSValue::object_ptr(field_obj as *mut u8));
    }
    let fields_jsval = JSValue::array_ptr(fields_array);
    result_array = js_array_push(result_array, fields_jsval);

    JSValue::array_ptr(result_array)
}

/// Create an empty result (for queries that don't return rows, like INSERT/UPDATE)
pub fn empty_result() -> JSValue {
    let mut result_array = js_array_alloc(2);
    let empty_rows = js_array_alloc(0);
    let empty_fields = js_array_alloc(0);
    result_array = js_array_push(result_array, JSValue::array_ptr(empty_rows));
    result_array = js_array_push(result_array, JSValue::array_ptr(empty_fields));
    JSValue::array_ptr(result_array)
}

/// Create a result with affected rows info (for INSERT/UPDATE/DELETE)
///
/// mysql2 returns a ResultSetHeader for non-SELECT queries with:
/// - affectedRows
/// - insertId
/// - warningStatus
pub fn affected_rows_result(affected: u64, last_insert_id: u64) -> JSValue {
    // Create result tuple [header, fields]
    let mut result_array = js_array_alloc(2);

    // Create ResultSetHeader object
    let header = js_object_alloc(0, 3);

    // Create keys array for property name lookup
    let mut keys_array = js_array_alloc(3);

    // Set affectedRows (field index 0)
    js_object_set_field(header, 0, JSValue::number(affected as f64));
    let key0 = js_string_from_bytes("affectedRows".as_ptr(), 12);
    keys_array = js_array_push(keys_array, JSValue::string_ptr(key0));

    // Set insertId (field index 1)
    js_object_set_field(header, 1, JSValue::number(last_insert_id as f64));
    let key1 = js_string_from_bytes("insertId".as_ptr(), 8);
    keys_array = js_array_push(keys_array, JSValue::string_ptr(key1));

    // Set warningStatus (field index 2)
    js_object_set_field(header, 2, JSValue::number(0.0));
    let key2 = js_string_from_bytes("warningStatus".as_ptr(), 13);
    keys_array = js_array_push(keys_array, JSValue::string_ptr(key2));

    // Attach keys to header object
    js_object_set_keys(header, keys_array);

    result_array = js_array_push(result_array, JSValue::object_ptr(header as *mut u8));

    // Empty fields array
    let empty_fields = js_array_alloc(0);
    result_array = js_array_push(result_array, JSValue::array_ptr(empty_fields));

    JSValue::array_ptr(result_array)
}
