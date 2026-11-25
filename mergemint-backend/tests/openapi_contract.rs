//! OpenAPI contract-to-backend integration tests.
//!
//! These tests validate that the live Axum router handlers produce responses
//! that conform to the schemas declared in the generated OpenAPI document, and
//! that the committed `openapi.snapshot.json` has not drifted from the document
//! generated at build time.
//!
//! If a route signature or schema changes, regenerate the snapshot and record
//! the change in `mergemint-backend/CHANGELOG.md`; otherwise these tests fail.

use axum::body::Body;
use axum::http::{header::CONTENT_TYPE, Request, StatusCode};
use jsonschema::JSONSchema;
use serde_json::Value;
use tower::ServiceExt;

use mergemint_backend::openapi::ApiDoc;
use mergemint_backend::routes::router;
use utoipa::OpenApi;

/// Path to the committed snapshot, relative to the crate root.
const SNAPSHOT_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/openapi.snapshot.json");

fn generated_document() -> Value {
    serde_json::to_value(ApiDoc::openapi()).expect("OpenAPI document must serialize to JSON")
}

fn snapshot_document() -> Value {
    let raw = std::fs::read_to_string(SNAPSHOT_PATH)
        .expect("openapi.snapshot.json must exist; run the snapshot generator");
    serde_json::from_str(&raw).expect("openapi.snapshot.json must be valid JSON")
}

/// Compile the JSON schema for a named component under `#/components/schemas`.
fn component_schema(doc: &Value, name: &str) -> JSONSchema {
    let schema = doc
        .pointer(&format!("/components/schemas/{name}"))
        .unwrap_or_else(|| panic!("missing component schema `{name}` in OpenAPI document"));
    JSONSchema::compile(schema).expect("component schema must compile")
}

/// Assert that a serialized response body validates against a component schema.
fn assert_validates(doc: &Value, name: &str, body: &Value) {
    let compiled = component_schema(doc, name);
    if let Err(errors) = compiled.validate(body) {
        let messages: Vec<String> = errors.map(|e| e.to_string()).collect();
        panic!(
            "response body does not match schema `{name}`:\n{}\nbody: {}",
            messages.join("\n"),
            serde_json::to_string_pretty(body).unwrap_or_default()
        );
    };
}

async fn get_json(path: &str) -> (StatusCode, Value) {
    let response = router()
        .oneshot(
            Request::builder()
                .uri(path)
                .method("GET")
                .body(Body::empty())
                .expect("request must build"),
        )
        .await
        .expect("router must respond");

    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body must be readable");
    let body = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).expect("response body must be valid JSON")
    };
    (status, body)
}

#[test]
fn snapshot_paths_match_generated_document() {
    let generated = generated_document();
    let snapshot = snapshot_document();

    let documented: std::collections::BTreeSet<String> = generated
        .get("paths")
        .and_then(Value::as_object)
        .expect("generated document must declare paths")
        .keys()
        .cloned()
        .collect();
    let recorded: std::collections::BTreeSet<String> = snapshot
        .get("paths")
        .and_then(Value::as_object)
        .expect("snapshot must declare paths")
        .keys()
        .cloned()
        .collect();

    assert_eq!(
        documented, recorded,
        "openapi.snapshot.json paths are out of date. Regenerate the snapshot and update \
         mergemint-backend/CHANGELOG.md to describe the contract change."
    );
}

#[test]
fn generated_document_is_openapi_3_1() {
    let generated = generated_document();
    let version = generated
        .get("openapi")
        .and_then(Value::as_str)
        .expect("generated document must declare an `openapi` version");
    assert!(
        version.starts_with("3.1"),
        "expected OpenAPI 3.1.x contract, found `{version}`"
    );
}

#[tokio::test]
async fn stats_response_matches_schema() {
    let doc = generated_document();
    let (status, body) = get_json("/api/stats").await;
    assert_eq!(status, StatusCode::OK, "stats handler must return 200");
    assert_validates(&doc, "StatsResponse", &body);
}

#[tokio::test]
async fn leaderboard_response_matches_schema() {
    let doc = generated_document();
    let (status, body) = get_json("/api/leaderboard").await;
    assert_eq!(
        status,
        StatusCode::OK,
        "leaderboard handler must return 200"
    );
    assert_validates(&doc, "LeaderboardPage", &body);
}

#[tokio::test]
async fn feed_response_is_xml() {
    let response = router()
        .oneshot(
            Request::builder()
                .uri("/api/feed.xml")
                .method("GET")
                .body(Body::empty())
                .expect("request must build"),
        )
        .await
        .expect("router must respond");
    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.contains("xml")),
        "feed handler must serve Atom XML"
    );
}

#[tokio::test]
async fn bounties_response_matches_schema() {
    let doc = generated_document();
    let (status, body) = get_json("/api/bounties").await;
    assert_eq!(status, StatusCode::OK, "bounties handler must return 200");
    assert_validates(&doc, "BountyPage", &body);
}

#[tokio::test]
async fn reputation_response_matches_schema() {
    let doc = generated_document();
    let address = format!("G{}", "A".repeat(55));
    let (status, body) = get_json(&format!("/api/contributors/{address}/reputation-history")).await;
    assert_eq!(status, StatusCode::OK, "reputation handler must return 200");
    assert_validates(&doc, "ReputationHistoryPage", &body);
}

#[tokio::test]
async fn error_response_matches_schema() {
    let doc = generated_document();
    let (status, body) = get_json("/api/bounties/not-a-valid-id").await;
    assert!(
        status.is_client_error(),
        "invalid bounty id must yield a 4xx error, got {status}"
    );
    assert_validates(&doc, "AppError", &body);
}
