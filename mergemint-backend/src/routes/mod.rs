pub mod bounties;
pub mod feed;
pub mod leaderboard;
pub mod reputation;
pub mod stats;
pub mod tx;

#[cfg(test)]
mod openapi_contract_tests {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::Router;
    use serde_json::Value;
    use tower::ServiceExt;

    /// Load the committed OpenAPI snapshot that CI enforces against.
    fn load_snapshot() -> Value {
        let raw = include_str!("../../openapi.snapshot.json");
        serde_json::from_str(raw).expect("openapi.snapshot.json must be valid JSON")
    }

    /// Resolve a `#/components/schemas/<Name>` reference inside the snapshot.
    fn resolve_schema<'a>(snapshot: &'a Value, reference: &str) -> &'a Value {
        let name = reference
            .strip_prefix("#/components/schemas/")
            .unwrap_or_else(|| panic!("unsupported schema reference: {reference}"));
        snapshot
            .get("components")
            .and_then(|c| c.get("schemas"))
            .and_then(|s| s.get(name))
            .unwrap_or_else(|| panic!("schema {name} missing from openapi.snapshot.json"))
    }

    /// Validate a serialized handler response body against the OpenAPI schema
    /// referenced by the given path/operation/response in the snapshot.
    fn assert_response_matches(
        snapshot: &Value,
        path: &str,
        method: &str,
        status: &str,
        body: &Value,
    ) {
        let operation = snapshot
            .get("paths")
            .and_then(|p| p.get(path))
            .and_then(|p| p.get(method))
            .unwrap_or_else(|| panic!("missing operation {method} {path} in snapshot"));

        let response = operation
            .get("responses")
            .and_then(|r| r.get(status))
            .unwrap_or_else(|| panic!("missing response {status} for {method} {path}"));

        let schema = response
            .get("content")
            .and_then(|c| c.get("application/json"))
            .and_then(|c| c.get("schema"))
            .unwrap_or_else(|| panic!("missing JSON schema for {method} {path} {status}"));

        let schema = match schema.get("$ref").and_then(|r| r.as_str()) {
            Some(reference) => resolve_schema(snapshot, reference),
            None => schema,
        };

        let validator = jsonschema::validator_for(schema)
            .unwrap_or_else(|e| panic!("invalid schema for {method} {path} {status}: {e}"));

        if let Err(error) = validator.validate(body) {
            panic!(
                "response for {method} {path} {status} drifted from OpenAPI schema: {error}\nbody: {body}"
            );
        }
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
                if !matches!(
                    method.as_str(),
                    "get" | "post" | "put" | "patch" | "delete"
                ) {
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
        let app = crate::routes::stats::router();
        let (status, body) = get_json(app, "/api/stats").await;
        assert_eq!(status, StatusCode::OK);
        assert_response_matches(&snapshot, "/api/stats", "get", "200", &body);
    }

    #[tokio::test]
    async fn leaderboard_response_matches_snapshot() {
        let snapshot = load_snapshot();
        let app = crate::routes::leaderboard::router();
        let (status, body) = get_json(app, "/api/leaderboard").await;
        assert_eq!(status, StatusCode::OK);
        assert_response_matches(&snapshot, "/api/leaderboard", "get", "200", &body);
    }

    #[tokio::test]
    async fn feed_response_matches_snapshot() {
        let snapshot = load_snapshot();
        let app = crate::routes::feed::router();
        let (status, body) = get_json(app, "/api/feed").await;
        assert_eq!(status, StatusCode::OK);
        assert_response_matches(&snapshot, "/api/feed", "get", "200", &body);
    }

    #[tokio::test]
    async fn bounties_response_matches_snapshot() {
        let snapshot = load_snapshot();
        let app = crate::routes::bounties::router();
        let (status, body) = get_json(app, "/api/bounties").await;
        assert_eq!(status, StatusCode::OK);
        assert_response_matches(&snapshot, "/api/bounties", "get", "200", &body);
    }

    #[tokio::test]
    async fn reputation_response_matches_snapshot() {
        let snapshot = load_snapshot();
        let app = crate::routes::reputation::router();
        let (status, body) = get_json(app, "/api/reputation").await;
        assert_eq!(status, StatusCode::OK);
        assert_response_matches(&snapshot, "/api/reputation", "get", "200", &body);
    }
}
