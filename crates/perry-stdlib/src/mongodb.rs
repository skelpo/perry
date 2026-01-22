//! MongoDB module
//!
//! Native implementation of the 'mongodb' npm package.
//! Provides MongoDB client functionality.

use bson::{doc, Document};
use perry_runtime::{
    js_object_alloc, js_object_set_field, js_promise_new, js_string_from_bytes,
    JSValue, ObjectHeader, Promise, StringHeader,
};
use mongodb::{Client, Collection, Database};
use crate::common::{get_handle, register_handle, spawn_for_promise, Handle};

/// Helper to extract string from StringHeader pointer
unsafe fn string_from_header(ptr: *const StringHeader) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    let len = (*ptr).length as usize;
    let data_ptr = (ptr as *const u8).add(std::mem::size_of::<StringHeader>());
    let bytes = std::slice::from_raw_parts(data_ptr, len);
    Some(String::from_utf8_lossy(bytes).to_string())
}

/// MongoDB client handle
pub struct MongoClientHandle {
    pub client: Client,
}

/// MongoDB database handle
pub struct MongoDatabaseHandle {
    pub db: Database,
}

/// MongoDB collection handle
pub struct MongoCollectionHandle {
    pub collection: Collection<Document>,
}

/// Convert BSON Document to JSValue object
#[allow(dead_code)]
unsafe fn bson_to_jsvalue(doc: &Document) -> *mut ObjectHeader {
    let field_count = doc.len() as u32;
    let obj = js_object_alloc(0, field_count);

    let mut idx = 0u32;
    for (_key, value) in doc.iter() {
        let js_val = match value {
            bson::Bson::Null => JSValue::null(),
            bson::Bson::Boolean(b) => JSValue::bool(*b),
            bson::Bson::Int32(n) => JSValue::int32(*n),
            bson::Bson::Int64(n) => JSValue::number(*n as f64),
            bson::Bson::Double(n) => JSValue::number(*n),
            bson::Bson::String(s) => {
                let ptr = js_string_from_bytes(s.as_ptr(), s.len() as u32);
                JSValue::string_ptr(ptr)
            }
            bson::Bson::ObjectId(oid) => {
                let s = oid.to_hex();
                let ptr = js_string_from_bytes(s.as_ptr(), s.len() as u32);
                JSValue::string_ptr(ptr)
            }
            bson::Bson::Document(nested) => {
                let nested_obj = bson_to_jsvalue(nested);
                JSValue::object_ptr(nested_obj as *mut u8)
            }
            bson::Bson::Array(arr) => {
                // Simplified array handling
                let arr_obj = js_object_alloc(0, arr.len() as u32);
                JSValue::object_ptr(arr_obj as *mut u8)
            }
            _ => JSValue::null(),
        };
        js_object_set_field(obj, idx, js_val);
        idx += 1;
    }

    obj
}

/// MongoClient.connect(uri) -> Promise<MongoClient>
#[no_mangle]
pub unsafe extern "C" fn js_mongodb_connect(uri_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();

    let uri = match string_from_header(uri_ptr) {
        Some(u) => u,
        None => {
            spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, _>("Invalid URI".to_string())
            });
            return promise;
        }
    };

    spawn_for_promise(promise as *mut u8, async move {
        let client = Client::with_uri_str(&uri).await
            .map_err(|e| format!("Failed to connect: {}", e))?;

        let handle = register_handle(MongoClientHandle { client });
        Ok(handle as u64)
    });

    promise
}

/// client.db(name) -> Database
#[no_mangle]
pub unsafe extern "C" fn js_mongodb_client_db(
    client_handle: Handle,
    name_ptr: *const StringHeader,
) -> Handle {
    let name = match string_from_header(name_ptr) {
        Some(n) => n,
        None => return -1,
    };

    if let Some(client_wrapper) = get_handle::<MongoClientHandle>(client_handle) {
        let db = client_wrapper.client.database(&name);
        register_handle(MongoDatabaseHandle { db })
    } else {
        -1
    }
}

/// db.collection(name) -> Collection
#[no_mangle]
pub unsafe extern "C" fn js_mongodb_db_collection(
    db_handle: Handle,
    name_ptr: *const StringHeader,
) -> Handle {
    let name = match string_from_header(name_ptr) {
        Some(n) => n,
        None => return -1,
    };

    if let Some(db_wrapper) = get_handle::<MongoDatabaseHandle>(db_handle) {
        let collection = db_wrapper.db.collection::<Document>(&name);
        register_handle(MongoCollectionHandle { collection })
    } else {
        -1
    }
}

