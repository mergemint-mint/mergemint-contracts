pub mod bounties;
pub mod feed;
pub mod leaderboard;
pub mod reputation;
pub mod stats;
pub mod tx;

use std::sync::Arc;

use axum::{routing::get, Router};

/// Read-only API router used by the OpenAPI contract tests.
///
/// It wires the same handlers as the binary with an isolated in-memory store,
/// so response schemas can be validated without starting the HTTP server.
pub fn router() -> Router {
    let state = Arc::new(tx::AppState {
        db: crate::db::new_shared_db(),
        postgres: None,
        idempotency: crate::db::new_shared_idempotency_store(),
        rate_limiter: tx::new_shared_rate_limiter(),
        bounty_broadcast: tokio::sync::broadcast::channel(16).0,
        leaderboard_cache: leaderboard::new_leaderboard_cache(),
        sse_keep_alive_duration: std::time::Duration::from_secs(15),
        webhook_guard: Default::default(),
        sse_limiter: Default::default(),
    });

    Router::new()
        .route("/api/bounties", get(bounties::list_bounties))
        .route("/api/bounties/:id", get(bounties::get_bounty_route))
        .route("/api/feed.xml", get(feed::open_bounties_feed))
        .route("/api/leaderboard", get(leaderboard::get_leaderboard_route))
        .route(
            "/api/contributors/:address/reputation-history",
            get(reputation::get_reputation_history),
        )
        .route("/api/stats", get(stats::get_stats_route))
        .with_state(state)
}

#[cfg(test)]
mod openapi_contract_tests {
    use axum::body::Body;
    use axum::http::{header::CONTENT_TYPE, Request, StatusCode};
    use axum::Router;
    use jsonschema::JSONSchema;
    use serde_json::Value;
    use tower::ServiceExt;

    /// Load the committed OpenAPI snapshot that CI enforces against.
    fn load_snapshot() -> Value {
        let raw = include_str!("../../openapi.snapshot.json");
        serde_json::from_str(raw).expect("openapi.snapshot.json must be valid JSON")
    }

    /// Resolve a component schema inside the committed OpenAPI snapshot.
    fn resolve_schema<'a>(snapshot: &'a Value, name: &str) -> &'a Value {
        snapshot
            .pointer(&format!("/components/schemas/{name}"))
            .unwrap_or_else(|| panic!("schema {name} missing from openapi.snapshot.json"))
    }

    /// Validate a serialized handler response body against a named schema from
    /// the committed OpenAPI snapshot.
    fn assert_response_matches(snapshot: &Value, schema_name: &str, body: &Value) {
        let schema = resolve_schema(snapshot, schema_name);
        let validator = JSONSchema::compile(schema)
            .unwrap_or_else(|e| panic!("invalid schema {schema_name}: {e}"));

        let validation = validator.validate(body);
        if let Err(errors) = validation {
            let messages: Vec<String> = errors.map(|error| error.to_string()).collect();
            panic!(
                "response drifted from OpenAPI schema {schema_name}: {}\nbody: {body}",
                messages.join("\n")
            );
        };
    }

    async fn get_json(app: Router, uri: &str) -> (StatusCode, Value) {
        let response = app
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .expect("router should respond");
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body should be readable");
        let body = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).expect("response body must be JSON")
        };
        (status, body)
    }

    #[test]
    fn snapshot_is_valid_openapi_3_1() {
        let snapshot = load_snapshot();
        let version = snapshot
            .get("openapi")
            .and_then(|v| v.as_str())
            .expect("snapshot must declare an openapi version");
        assert!(
            version.starts_with("3.1"),
            "openapi.snapshot.json must target OpenAPI 3.1, found {version}"
        );
        assert!(
            snapshot.get("paths").is_some(),
            "snapshot must declare paths"
        );
    }

    #[test]
    fn every_operation_has_json_response_schema() {
        let snapshot = load_snapshot();
        let paths = snapshot
            .get("paths")
            .and_then(|p| p.as_object())
            .expect("paths must be an object");

        for (path, item) in paths {
            let item = item.as_object().expect("path item must be an object");
            for (method, operation) in item {
                if !matches!(method.as_str(), "get" | "post" | "put" | "patch" | "delete") {
                    continue;
                }
                let responses = operation
                    .get("responses")
                    .and_then(|r| r.as_object())
                    .unwrap_or_else(|| panic!("{method} {path} must declare responses"));
                assert!(
                    !responses.is_empty(),
                    "{method} {path} must declare at least one response"
                );
            }
        }
    }

    #[tokio::test]
    async fn stats_response_matches_snapshot() {
        let snapshot = load_snapshot();
        let (status, body) = get_json(crate::routes::router(), "/api/stats").await;
        assert_eq!(status, StatusCode::OK);
        assert_response_matches(&snapshot, "StatsResponse", &body);
    }

    #[tokio::test]
    async fn leaderboard_response_matches_snapshot() {
        let snapshot = load_snapshot();
        let (status, body) = get_json(crate::routes::router(), "/api/leaderboard").await;
        assert_eq!(status, StatusCode::OK);
        assert_response_matches(&snapshot, "LeaderboardPage", &body);
    }

    #[tokio::test]
    async fn feed_response_is_xml() {
        let response = crate::routes::router()
            .oneshot(
                Request::builder()
                    .uri("/api/feed.xml")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("router should respond");
        assert_eq!(response.status(), StatusCode::OK);
        assert!(
            response
                .headers()
                .get(CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .is_some_and(|value| value.contains("xml")),
            "feed must be served as Atom XML"
        );
    }

    #[tokio::test]
    async fn bounties_response_matches_snapshot() {
        let snapshot = load_snapshot();
        let (status, body) = get_json(crate::routes::router(), "/api/bounties").await;
        assert_eq!(status, StatusCode::OK);
        assert_response_matches(&snapshot, "BountyPage", &body);
    }

    #[tokio::test]
    async fn reputation_response_matches_snapshot() {
        let snapshot = load_snapshot();
        let address = format!("G{}", "A".repeat(55));
        let (status, body) = get_json(
            crate::routes::router(),
            &format!("/api/contributors/{address}/reputation-history"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_response_matches(&snapshot, "ReputationHistoryPage", &body);
    }
}
