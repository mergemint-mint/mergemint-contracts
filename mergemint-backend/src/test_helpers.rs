// mergemint-backend/src/test_helpers.rs
//
// Shared test utilities for mergemint-backend integration and unit tests.
//
// ## Why this module exists (#487)
//
// Issue #487 identified "zero test files anywhere in mergemint-backend/src" as
// the biggest gap in test coverage.  This module provides the foundational
// fixture helpers so that every future test can get a clean, isolated database
// handle with a single function call — no global state, no shared mutation
// between test threads.
//
// ## Usage
//
// ```rust
// #[cfg(test)]
// mod tests {
//     use crate::test_helpers::test_db;
//
//     #[test]
//     fn my_test() {
//         let db = test_db();
//         // `db` is a fresh SharedDb scoped to this test only.
//     }
// }
// ```
//
// This module is compiled only when `cfg(test)` is active.  It is never
// included in production binaries.

use crate::db::{new_shared_db, SharedDb};
use serde_json::Value;
use utoipa::openapi::OpenApi;

/// Validate a JSON response body against the matching OpenAPI response schema.
pub fn validate_openapi_response(
    openapi: &OpenApi,
    path: &str,
    status: u16,
    body: &Value,
) -> Result<(), String> {
    let document = serde_json::to_value(openapi).map_err(|error| error.to_string())?;
    let status = status.to_string();
    let path_pointer = path.replace('~', "~0").replace('/', "~1");
    let schema = document
        .pointer(&format!(
            "/paths/{path_pointer}/get/responses/{status}/content/application~1json/schema"
        ))
        .cloned()
        .ok_or_else(|| format!("no JSON schema documented for GET {path} status {status}"))?;
    let mut root = serde_json::json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "components": document["components"],
        "responseSchema": schema,
        "$ref": "#/responseSchema"
    });
    normalize_nullable_schemas(&mut root);
    let validator = jsonschema::JSONSchema::compile(&root)
        .map_err(|error| format!("invalid OpenAPI response schema for GET {path}: {error}"))?;
    // Bind the result so the borrowed error iterator is dropped before `validator`.
    let result = match validator.validate(body) {
        Ok(()) => Ok(()),
        Err(errors) => Err(errors
            .map(|error| format!("{}: {error}", error.instance_path))
            .collect::<Vec<_>>()
            .join("; ")),
    };
    result
}

fn normalize_nullable_schemas(schema: &mut Value) {
    match schema {
        Value::Array(items) => {
            for item in items {
                normalize_nullable_schemas(item);
            }
        }
        Value::Object(object) => {
            for value in object.values_mut() {
                normalize_nullable_schemas(value);
            }
            if object.remove("nullable") == Some(Value::Bool(true)) {
                let replacement = match object.remove("type") {
                    Some(Value::String(kind)) => {
                        object.insert("type".to_string(), serde_json::json!([kind, "null"]));
                        None
                    }
                    Some(Value::Array(mut kinds)) => {
                        if !kinds.iter().any(|kind| kind == "null") {
                            kinds.push(Value::String("null".to_string()));
                        }
                        object.insert("type".to_string(), Value::Array(kinds));
                        None
                    }
                    Some(kind) => {
                        object.insert("type".to_string(), kind);
                        Some(Value::Object(object.clone()))
                    }
                    None => Some(Value::Object(object.clone())),
                };
                if let Some(non_null) = replacement {
                    *schema = serde_json::json!({
                        "anyOf": [non_null, {"type": "null"}]
                    });
                }
            }
        }
        _ => {}
    }
}

/// Return a fresh, empty [`SharedDb`] suitable for use in a single test.
///
/// Each call produces an independent in-memory store, so tests cannot
/// accidentally share or corrupt each other's data even when run in parallel
/// with `cargo test`.
///
/// When the database uses a real on-disk backend (e.g. SQLite via a tempfile)
/// the fixture should create a [`tempfile::NamedTempFile`] and keep it alive
/// for the duration of the test.  The current `DbStore` is purely in-memory,
/// so no tempfile is needed yet — the signature is intentionally simple so
/// callers do not need updating when a file-backed store is introduced.
pub fn test_db() -> SharedDb {
    new_shared_db()
}

// ---------------------------------------------------------------------------
// Smoke tests — verify the harness itself compiles and behaves correctly.
// ---------------------------------------------------------------------------

#[test]
fn it_compiles() {
    // The simplest possible assertion: if this test runs, the harness compiled.
    let _ = test_db();
}

#[test]
fn test_db_starts_empty() {
    let db = test_db();
    let guard = db.read().expect("lock should not be poisoned");
    assert!(
        guard.records.is_empty(),
        "a freshly created test_db must contain no records"
    );
}

#[test]
fn test_db_instances_are_independent() {
    let db_a = test_db();
    let db_b = test_db();

    // Write into db_a.
    db_a.write()
        .unwrap()
        .records
        .insert("key".to_string(), "value_a".to_string());

    // db_b must be unaffected.
    assert!(
        db_b.read().unwrap().records.is_empty(),
        "db_b must not share state with db_a"
    );
}