/// collection.findOne(filter) -> Promise<Document | null>
#[no_mangle]
pub unsafe extern "C" fn js_mongodb_collection_find_one(
    collection_handle: Handle,
    filter_json_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();

    let filter_json = string_from_header(filter_json_ptr).unwrap_or_else(|| "{}".to_string());

    spawn_for_promise(promise as *mut u8, async move {
        if let Some(coll_wrapper) = get_handle::<MongoCollectionHandle>(collection_handle) {
            let filter: Document = serde_json::from_str(&filter_json)
                .unwrap_or_else(|_| doc! {});

            match coll_wrapper.collection.find_one(filter, None).await {
                Ok(Some(doc)) => {
                    let json = serde_json::to_string(&doc)
                        .unwrap_or_else(|_| "{}".to_string());
                    let ptr = js_string_from_bytes(json.as_ptr(), json.len() as u32);
                    Ok(JSValue::string_ptr(ptr).bits())
                }
                Ok(None) => Ok(JSValue::null().bits()),
                Err(e) => Err(format!("Find failed: {}", e)),
            }
        } else {
            Err("Invalid collection handle".to_string())
        }
    });

    promise
}

/// collection.find(filter) -> Promise<Document[]>
#[no_mangle]
pub unsafe extern "C" fn js_mongodb_collection_find(
    collection_handle: Handle,
    filter_json_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();

    let filter_json = string_from_header(filter_json_ptr).unwrap_or_else(|| "{}".to_string());

    spawn_for_promise(promise as *mut u8, async move {
        use futures_util::TryStreamExt;

        if let Some(coll_wrapper) = get_handle::<MongoCollectionHandle>(collection_handle) {
            let filter: Document = serde_json::from_str(&filter_json)
                .unwrap_or_else(|_| doc! {});

            match coll_wrapper.collection.find(filter, None).await {
                Ok(cursor) => {
                    let docs: Vec<Document> = cursor.try_collect().await
                        .map_err(|e| format!("Cursor error: {}", e))?;

                    let json = serde_json::to_string(&docs)
                        .unwrap_or_else(|_| "[]".to_string());
                    let ptr = js_string_from_bytes(json.as_ptr(), json.len() as u32);
                    Ok(JSValue::string_ptr(ptr).bits())
                }
                Err(e) => Err(format!("Find failed: {}", e)),
            }
        } else {
            Err("Invalid collection handle".to_string())
        }
    });

    promise
}

/// collection.insertOne(doc) -> Promise<InsertOneResult>
#[no_mangle]
pub unsafe extern "C" fn js_mongodb_collection_insert_one(
    collection_handle: Handle,
    doc_json_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();

    let doc_json = match string_from_header(doc_json_ptr) {
        Some(j) => j,
        None => {
            spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, _>("Invalid document".to_string())
            });
            return promise;
        }
    };

    spawn_for_promise(promise as *mut u8, async move {
        if let Some(coll_wrapper) = get_handle::<MongoCollectionHandle>(collection_handle) {
            let doc: Document = serde_json::from_str(&doc_json)
                .map_err(|e| format!("Invalid JSON: {}", e))?;

            match coll_wrapper.collection.insert_one(doc, None).await {
                Ok(result) => {
                    let id = result.inserted_id.to_string();
                    let ptr = js_string_from_bytes(id.as_ptr(), id.len() as u32);
                    Ok(JSValue::string_ptr(ptr).bits())
                }
                Err(e) => Err(format!("Insert failed: {}", e)),
            }
        } else {
            Err("Invalid collection handle".to_string())
        }
    });

    promise
}

/// collection.insertMany(docs) -> Promise<InsertManyResult>
#[no_mangle]
pub unsafe extern "C" fn js_mongodb_collection_insert_many(
    collection_handle: Handle,
    docs_json_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();

    let docs_json = match string_from_header(docs_json_ptr) {
        Some(j) => j,
        None => {
            spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, _>("Invalid documents".to_string())
            });
            return promise;
        }
    };

    spawn_for_promise(promise as *mut u8, async move {
        if let Some(coll_wrapper) = get_handle::<MongoCollectionHandle>(collection_handle) {
            let docs: Vec<Document> = serde_json::from_str(&docs_json)
                .map_err(|e| format!("Invalid JSON: {}", e))?;

            match coll_wrapper.collection.insert_many(docs, None).await {
                Ok(result) => {
                    let count = result.inserted_ids.len();
                    Ok(count as u64)
                }
                Err(e) => Err(format!("Insert failed: {}", e)),
            }
        } else {
            Err("Invalid collection handle".to_string())
        }
    });

    promise
}

/// collection.updateOne(filter, update) -> Promise<UpdateResult>
#[no_mangle]
pub unsafe extern "C" fn js_mongodb_collection_update_one(
    collection_handle: Handle,
    filter_json_ptr: *const StringHeader,
    update_json_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();

    let filter_json = string_from_header(filter_json_ptr).unwrap_or_else(|| "{}".to_string());
    let update_json = match string_from_header(update_json_ptr) {
        Some(j) => j,
        None => {
            spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, _>("Invalid update".to_string())
            });
            return promise;
        }
    };

    spawn_for_promise(promise as *mut u8, async move {
        if let Some(coll_wrapper) = get_handle::<MongoCollectionHandle>(collection_handle) {
            let filter: Document = serde_json::from_str(&filter_json)
                .unwrap_or_else(|_| doc! {});
            let update: Document = serde_json::from_str(&update_json)
                .map_err(|e| format!("Invalid update JSON: {}", e))?;

            match coll_wrapper.collection.update_one(filter, update, None).await {
                Ok(result) => {
                    Ok(result.modified_count as u64)
                }
                Err(e) => Err(format!("Update failed: {}", e)),
            }
        } else {
            Err("Invalid collection handle".to_string())
        }
    });

    promise
}

