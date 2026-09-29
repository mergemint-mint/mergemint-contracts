use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

use crate::db::{
    add_webhook_subscription, delete_webhook_subscription, get_bounty, get_webhook_subscription,
    list_webhook_subscriptions, SharedDb, WebhookSubscription,
};
use crate::routes::tx::AppState;

type HmacSha256 = Hmac<Sha256>;

/// Delivery payload delivered to webhook subscriber endpoints.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WebhookPayload {
    pub id: String,
    pub event: String,
    pub timestamp: DateTime<Utc>,
    pub bounty_id: String,
    pub data: serde_json::Value,
}

/// Request body for registering a new webhook subscription.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSubscriptionRequest {
    pub url: String,
    pub secret: String,
    #[serde(default)]
    pub event_types: Vec<String>,
}

/// Configuration options for exponential backoff webhook delivery retries.
#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_attempts: usize,
    pub initial_delay: Duration,
    pub backoff_factor: f64,
    pub max_delay: Duration,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay: Duration::from_millis(200),
            backoff_factor: 2.0,
            max_delay: Duration::from_secs(30),
        }
    }
}

/// Errors that can occur during webhook delivery and retry attempts.
#[derive(Debug)]
pub enum WebhookDeliveryError {
    SerializationError(String),
    ClientError(StatusCode, String),
    MaxRetriesExceeded(usize, String),
}

impl std::fmt::Display for WebhookDeliveryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SerializationError(msg) => write!(f, "Payload serialization error: {msg}"),
            Self::ClientError(status, msg) => {
                write!(f, "Non-retryable client error {status}: {msg}")
            }
            Self::MaxRetriesExceeded(attempts, msg) => {
                write!(f, "Delivery failed after {attempts} attempts: {msg}")
            }
        }
    }
}

impl std::error::Error for WebhookDeliveryError {}

/// Compute the HMAC-SHA256 hex digest for a secret and raw payload bytes.
pub fn compute_hmac_sha256(secret: &str, payload: &[u8]) -> String {
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC can accept key of any size");
    mac.update(payload);
    hex::encode(mac.finalize().into_bytes())
}

/// Verify that an incoming HMAC-SHA256 signature matches the payload and secret using constant-time comparison.
#[allow(dead_code)]
pub fn verify_hmac_sha256(secret: &str, payload: &[u8], signature_hex: &str) -> bool {
    let Ok(expected_bytes) = hex::decode(signature_hex) else {
        return false;
    };
    let Ok(mut mac) = HmacSha256::new_from_slice(secret.as_bytes()) else {
        return false;
    };
    mac.update(payload);
    mac.verify_slice(&expected_bytes).is_ok()
}

/// Determine whether a subscription is eligible to receive notifications for a specific event type.
pub fn subscription_matches_event(subscription: &WebhookSubscription, event_name: &str) -> bool {
    if !subscription.active {
        return false;
    }
    if subscription.event_types.is_empty() {
        return true;
    }
    subscription
        .event_types
        .iter()
        .any(|pattern| pattern == "*" || pattern == event_name)
}

/// Calculate the delay duration for a specific retry attempt under exponential backoff.
pub fn calculate_backoff_delay(attempt: usize, config: &RetryConfig) -> Duration {
    if attempt <= 1 {
        return Duration::ZERO;
    }
    let exponent = (attempt - 1) as i32;
    let multiplier = config.backoff_factor.powi(exponent);
    let calculated_millis = config.initial_delay.as_millis() as f64 * multiplier;
    let max_millis = config.max_delay.as_millis() as f64;
    Duration::from_millis(calculated_millis.min(max_millis) as u64)
}

