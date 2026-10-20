use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use sqlx::Row;
use std::sync::Arc;
use std::time::Duration;
use utoipa::IntoParams;
use uuid::Uuid;

use crate::db::{
    add_webhook_subscription, delete_webhook_subscription, get_bounty, get_webhook_subscription,
    list_webhook_subscriptions, SharedDb, WebhookSubscription,
};
use crate::events::decode_bounty_event;
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

impl RetryConfig {
    pub fn from_env() -> Self {
        let default = Self::default();
        Self {
            max_attempts: std::env::var("WEBHOOK_MAX_ATTEMPTS")
                .ok()
                .and_then(|value| value.parse().ok())
                .filter(|value| *value > 0)
                .unwrap_or(default.max_attempts),
            initial_delay: Duration::from_millis(
                std::env::var("WEBHOOK_INITIAL_DELAY_MS")
                    .ok()
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(default.initial_delay.as_millis() as u64),
            ),
            backoff_factor: std::env::var("WEBHOOK_BACKOFF_FACTOR")
                .ok()
                .and_then(|value| value.parse::<f64>().ok())
                .filter(|value| value.is_finite() && *value >= 1.0)
                .unwrap_or(default.backoff_factor),
            max_delay: Duration::from_millis(
                std::env::var("WEBHOOK_MAX_DELAY_MS")
                    .ok()
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(default.max_delay.as_millis() as u64),
            ),
        }
    }
}

/// Errors that can occur during webhook delivery and retry attempts.
#[derive(Debug)]
pub enum WebhookDeliveryError {
    SerializationError(String),
    ClientError(StatusCode, String),
    MaxRetriesExceeded(usize, String, Option<StatusCode>),
}