/// collection.updateMany(filter, update) -> Promise<UpdateResult>
#[no_mangle]
pub unsafe extern "C" fn js_mongodb_collection_update_many(
    collection_handle: Handle,
    filter_json_ptr: *const StringHeader,
    update_json_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();

    let filter_json = string_from_header(filter_json_ptr).unwrap_or_else(|| "{}".to_string());
    let update_json = match string_from_header(update_json_ptr) {
        Some(j) => j,
        None => {
            spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, _>("Invalid update".to_string())
            });
            return promise;
        }
    };

    spawn_for_promise(promise as *mut u8, async move {
        if let Some(coll_wrapper) = get_handle::<MongoCollectionHandle>(collection_handle) {
            let filter: Document = serde_json::from_str(&filter_json)
                .unwrap_or_else(|_| doc! {});
            let update: Document = serde_json::from_str(&update_json)
                .map_err(|e| format!("Invalid update JSON: {}", e))?;

            match coll_wrapper.collection.update_many(filter, update, None).await {
                Ok(result) => {
                    Ok(result.modified_count as u64)
                }
                Err(e) => Err(format!("Update failed: {}", e)),
            }
        } else {
            Err("Invalid collection handle".to_string())
        }
    });

    promise
}

/// collection.deleteOne(filter) -> Promise<DeleteResult>
#[no_mangle]
pub unsafe extern "C" fn js_mongodb_collection_delete_one(
    collection_handle: Handle,
    filter_json_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();

    let filter_json = string_from_header(filter_json_ptr).unwrap_or_else(|| "{}".to_string());

    spawn_for_promise(promise as *mut u8, async move {
        if let Some(coll_wrapper) = get_handle::<MongoCollectionHandle>(collection_handle) {
            let filter: Document = serde_json::from_str(&filter_json)
                .unwrap_or_else(|_| doc! {});

            match coll_wrapper.collection.delete_one(filter, None).await {
                Ok(result) => {
                    Ok(result.deleted_count as u64)
                }
                Err(e) => Err(format!("Delete failed: {}", e)),
            }
        } else {
            Err("Invalid collection handle".to_string())
        }
    });

    promise
}

/// collection.deleteMany(filter) -> Promise<DeleteResult>
#[no_mangle]
pub unsafe extern "C" fn js_mongodb_collection_delete_many(
    collection_handle: Handle,
    filter_json_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();

    let filter_json = string_from_header(filter_json_ptr).unwrap_or_else(|| "{}".to_string());

    spawn_for_promise(promise as *mut u8, async move {
        if let Some(coll_wrapper) = get_handle::<MongoCollectionHandle>(collection_handle) {
            let filter: Document = serde_json::from_str(&filter_json)
                .unwrap_or_else(|_| doc! {});

            match coll_wrapper.collection.delete_many(filter, None).await {
                Ok(result) => {
                    Ok(result.deleted_count as u64)
                }
                Err(e) => Err(format!("Delete failed: {}", e)),
            }
        } else {
            Err("Invalid collection handle".to_string())
        }
    });

    promise
}

/// collection.countDocuments(filter) -> Promise<number>
#[no_mangle]
pub unsafe extern "C" fn js_mongodb_collection_count(
    collection_handle: Handle,
    filter_json_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();

    let filter_json = string_from_header(filter_json_ptr).unwrap_or_else(|| "{}".to_string());

    spawn_for_promise(promise as *mut u8, async move {
        if let Some(coll_wrapper) = get_handle::<MongoCollectionHandle>(collection_handle) {
            let filter: Document = serde_json::from_str(&filter_json)
                .unwrap_or_else(|_| doc! {});

            match coll_wrapper.collection.count_documents(filter, None).await {
                Ok(count) => {
                    Ok(count as u64)
                }
                Err(e) => Err(format!("Count failed: {}", e)),
            }
        } else {
            Err("Invalid collection handle".to_string())
        }
    });

    promise
}

/// client.close() -> Promise<void>
#[no_mangle]
pub unsafe extern "C" fn js_mongodb_client_close(_client_handle: Handle) -> *mut Promise {
    let promise = js_promise_new();

    spawn_for_promise(promise as *mut u8, async move {
        // MongoDB client doesn't need explicit close in Rust driver
        // The connection pool is managed automatically
        Ok(JSValue::undefined().bits())
    });

    promise
}