/// Deliver a webhook payload to a single subscription with exponential backoff retries.
pub async fn deliver_webhook_payload(
    client: &reqwest::Client,
    subscription: &WebhookSubscription,
    payload: &WebhookPayload,
    retry_config: &RetryConfig,
) -> Result<(), WebhookDeliveryError> {
    let payload_bytes = serde_json::to_vec(payload)
        .map_err(|e| WebhookDeliveryError::SerializationError(e.to_string()))?;
    let signature_hex = compute_hmac_sha256(&subscription.secret, &payload_bytes);
    let signature_header = format!("sha256={signature_hex}");

    let mut last_error_message = String::new();

    for attempt in 1..=retry_config.max_attempts {
        if attempt > 1 {
            let delay = calculate_backoff_delay(attempt, retry_config);
            tokio::time::sleep(delay).await;
        }

        let request = client
            .post(&subscription.url)
            .header("Content-Type", "application/json")
            .header("X-MergeMint-Signature", &signature_header)
            .header("X-Hub-Signature-256", &signature_header)
            .header("X-MergeMint-Event", &payload.event)
            .header("X-MergeMint-Delivery", &payload.id)
            .body(payload_bytes.clone());

        match request.send().await {
            Ok(response) => {
                let status = response.status();
                if status.is_success() {
                    return Ok(());
                }

                if status.is_client_error() && status != reqwest::StatusCode::TOO_MANY_REQUESTS {
                    let body_text = response.text().await.unwrap_or_default();
                    return Err(WebhookDeliveryError::ClientError(
                        StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_REQUEST),
                        body_text,
                    ));
                }

                last_error_message = format!("HTTP status {status}");
            }
            Err(err) => {
                last_error_message = err.to_string();
            }
        }
    }

    Err(WebhookDeliveryError::MaxRetriesExceeded(
        retry_config.max_attempts,
        last_error_message,
    ))
}

/// Find all matching subscriptions in the database and dispatch a webhook event to each.
pub async fn dispatch_event(
    db: &SharedDb,
    client: &reqwest::Client,
    payload: &WebhookPayload,
    retry_config: &RetryConfig,
) -> Vec<Result<(), WebhookDeliveryError>> {
    let subscriptions = list_webhook_subscriptions(db);
    let mut results = Vec::new();

    for subscription in subscriptions {
        if subscription_matches_event(&subscription, &payload.event) {
            let result =
                deliver_webhook_payload(client, &subscription, payload, retry_config).await;
            results.push(result);
        }
    }

    results
}

/// Build a structured webhook payload for a bounty change event.
pub fn build_bounty_event_payload(
    db: &SharedDb,
    event_type: &str,
    bounty_id: &str,
) -> WebhookPayload {
    let bounty_data = get_bounty(db, bounty_id)
        .map(|b| serde_json::to_value(&b).unwrap_or(serde_json::json!({ "id": bounty_id })))
        .unwrap_or(serde_json::json!({ "id": bounty_id }));

    WebhookPayload {
        id: format!("evt_{}", Uuid::new_v4().simple()),
        event: event_type.to_string(),
        timestamp: Utc::now(),
        bounty_id: bounty_id.to_string(),
        data: bounty_data,
    }
}

/// Spawn the asynchronous background task that listens for broadcast bounty events and dispatches webhooks.
pub fn start_webhook_dispatcher(
    state: Arc<AppState>,
    client: reqwest::Client,
    retry_config: RetryConfig,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut rx = state.bounty_broadcast.subscribe();

        loop {
            match rx.recv().await {
                Ok(raw_message) => {
                    let (event_name, bounty_id) =
                        if let Some((evt, id)) = raw_message.split_once(':') {
                            (evt.to_string(), id.to_string())
                        } else {
                            ("bounty_updated".to_string(), raw_message)
                        };

                    let payload = build_bounty_event_payload(&state.db, &event_name, &bounty_id);
                    let _ = dispatch_event(&state.db, &client, &payload, &retry_config).await;
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(missed)) => {
                    tracing::warn!("Webhook dispatcher lagged behind by {missed} events");
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    tracing::info!("Bounty broadcast channel closed; exiting webhook dispatcher");
                    break;
                }
            }
        }
    })
}

/// Create a new webhook subscription.
pub async fn create_subscription_route(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateSubscriptionRequest>,
) -> Result<(StatusCode, Json<WebhookSubscription>), (StatusCode, Json<serde_json::Value>)> {
    let trimmed_url = payload.url.trim();
    if trimmed_url.is_empty()
        || (!trimmed_url.starts_with("http://") && !trimmed_url.starts_with("https://"))
    {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Invalid webhook URL: must start with http:// or https://"
            })),
        ));
    }

    let trimmed_secret = payload.secret.trim();
    if trimmed_secret.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Secret cannot be empty"
            })),
        ));
    }

    let subscription_id = format!("sub_{}", Uuid::new_v4().simple());
    let event_types = if payload.event_types.is_empty() {
        vec!["*".to_string()]
    } else {
        payload.event_types
    };

    let subscription = add_webhook_subscription(
        &state.db,
        subscription_id,
        trimmed_url.to_string(),
        trimmed_secret.to_string(),
        event_types,
    );

    Ok((StatusCode::CREATED, Json(subscription)))
}