impl std::fmt::Display for WebhookDeliveryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SerializationError(msg) => write!(f, "Payload serialization error: {msg}"),
            Self::ClientError(status, msg) => {
                write!(f, "Non-retryable client error {status}: {msg}")
            }
            Self::MaxRetriesExceeded(attempts, msg, _) => {
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
    let mut last_status = None;

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
                last_status = Some(
                    StatusCode::from_u16(status.as_u16())
                        .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                );
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
        last_status,
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
    if let Some(pool) = state.postgres.clone() {
        let delivery_client = client.clone();
        let delivery_config = retry_config.clone();
        tokio::spawn(async move {
            loop {
                match process_next_delivery(&pool, &delivery_client, &delivery_config).await {
                    Ok(true) => {}
                    Ok(false) => tokio::time::sleep(Duration::from_millis(250)).await,
                    Err(error) => {
                        tracing::error!(%error, "webhook delivery queue processing failed");
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                }
            }
        });
    }

    tokio::spawn(async move {
        let mut rx = state.bounty_broadcast.subscribe();

        loop {
            match rx.recv().await {
                Ok(raw_message) => {
                    let event = decode_bounty_event(&raw_message);
                    let payload =
                        build_bounty_event_payload(&state.db, &event.name, &event.bounty_id);
                    if let Some(pool) = &state.postgres {
                        let subscriptions = list_webhook_subscriptions(&state.db);
                        for subscription in subscriptions {
                            if subscription_matches_event(&subscription, &payload.event) {
                                if let Err(error) = sqlx::query(
                                    "INSERT INTO webhook_deliveries (subscription_id, subscription_url, subscription_secret, payload) VALUES ($1, $2, $3, $4)",
                                )
                                .bind(&subscription.id)
                                .bind(&subscription.url)
                                .bind(&subscription.secret)
                                .bind(serde_json::to_value(&payload).unwrap_or_default())
                                .execute(pool)
                                .await
                                {
                                    tracing::error!(%error, "failed to enqueue webhook delivery");
                                }
                            }
                        }
                    } else {
                        for result in
                            dispatch_event(&state.db, &client, &payload, &retry_config).await
                        {
                            if let Err(error) = result {
                                tracing::error!(
                                    event = %event.name,
                                    bounty_id = %event.bounty_id,
                                    %error,
                                    "webhook delivery failed"
                                );
                            }
                        }
                    }
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

async fn process_next_delivery(
    pool: &sqlx::PgPool,
    client: &reqwest::Client,
    retry_config: &RetryConfig,
) -> anyhow::Result<bool> {
    let row = sqlx::query(
        "WITH next_delivery AS (SELECT id FROM webhook_deliveries WHERE status = 'pending' AND next_attempt_at <= NOW() ORDER BY next_attempt_at, id FOR UPDATE SKIP LOCKED LIMIT 1) UPDATE webhook_deliveries AS delivery SET status = 'delivering', updated_at = NOW() FROM next_delivery WHERE delivery.id = next_delivery.id RETURNING delivery.id, delivery.subscription_id, delivery.subscription_url, delivery.subscription_secret, delivery.payload, delivery.attempts",
    )
    .fetch_optional(pool)
    .await?;
    let Some(row) = row else {
        return Ok(false);
    };

    let id: i64 = row.try_get("id")?;
    let subscription = WebhookSubscription {
        id: row.try_get("subscription_id")?,
        url: row.try_get("subscription_url")?,
        secret: row.try_get("subscription_secret")?,
        event_types: Vec::new(),
        active: true,
        created_at: Utc::now(),
    };
    let payload: WebhookPayload = serde_json::from_value(row.try_get("payload")?)?;
    let attempts: i32 = row.try_get("attempts")?;
    let mut single_attempt = retry_config.clone();
    single_attempt.max_attempts = 1;

    let result = deliver_webhook_payload(client, &subscription, &payload, &single_attempt).await;
    let attempt = attempts.saturating_add(1);
    let (status_code, error_detail, permanent) = match &result {
        Ok(()) => (Some(200), None, false),
        Err(WebhookDeliveryError::ClientError(status, detail)) => {
            (Some(status.as_u16() as i32), Some(detail.clone()), true)
        }
        Err(WebhookDeliveryError::MaxRetriesExceeded(_, detail, status)) => (
            status.map(|status| status.as_u16() as i32),
            Some(detail.clone()),
            false,
        ),
        Err(error) => (None, Some(error.to_string()), true),
    };
    sqlx::query(
        "INSERT INTO webhook_delivery_attempts (delivery_id, attempt_number, status_code, error) VALUES ($1, $2, $3, $4)",
    )
    .bind(id)
    .bind(attempt)
    .bind(status_code)
    .bind(error_detail.as_deref())
    .execute(pool)
    .await?;

    match result {
        Ok(()) => {
            sqlx::query(
                "UPDATE webhook_deliveries SET status = 'delivered', attempts = $2, updated_at = NOW(), delivered_at = NOW(), last_status = 200, last_error = NULL WHERE id = $1",
            )
            .bind(id)
            .bind(attempt)
            .execute(pool)
            .await?;
        }
        Err(error) => {
            let exhausted = attempt as usize >= retry_config.max_attempts.max(1);
            if permanent || exhausted {
                sqlx::query(
                    "UPDATE webhook_deliveries SET status = 'dead_letter', attempts = $2, last_status = $3, last_error = $4, updated_at = NOW() WHERE id = $1",
                )
                .bind(id)
                .bind(attempt)
                .bind(status_code)
                .bind(error_detail.as_deref())
                .execute(pool)
                .await?;
            } else {
                let delay = calculate_backoff_delay(attempt as usize + 1, retry_config);
                sqlx::query(
                    "UPDATE webhook_deliveries SET status = 'pending', attempts = $2, last_status = $3, last_error = $4, next_attempt_at = NOW() + ($5 * INTERVAL '1 millisecond'), updated_at = NOW() WHERE id = $1",
                )
                .bind(id)
                .bind(attempt)
                .bind(status_code)
                .bind(error_detail.as_deref())
                .bind(delay.as_millis() as i64)
                .execute(pool)
                .await?;
            }
            tracing::warn!(delivery_id = id, attempt, %error, "webhook delivery attempt failed");
        }
    }

    Ok(true)
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct FailedWebhookQuery {
    pub cursor: Option<i64>,
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct FailedWebhookDelivery {
    pub id: i64,
    pub subscription_id: String,
    #[schema(value_type = Object)]
    pub payload: serde_json::Value,
    pub attempts: i32,
    pub last_status: Option<i32>,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct FailedWebhookPage {
    pub deliveries: Vec<FailedWebhookDelivery>,
    pub next_cursor: Option<i64>,
}

/// List dead-lettered webhook deliveries with cursor pagination.
#[utoipa::path(
    get,
    path = "/admin/webhooks/failed",
    params(FailedWebhookQuery),
    responses(
        (status = 200, description = "Paginated failed webhook deliveries", body = FailedWebhookPage),
        (status = 503, description = "Persistent delivery storage is unavailable", body = crate::routes::tx::AppError),
    ),
    tag = "admin"
)]
pub async fn list_failed_webhooks_route(
    State(state): State<Arc<AppState>>,
    Query(params): Query<FailedWebhookQuery>,
) -> Result<Json<FailedWebhookPage>, (StatusCode, Json<crate::routes::tx::AppError>)> {
    let pool = state.postgres.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(crate::routes::tx::AppError {
                code: StatusCode::SERVICE_UNAVAILABLE.as_u16(),
                message: "persistent delivery storage is unavailable".to_string(),
            }),
        )
    })?;
    if params.cursor.is_some_and(|cursor| cursor <= 0) {
        return Err(crate::routes::tx::AppError::bad_request(
            "cursor must be a positive delivery id",
        ));
    }
    let limit = params.limit.unwrap_or(50).clamp(1, 100);
    let rows = sqlx::query(
        "SELECT id, subscription_id, payload, attempts, last_status, last_error, created_at FROM webhook_deliveries WHERE status = 'dead_letter' AND ($1::BIGINT IS NULL OR id < $1) ORDER BY id DESC LIMIT $2",
    )
    .bind(params.cursor)
    .bind(limit + 1)
    .fetch_all(pool)
    .await
    .map_err(|error| {
        tracing::error!(%error, "failed to query dead-lettered webhook deliveries");
        crate::routes::tx::AppError::internal("failed to query webhook deliveries")
    })?;

    let mut deliveries = rows
        .into_iter()
        .map(|row| {
            Ok(FailedWebhookDelivery {
                id: row.try_get("id").map_err(|error| {
                    tracing::error!(%error, "invalid webhook delivery row");
                    crate::routes::tx::AppError::internal("failed to read webhook delivery")
                })?,
                subscription_id: row.try_get("subscription_id").map_err(|error| {
                    tracing::error!(%error, "invalid webhook delivery row");
                    crate::routes::tx::AppError::internal("failed to read webhook delivery")
                })?,
                payload: row.try_get("payload").map_err(|error| {
                    tracing::error!(%error, "invalid webhook delivery row");
                    crate::routes::tx::AppError::internal("failed to read webhook delivery")
                })?,
                attempts: row.try_get("attempts").map_err(|error| {
                    tracing::error!(%error, "invalid webhook delivery row");
                    crate::routes::tx::AppError::internal("failed to read webhook delivery")
                })?,
                last_status: row.try_get("last_status").map_err(|error| {
                    tracing::error!(%error, "invalid webhook delivery row");
                    crate::routes::tx::AppError::internal("failed to read webhook delivery")
                })?,
                last_error: row.try_get("last_error").map_err(|error| {
                    tracing::error!(%error, "invalid webhook delivery row");
                    crate::routes::tx::AppError::internal("failed to read webhook delivery")
                })?,
                created_at: row.try_get("created_at").map_err(|error| {
                    tracing::error!(%error, "invalid webhook delivery row");
                    crate::routes::tx::AppError::internal("failed to read webhook delivery")
                })?,
            })
        })
        .collect::<Result<Vec<_>, (StatusCode, Json<crate::routes::tx::AppError>)>>()?;
    let has_more = deliveries.len() as i64 > limit;
    if has_more {
        deliveries.truncate(limit as usize);
    }
    let next_cursor = has_more
        .then(|| deliveries.last().map(|delivery| delivery.id))
        .flatten();
    Ok(Json(FailedWebhookPage {
        deliveries,
        next_cursor,
    }))
}

/// Requeue a dead-lettered delivery and record the administrative action.
#[utoipa::path(
    post,
    path = "/admin/webhooks/{id}/replay",
    params(("id" = i64, Path, description = "Dead-letter delivery identifier")),
    responses(
        (status = 202, description = "Delivery queued for replay"),
        (status = 404, description = "Dead-letter delivery not found", body = crate::routes::tx::AppError),
        (status = 503, description = "Persistent delivery storage is unavailable", body = crate::routes::tx::AppError),
    ),
    tag = "admin"
)]
pub async fn replay_webhook_route(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, Json<crate::routes::tx::AppError>)> {
    let pool = state.postgres.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(crate::routes::tx::AppError {
                code: StatusCode::SERVICE_UNAVAILABLE.as_u16(),
                message: "persistent delivery storage is unavailable".to_string(),
            }),
        )
    })?;
    let mut transaction = pool.begin().await.map_err(|error| {
        tracing::error!(%error, delivery_id = id, "failed to begin webhook replay transaction");
        crate::routes::tx::AppError::internal("failed to replay webhook delivery")
    })?;
    let replayed = sqlx::query(
        "UPDATE webhook_deliveries SET status = 'pending', attempts = 0, last_status = NULL, last_error = NULL, next_attempt_at = NOW(), updated_at = NOW(), delivered_at = NULL WHERE id = $1 AND status = 'dead_letter' RETURNING id",
    )
    .bind(id)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(|error| {
        tracing::error!(%error, delivery_id = id, "failed to replay webhook delivery");
        crate::routes::tx::AppError::internal("failed to replay webhook delivery")
    })?;
    if replayed.is_none() {
        return Err(crate::routes::tx::AppError::not_found(
            "dead-letter delivery not found",
        ));
    }

    let timestamp = Utc::now().timestamp().max(0) as u64;
    sqlx::query(
        "INSERT INTO audit_logs (actor, action, target, timestamp) VALUES ($1, $2, $3, $4)",
    )
    .bind("admin")
    .bind("webhook_delivery_replay")
    .bind(id.to_string())
    .bind(timestamp as i64)
    .execute(&mut *transaction)
    .await
    .map_err(|error| {
        tracing::error!(%error, delivery_id = id, "failed to record webhook replay audit entry");
        crate::routes::tx::AppError::internal("failed to record replay audit entry")
    })?;
    transaction.commit().await.map_err(|error| {
        tracing::error!(%error, delivery_id = id, "failed to commit webhook replay transaction");
        crate::routes::tx::AppError::internal("failed to replay webhook delivery")
    })?;
    crate::db::write_audit_log(
        &state.db,
        crate::db::AuditLog::new(
            "admin".to_string(),
            "webhook_delivery_replay".to_string(),
            id.to_string(),
            timestamp,
        ),
    );

    Ok((
        StatusCode::ACCEPTED,
        Json(serde_json::json!({ "id": id, "queued": true })),
    ))
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

    if let Some(pool) = &state.postgres {
        sqlx::query(
            "INSERT INTO webhook_subscriptions (id, url, secret, event_types) VALUES ($1, $2, $3, $4)",
        )
        .bind(&subscription_id)
        .bind(trimmed_url)
        .bind(trimmed_secret)
        .bind(&event_types)
        .execute(pool)
        .await
        .map_err(|error| {
            tracing::error!(%error, "failed to persist webhook subscription");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({
                    "error": "webhook subscription storage unavailable"
                })),
            )
        })?;
    }

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
#[utoipa::path(
    get,
    path = "/webhooks/subscriptions",
    responses(
        (status = 200, description = "Registered webhook subscriptions", body = [WebhookSubscription]),
    ),
    tag = "webhooks"
)]
pub async fn list_subscriptions_route(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<WebhookSubscription>> {
    Json(list_webhook_subscriptions(&state.db))
}

/// Retrieve a single webhook subscription by identifier.
#[utoipa::path(
    get,
    path = "/webhooks/subscriptions/{id}",
    params(("id" = String, Path, description = "Webhook subscription identifier")),
    responses(
        (status = 200, description = "Webhook subscription", body = WebhookSubscription),
        (status = 404, description = "Subscription not found", body = crate::routes::tx::AppError),
    ),
    tag = "webhooks"
)]
pub async fn get_subscription_route(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<WebhookSubscription>, (StatusCode, Json<crate::routes::tx::AppError>)> {
    match get_webhook_subscription(&state.db, &id) {
        Some(sub) => Ok(Json(sub)),
        None => Err(crate::routes::tx::AppError::not_found(format!(
            "webhook subscription '{id}' not found"
        ))),
    }
}

/// Delete a webhook subscription by identifier.
pub async fn delete_subscription_route(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    if let Some(pool) = &state.postgres {
        sqlx::query("DELETE FROM webhook_subscriptions WHERE id = $1")
            .bind(&id)
            .execute(pool)
            .await
            .map_err(|error| {
                tracing::error!(%error, subscription_id = %id, "failed to delete webhook subscription");
                (
                    StatusCode::SERVICE_UNAVAILABLE,
                    Json(serde_json::json!({
                        "error": "webhook subscription storage unavailable"
                    })),
                )
            })?;
    }
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
            Err(WebhookDeliveryError::MaxRetriesExceeded(3, _, _))
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
            postgres: None,
            idempotency: crate::db::new_shared_idempotency_store(),
            rate_limiter: new_shared_rate_limiter(),
            bounty_broadcast: tokio::sync::broadcast::channel(16).0,
            leaderboard_cache: crate::routes::leaderboard::new_leaderboard_cache(),
            sse_keep_alive_duration: std::time::Duration::from_secs(15),
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
            postgres: None,
            idempotency: crate::db::new_shared_idempotency_store(),
            rate_limiter: new_shared_rate_limiter(),
            bounty_broadcast: bounty_broadcast.clone(),
            leaderboard_cache: crate::routes::leaderboard::new_leaderboard_cache(),
            sse_keep_alive_duration: std::time::Duration::from_secs(15),
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