/// Retrieve all registered webhook subscriptions.
pub async fn list_subscriptions_route(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<WebhookSubscription>> {
    Json(list_webhook_subscriptions(&state.db))
}

/// Retrieve a single webhook subscription by identifier.
pub async fn get_subscription_route(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<WebhookSubscription>, (StatusCode, Json<serde_json::Value>)> {
    match get_webhook_subscription(&state.db, &id) {
        Some(sub) => Ok(Json(sub)),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": format!("Webhook subscription '{id}' not found")
            })),
        )),
    }
}

/// Delete a webhook subscription by identifier.
pub async fn delete_subscription_route(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    if delete_webhook_subscription(&state.db, &id) {
        Ok((
            StatusCode::OK,
            Json(serde_json::json!({
                "deleted": true,
                "id": id
            })),
        ))
    } else {
        Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": format!("Webhook subscription '{id}' not found")
            })),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::new_shared_db;
    use crate::routes::tx::new_shared_rate_limiter;
    use axum::routing::{get, post};
    use axum::Router;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::net::TcpListener;

    #[test]
    fn test_hmac_sha256_generation_and_verification() {
        let secret = "top-secret-signing-key";
        let body = br#"{"event":"bounty_claimed","bounty_id":"0xabc123"}"#;

        let signature = compute_hmac_sha256(secret, body);
        assert!(!signature.is_empty());
        assert!(verify_hmac_sha256(secret, body, &signature));

        let wrong_secret = "different-key";
        assert!(!verify_hmac_sha256(wrong_secret, body, &signature));

        let tampered_body = br#"{"event":"bounty_claimed","bounty_id":"0xabc999"}"#;
        assert!(!verify_hmac_sha256(secret, tampered_body, &signature));

        assert!(!verify_hmac_sha256(secret, body, "invalidhex!"));
    }

    #[test]
    fn test_event_matching_wildcard_and_specific() {
        let sub_wildcard = WebhookSubscription {
            id: "sub-1".to_string(),
            url: "https://example.com".to_string(),
            secret: "s".to_string(),
            event_types: vec!["*".to_string()],
            active: true,
            created_at: Utc::now(),
        };
        assert!(subscription_matches_event(&sub_wildcard, "bounty_created"));
        assert!(subscription_matches_event(&sub_wildcard, "bounty_claimed"));

        let sub_specific = WebhookSubscription {
            id: "sub-2".to_string(),
            url: "https://example.com".to_string(),
            secret: "s".to_string(),
            event_types: vec!["bounty_claimed".to_string(), "bounty_completed".to_string()],
            active: true,
            created_at: Utc::now(),
        };
        assert!(subscription_matches_event(&sub_specific, "bounty_claimed"));
        assert!(subscription_matches_event(
            &sub_specific,
            "bounty_completed"
        ));
        assert!(!subscription_matches_event(&sub_specific, "bounty_created"));

        let sub_inactive = WebhookSubscription {
            id: "sub-3".to_string(),
            url: "https://example.com".to_string(),
            secret: "s".to_string(),
            event_types: vec!["*".to_string()],
            active: false,
            created_at: Utc::now(),
        };
        assert!(!subscription_matches_event(&sub_inactive, "bounty_claimed"));
    }

    #[test]
    fn test_backoff_delay_calculation() {
        let config = RetryConfig {
            max_attempts: 4,
            initial_delay: Duration::from_millis(100),
            backoff_factor: 2.0,
            max_delay: Duration::from_millis(1000),
        };

        assert_eq!(calculate_backoff_delay(1, &config), Duration::ZERO);
        assert_eq!(
            calculate_backoff_delay(2, &config),
            Duration::from_millis(200)
        );
        assert_eq!(
            calculate_backoff_delay(3, &config),
            Duration::from_millis(400)
        );
        assert_eq!(
            calculate_backoff_delay(4, &config),
            Duration::from_millis(800)
        );
    }

    #[tokio::test]
    async fn test_successful_webhook_delivery_with_signature_check() {
        let secret = "delivery-test-secret";
        let captured_signature = Arc::new(tokio::sync::Mutex::new(String::new()));
        let captured_body = Arc::new(tokio::sync::Mutex::new(Vec::new()));

        let sig_clone = Arc::clone(&captured_signature);
        let body_clone = Arc::clone(&captured_body);

        let receiver_app = Router::new().route(
            "/webhook",
            post(
                move |headers: axum::http::HeaderMap, body: axum::body::Bytes| {
                    let sig = sig_clone.clone();
                    let b = body_clone.clone();
                    async move {
                        if let Some(val) = headers.get("X-MergeMint-Signature") {
                            let mut lock = sig.lock().await;
                            *lock = val.to_str().unwrap_or_default().to_string();
                        }
                        let mut b_lock = b.lock().await;
                        *b_lock = body.to_vec();
                        StatusCode::OK
                    }
                },
            ),
        );

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, receiver_app).await.unwrap();
        });

        let target_url = format!("http://{addr}/webhook");
        let subscription = WebhookSubscription {
            id: "sub-test".to_string(),
            url: target_url,
            secret: secret.to_string(),
            event_types: vec!["*".to_string()],
            active: true,
            created_at: Utc::now(),
        };

        let payload = WebhookPayload {
            id: "evt-123".to_string(),
            event: "bounty_claimed".to_string(),
            timestamp: Utc::now(),
            bounty_id: "bounty-999".to_string(),
            data: serde_json::json!({ "status": "claimed" }),
        };

        let client = reqwest::Client::new();
        let config = RetryConfig::default();

        let result = deliver_webhook_payload(&client, &subscription, &payload, &config).await;
        assert!(result.is_ok());

        let sig_header = captured_signature.lock().await.clone();
        assert!(sig_header.starts_with("sha256="));
        let sig_hex = sig_header.strip_prefix("sha256=").unwrap();

        let raw_body = captured_body.lock().await.clone();
        assert!(verify_hmac_sha256(secret, &raw_body, sig_hex));
    }

    #[tokio::test]
    async fn test_webhook_retry_success_after_transient_failures() {
        let attempts_count = Arc::new(AtomicUsize::new(0));
        let count_clone = Arc::clone(&attempts_count);

        let receiver_app = Router::new().route(
            "/flaky",
            post(move || {
                let count = count_clone.clone();
                async move {
                    let current = count.fetch_add(1, Ordering::SeqCst);
                    if current < 2 {
                        StatusCode::INTERNAL_SERVER_ERROR
                    } else {
                        StatusCode::OK
                    }
                }
            }),
        );

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, receiver_app).await.unwrap();
        });

        let subscription = WebhookSubscription {
            id: "flaky-sub".to_string(),
            url: format!("http://{addr}/flaky"),
            secret: "key".to_string(),
            event_types: vec!["*".to_string()],
            active: true,
            created_at: Utc::now(),
        };

        let payload = WebhookPayload {
            id: "evt-flaky".to_string(),
            event: "bounty_updated".to_string(),
            timestamp: Utc::now(),
            bounty_id: "bounty-flaky".to_string(),
            data: serde_json::json!({}),
        };

        let config = RetryConfig {
            max_attempts: 3,
            initial_delay: Duration::from_millis(10),
            backoff_factor: 1.5,
            max_delay: Duration::from_millis(100),
        };

        let client = reqwest::Client::new();
        let result = deliver_webhook_payload(&client, &subscription, &payload, &config).await;
        assert!(result.is_ok());
        assert_eq!(attempts_count.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_webhook_retry_exceeded_on_persistent_500() {
        let attempts_count = Arc::new(AtomicUsize::new(0));
        let count_clone = Arc::clone(&attempts_count);

        let receiver_app = Router::new().route(
            "/broken",
            post(move || {
                let count = count_clone.clone();
                async move {
                    count.fetch_add(1, Ordering::SeqCst);
                    StatusCode::INTERNAL_SERVER_ERROR
                }
            }),
        );

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, receiver_app).await.unwrap();
        });

        let subscription = WebhookSubscription {
            id: "broken-sub".to_string(),
            url: format!("http://{addr}/broken"),
            secret: "key".to_string(),
            event_types: vec!["*".to_string()],
            active: true,
            created_at: Utc::now(),
        };

        let payload = WebhookPayload {
            id: "evt-broken".to_string(),
            event: "bounty_updated".to_string(),
            timestamp: Utc::now(),
            bounty_id: "bounty-1".to_string(),
            data: serde_json::json!({}),
        };

        let config = RetryConfig {
            max_attempts: 3,
            initial_delay: Duration::from_millis(10),
            backoff_factor: 1.5,
            max_delay: Duration::from_millis(100),
        };

        let client = reqwest::Client::new();
        let result = deliver_webhook_payload(&client, &subscription, &payload, &config).await;
        assert!(matches!(
            result,
            Err(WebhookDeliveryError::MaxRetriesExceeded(3, _))
        ));
        assert_eq!(attempts_count.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_webhook_no_retry_on_bad_request_client_error() {
        let attempts_count = Arc::new(AtomicUsize::new(0));
        let count_clone = Arc::clone(&attempts_count);

        let receiver_app = Router::new().route(
            "/client-err",
            post(move || {
                let count = count_clone.clone();
                async move {
                    count.fetch_add(1, Ordering::SeqCst);
                    StatusCode::BAD_REQUEST
                }
            }),
        );

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, receiver_app).await.unwrap();
        });

        let subscription = WebhookSubscription {
            id: "err-sub".to_string(),
            url: format!("http://{addr}/client-err"),
            secret: "key".to_string(),
            event_types: vec!["*".to_string()],
            active: true,
            created_at: Utc::now(),
        };

        let payload = WebhookPayload {
            id: "evt-client".to_string(),
            event: "bounty_updated".to_string(),
            timestamp: Utc::now(),
            bounty_id: "bounty-1".to_string(),
            data: serde_json::json!({}),
        };

        let config = RetryConfig {
            max_attempts: 3,
            initial_delay: Duration::from_millis(10),
            backoff_factor: 1.5,
            max_delay: Duration::from_millis(100),
        };

        let client = reqwest::Client::new();
        let result = deliver_webhook_payload(&client, &subscription, &payload, &config).await;
        assert!(matches!(
            result,
            Err(WebhookDeliveryError::ClientError(
                StatusCode::BAD_REQUEST,
                _
            ))
        ));
        assert_eq!(attempts_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_subscription_routes_lifecycle() {
        let state = Arc::new(AppState {
            db: new_shared_db(),
            idempotency: crate::db::new_shared_idempotency_store(),
            rate_limiter: new_shared_rate_limiter(),
            bounty_broadcast: tokio::sync::broadcast::channel(16).0,
        });

        let router = Router::new()
            .route(
                "/webhooks/subscriptions",
                post(create_subscription_route).get(list_subscriptions_route),
            )
            .route(
                "/webhooks/subscriptions/:id",
                get(get_subscription_route).delete(delete_subscription_route),
            )
            .with_state(state.clone());

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });

        let client = reqwest::Client::new();
        let base_url = format!("http://{addr}/webhooks/subscriptions");

        let invalid_resp = client
            .post(&base_url)
            .header("Content-Type", "application/json")
            .body(
                serde_json::to_vec(&serde_json::json!({ "url": "invalid-url", "secret": "s" }))
                    .unwrap(),
            )
            .send()
            .await
            .unwrap();
        assert_eq!(invalid_resp.status(), StatusCode::BAD_REQUEST);

        let empty_secret_resp = client
            .post(&base_url)
            .header("Content-Type", "application/json")
            .body(
                serde_json::to_vec(
                    &serde_json::json!({ "url": "https://example.com", "secret": "" }),
                )
                .unwrap(),
            )
            .send()
            .await
            .unwrap();
        assert_eq!(empty_secret_resp.status(), StatusCode::BAD_REQUEST);

        let create_resp = client
            .post(&base_url)
            .header("Content-Type", "application/json")
            .body(
                serde_json::to_vec(&serde_json::json!({
                    "url": "https://example.com/receiver",
                    "secret": "my-secret-key",
                    "event_types": ["bounty_claimed"]
                }))
                .unwrap(),
            )
            .send()
            .await
            .unwrap();
        assert_eq!(create_resp.status(), StatusCode::CREATED);
        let created_sub: WebhookSubscription =
            serde_json::from_slice(&create_resp.bytes().await.unwrap()).unwrap();
        assert_eq!(created_sub.url, "https://example.com/receiver");
        assert_eq!(created_sub.event_types, vec!["bounty_claimed"]);

        let list_resp = client.get(&base_url).send().await.unwrap();
        assert_eq!(list_resp.status(), StatusCode::OK);
        let subs: Vec<WebhookSubscription> =
            serde_json::from_slice(&list_resp.bytes().await.unwrap()).unwrap();
        assert_eq!(subs.len(), 1);
        assert_eq!(subs[0].id, created_sub.id);

        let get_resp = client
            .get(format!("{base_url}/{}", created_sub.id))
            .send()
            .await
            .unwrap();
        assert_eq!(get_resp.status(), StatusCode::OK);
        let fetched_sub: WebhookSubscription =
            serde_json::from_slice(&get_resp.bytes().await.unwrap()).unwrap();
        assert_eq!(fetched_sub.id, created_sub.id);

        let delete_resp = client
            .delete(format!("{base_url}/{}", created_sub.id))
            .send()
            .await
            .unwrap();
        assert_eq!(delete_resp.status(), StatusCode::OK);

        let get_after_del = client
            .get(format!("{base_url}/{}", created_sub.id))
            .send()
            .await
            .unwrap();
        assert_eq!(get_after_del.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_dispatcher_broadcast_end_to_end() {
        let secret = "broadcast-secret";
        let captured_payload = Arc::new(tokio::sync::Mutex::new(None));
        let captured_sig = Arc::new(tokio::sync::Mutex::new(String::new()));
        let captured_raw = Arc::new(tokio::sync::Mutex::new(Vec::new()));

        let payload_clone = Arc::clone(&captured_payload);
        let sig_clone = Arc::clone(&captured_sig);
        let raw_clone = Arc::clone(&captured_raw);

        let receiver_app = Router::new().route(
            "/notify",
            post(
                move |headers: axum::http::HeaderMap, body: axum::body::Bytes| {
                    let p = payload_clone.clone();
                    let s = sig_clone.clone();
                    let r = raw_clone.clone();
                    async move {
                        if let Some(val) = headers.get("X-MergeMint-Signature") {
                            let mut lock = s.lock().await;
                            *lock = val.to_str().unwrap_or_default().to_string();
                        }
                        let mut r_lock = r.lock().await;
                        *r_lock = body.to_vec();
                        if let Ok(parsed) = serde_json::from_slice::<WebhookPayload>(&body) {
                            let mut p_lock = p.lock().await;
                            *p_lock = Some(parsed);
                        }
                        StatusCode::OK
                    }
                },
            ),
        );

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, receiver_app).await.unwrap();
        });

        let (bounty_broadcast, _) = tokio::sync::broadcast::channel(16);
        let db = new_shared_db();
        let state = Arc::new(AppState {
            db: db.clone(),
            idempotency: crate::db::new_shared_idempotency_store(),
            rate_limiter: new_shared_rate_limiter(),
            bounty_broadcast: bounty_broadcast.clone(),
        });

        add_webhook_subscription(
            &db,
            "sub-broadcast".to_string(),
            format!("http://{addr}/notify"),
            secret.to_string(),
            vec!["*".to_string()],
        );

        let client = reqwest::Client::new();
        let dispatcher_handle = start_webhook_dispatcher(
            state.clone(),
            client,
            RetryConfig {
                max_attempts: 2,
                initial_delay: Duration::from_millis(10),
                backoff_factor: 1.0,
                max_delay: Duration::from_millis(50),
            },
        );

        tokio::time::sleep(Duration::from_millis(50)).await;

        let _ = bounty_broadcast.send("bounty_completed:bounty-xyz-123".to_string());

        tokio::time::sleep(Duration::from_millis(150)).await;

        dispatcher_handle.abort();

        let maybe_payload = captured_payload.lock().await.clone();
        assert!(maybe_payload.is_some());
        let payload = maybe_payload.unwrap();
        assert_eq!(payload.event, "bounty_completed");
        assert_eq!(payload.bounty_id, "bounty-xyz-123");

        let sig_header = captured_sig.lock().await.clone();
        assert!(sig_header.starts_with("sha256="));
        let sig_hex = sig_header.strip_prefix("sha256=").unwrap();

        let raw_bytes = captured_raw.lock().await.clone();
        assert!(verify_hmac_sha256(secret, &raw_bytes, sig_hex));
    }
}
