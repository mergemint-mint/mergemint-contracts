// mergemint-backend/src/routes/tx.rs
//
// Transaction / bounty route handlers.

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;

use crate::db::{
    acquire_idempotency, read_db, read_idempotency, IdempotencyEntry, SharedDb,
    SharedIdempotencyStore,
};

// ---------------------------------------------------------------------------
// Shared application state
// ---------------------------------------------------------------------------

pub struct AppState {
    pub db: SharedDb,
    pub idempotency: SharedIdempotencyStore,
}

// ---------------------------------------------------------------------------
// Idempotency-Key handling (claim_bounty double-submit guard)
// ---------------------------------------------------------------------------

/// The header clients set to make a transaction-submitting request safe to
/// retry after a timeout.
const IDEMPOTENCY_KEY_HEADER: &str = "idempotency-key";

/// Outcome of checking an inbound `Idempotency-Key` before doing any work.
enum IdempotencyCheck {
    /// No key was supplied; proceed without dedup (back-compat default).
    NotRequested,
    /// A fresh key was reserved as in-flight; the caller must finalize it
    /// via `finalize_idempotency_key` once the request completes.
    Proceed(String),
    /// A prior request with this key already completed; replay its response
    /// instead of resubmitting the transaction.
    Replay(Response),
    /// A prior request with this key is still being processed.
    Conflict(Response),
}

/// Looks up `Idempotency-Key` in `headers` against `store` and either
/// reserves the key as in-flight or short-circuits with a replayed /
/// conflict response.
///
/// ## Why dedup here (double-submit guard)
///
/// `resolve_dispute` and `self_claim` both submit a chain transaction. If a
/// client's connection drops after the transaction was built but before the
/// response arrived, a naive retry would submit it a second time. Callers
/// that pass an `Idempotency-Key` header get exactly-once handling: the
/// first request wins and every retry with the same key gets that same
/// result back instead of re-triggering the transaction build.
fn check_idempotency_key(headers: &HeaderMap, store: &SharedIdempotencyStore) -> IdempotencyCheck {
    let Some(key) = headers
        .get(IDEMPOTENCY_KEY_HEADER)
        .and_then(|v| v.to_str().ok())
        .filter(|v| !v.is_empty())
    else {
        return IdempotencyCheck::NotRequested;
    };

    // Fast path: a read lock is enough to detect an already-completed or
    // already-in-flight key without taking the write lock.
    if let Some(entry) = read_idempotency(store).entries.get(key) {
        return idempotency_check_from_entry(entry);
    }

    let mut guard = acquire_idempotency(store);
    // Re-check under the write lock in case another request reserved the
    // key between our read above and acquiring the write lock.
    match guard.entries.get(key) {
        Some(entry) => idempotency_check_from_entry(entry),
        None => {
            guard
                .entries
                .insert(key.to_string(), IdempotencyEntry::InFlight);
            IdempotencyCheck::Proceed(key.to_string())
        }
    }
}

fn idempotency_check_from_entry(entry: &IdempotencyEntry) -> IdempotencyCheck {
    match entry {
        IdempotencyEntry::Completed(body) => IdempotencyCheck::Replay(
            Response::builder()
                .status(StatusCode::OK)
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(body.clone()))
                .expect("static response parts always build a valid response"),
        ),
        IdempotencyEntry::InFlight => IdempotencyCheck::Conflict(
            AppError {
                code: 409,
                message: "a request with this Idempotency-Key is already in progress".to_string(),
            }
            .into_response(),
        ),
    }
}

/// Records the final outcome of a request that reserved `key` via
/// `check_idempotency_key`.
///
/// On success the response body is cached so retries replay it. On failure
/// the reservation is removed entirely -- no transaction was submitted, so
/// the key is free to be reused once the client fixes the request.
fn finalize_idempotency_key(store: &SharedIdempotencyStore, key: String, result: Option<String>) {
    let mut guard = acquire_idempotency(store);
    match result {
        Some(body) => {
            guard.entries.insert(key, IdempotencyEntry::Completed(body));
        }
        None => {
            guard.entries.remove(&key);
        }
    }
}

/// Default self-claim rate limit: 5 relay submissions per claimant per minute.
pub const SELF_CLAIM_RATE_LIMIT: u32 = 5;
pub const SELF_CLAIM_RATE_WINDOW: Duration = Duration::from_secs(60);

// ---------------------------------------------------------------------------
// Error helpers
// ---------------------------------------------------------------------------

/// The JSON shape returned to the client on any error.
///
/// **Security note (#469):** For `INTERNAL_SERVER_ERROR` (500) responses the
/// `message` field is *always* set to the generic string
/// `"internal server error"` before serialising.  The original detail is
/// emitted via `tracing::error!` so it appears in server logs without ever
/// being sent to the client.  This prevents RPC debug strings, stack traces,
/// and other internal state from leaking across the API boundary.
#[derive(Debug, Serialize)]
pub struct AppError {
    pub code: u16,
    pub message: String,
}

impl AppError {
    pub fn bad_request(msg: impl Into<String>) -> (StatusCode, Json<AppError>) {
        let err = AppError {
            code: 400,
            message: msg.into(),
        };
        (StatusCode::BAD_REQUEST, Json(err))
    }

    pub fn not_found(msg: impl Into<String>) -> (StatusCode, Json<AppError>) {
        let err = AppError {
            code: 404,
            message: msg.into(),
        };
        (StatusCode::NOT_FOUND, Json(err))
    }

    pub fn too_many_requests(msg: impl Into<String>) -> (StatusCode, Json<AppError>) {
        let err = AppError {
            code: 429,
            message: msg.into(),
        };
        (StatusCode::TOO_MANY_REQUESTS, Json(err))
    }

    /// Construct an internal server error.
    ///
    /// `detail` is **only** logged via `tracing::error!`; it is never
    /// included in the HTTP response body (see [`IntoResponse`] impl below).
    ///
    /// No handler in this stub backend currently produces a 500 (both
    /// `resolve_dispute` and `self_claim` only ever return 400/404), so this
    /// constructor has no live call site yet — it's exercised by the tests
    /// below and is here for handlers that build real Horizon/RPC calls.
    #[allow(dead_code)]
    pub fn internal(detail: impl std::fmt::Display) -> AppError {
        tracing::error!(detail = %detail, "internal server error");
        AppError {
            code: 500,
            // The client-facing message is always generic — the real detail
            // lives in the log line emitted above.
            message: "internal server error".to_string(),
        }
    }
}

/// Converts `AppError` into an HTTP response, redacting internal detail.
///
/// For status 500 the `message` field is *replaced* with the generic string
/// `"internal server error"` even if the `AppError` value was constructed some
/// other way.  This acts as a safety net so that any code path that produces a
/// 500 cannot accidentally expose internal state to the client.
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = StatusCode::from_u16(self.code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

        // Redact the message for any 5xx response before sending to client.
        let safe_message = if status.is_server_error() {
            "internal server error".to_string()
        } else {
            self.message
        };

        let body = AppError {
            code: status.as_u16(),
            message: safe_message,
        };

        (status, Json(body)).into_response()
    }
}

// ---------------------------------------------------------------------------
// Domain types (minimal stubs)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bounty {
    pub id: String,
    pub creator: String,
    pub amount: u64,
    /// Unix timestamp (seconds) after which a self-claim is considered stale.
    pub expires_at: i64,
}

#[derive(Debug, Deserialize)]
pub struct ResolveDisputeRequest {
    pub bounty_id: String,
    pub arbitrator: String,
    pub winner: String,
}

#[derive(Debug, Serialize)]
pub struct ResolveDisputeResponse {
    pub ok: bool,
    pub xdr: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SelfClaimRequest {
    pub bounty_id: String,
    pub claimant: String,
}

// ---------------------------------------------------------------------------
// resolve_dispute handler (#474 + #475)
// ---------------------------------------------------------------------------

/// Resolve a disputed bounty by paying out the `winner`.
///
/// ## Short-circuit precheck (#474)
///
/// Only the bounty creator is authorised to act as arbitrator.  We reject
/// requests where `arbitrator != bounty.creator` *before* building XDR so we
/// never waste Horizon round-trips on unauthorised calls.
pub async fn resolve_dispute(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ResolveDisputeRequest>,
) -> Result<Json<ResolveDisputeResponse>, (StatusCode, Json<AppError>)> {
    let bounty = {
        let db = read_db(&state.db);
        let raw = db
            .records
            .get(&req.bounty_id)
            .ok_or_else(|| AppError::not_found("bounty not found"))?
            .clone();
        serde_json::from_str::<Bounty>(&raw)
            .map_err(|_| AppError::bad_request("corrupt bounty record"))?
    };

    // -- precheck: only the bounty creator may arbitrate (#474) --------------
    if req.arbitrator != bounty.creator {
        return Err(AppError::bad_request(
            "only the bounty creator may act as arbitrator",
        ));
    }

    // Build the payout XDR (stub — real implementation invokes Stellar SDK).
    let xdr = build_payout_xdr(&bounty, &req.winner);

    Ok(Json(ResolveDisputeResponse {
        ok: true,
        xdr: Some(xdr),
    }))
}

/// Stub XDR builder.  Replace with real Stellar `TransactionBuilder` logic.
fn build_payout_xdr(bounty: &Bounty, winner: &str) -> String {
    format!(
        "XDR:bounty={},winner={},amount={}",
        bounty.id, winner, bounty.amount
    )
}

// ---------------------------------------------------------------------------
// self_claim handler (#475)
// ---------------------------------------------------------------------------

/// Allow a claimant to self-claim a bounty after the creator has not resolved
/// it within the agreed window.
///
/// ## Staleness window note (#475)
///
/// The `expires_at` field on the bounty marks the Unix timestamp (in seconds)
/// after which the bounty is considered unresolved by the creator and the
/// claimant may collect it unilaterally.  We check the current time against
/// this threshold *before* any on-chain interaction to avoid wasting gas on
/// claims that would be rejected by the contract anyway.
///
/// The window is set by the bounty creator at creation time and is stored
/// on-chain; this server-side check is an optimistic guard only — the contract
/// enforces the same rule authoritatively.
///
/// ## Idempotency-Key double-submit guard
///
/// This handler submits a chain transaction, so a client retry after a
/// timeout could double-submit without protection. Callers may pass an
/// `Idempotency-Key` header; see `check_idempotency_key` /
/// `finalize_idempotency_key` above. The header is optional so existing
/// callers keep working unchanged.
pub async fn self_claim(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<SelfClaimRequest>,
) -> Result<Response, (StatusCode, Json<AppError>)> {
    let idempotency_key = match check_idempotency_key(&headers, &state.idempotency) {
        IdempotencyCheck::NotRequested => None,
        IdempotencyCheck::Proceed(key) => Some(key),
        IdempotencyCheck::Replay(response) | IdempotencyCheck::Conflict(response) => {
            return Ok(response)
        }
    };

    let result = self_claim_inner(&state, &req).await;

    if let Some(key) = idempotency_key {
        let cached_body = result
            .as_ref()
            .ok()
            .and_then(|resp| serde_json::to_string(resp).ok());
        finalize_idempotency_key(&state.idempotency, key, cached_body);
    }

    result.map(|resp| Json(resp).into_response())
}

/// The actual self-claim business logic, factored out of the handler so the
/// idempotency wrapper in `self_claim` can call it without duplicating the
/// staleness check or XDR-building steps.
async fn self_claim_inner(
    state: &AppState,
    req: &SelfClaimRequest,
) -> Result<ResolveDisputeResponse, (StatusCode, Json<AppError>)> {
    let bounty = {
        let db = read_db(&state.db);
        let raw = db
            .records
            .get(&req.bounty_id)
            .ok_or_else(|| AppError::not_found("bounty not found"))?
            .clone();
        serde_json::from_str::<Bounty>(&raw)
            .map_err(|_| AppError::bad_request("corrupt bounty record"))?
    };

    // -- self-claim staleness precheck (#475) ---------------------------------
    // The staleness window is the period between bounty creation and
    // `expires_at`.  A claimant may only self-claim once that window has
    // elapsed, i.e. when the current time is strictly past `expires_at`.
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    if now <= bounty.expires_at {
        return Err(AppError::bad_request(
            "self-claim not yet available: staleness window has not elapsed",
        ));
    }

    let xdr = build_payout_xdr(&bounty, &req.claimant);

    Ok(ResolveDisputeResponse {
        ok: true,
        xdr: Some(xdr),
    })
}

// ---------------------------------------------------------------------------
// Tests (#469) — 500 responses must never echo internal detail to client
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rate_limit::TokenBucketLimiter;
    use axum::body::to_bytes;
    use axum::extract::State;
    use axum::response::IntoResponse;
    use std::time::Duration;

    /// Helper: convert a Response body to a String.
    async fn body_string(response: axum::response::Response) -> String {
        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("failed to read response body");
        String::from_utf8_lossy(&bytes).into_owned()
    }

    /// A 500 response must never contain the raw internal detail string in the
    /// response body.  The detail must only appear in server-side logs.
    #[tokio::test]
    async fn internal_error_body_does_not_contain_raw_detail() {
        let raw_detail = "sqlx error: could not connect to postgres://internal-host:5432/db";

        let response = AppError::internal(raw_detail).into_response();

        assert_eq!(
            response.status(),
            StatusCode::INTERNAL_SERVER_ERROR,
            "status must be 500"
        );

        let body = body_string(response).await;

        assert!(
            !body.contains(raw_detail),
            "response body must NOT contain the raw internal detail; got: {body}"
        );
        assert!(
            body.contains("internal server error"),
            "response body must contain the generic message; got: {body}"
        );
    }

    /// Even if an `AppError` is constructed by hand with `code: 500` and a
    /// sensitive message, `IntoResponse` must still redact it.
    #[tokio::test]
    async fn manually_constructed_500_is_redacted() {
        let sensitive = "postgres password is hunter2";
        let err = AppError {
            code: 500,
            message: sensitive.to_string(),
        };

        let body = body_string(err.into_response()).await;

        assert!(
            !body.contains(sensitive),
            "manually constructed 500 body must not contain sensitive text; got: {body}"
        );
        assert!(
            body.contains("internal server error"),
            "body must contain the safe fallback message; got: {body}"
        );
    }

    /// 4xx errors must still surface their descriptive message to the client.
    #[tokio::test]
    async fn client_errors_preserve_message() {
        let (status, Json(err)) = AppError::bad_request("bounty id is required");
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(err.message, "bounty id is required");
    }

    /// `AppError` via `IntoResponse` for a 400 must preserve the message.
    #[tokio::test]
    async fn bad_request_into_response_preserves_message() {
        let err = AppError {
            code: 400,
            message: "missing field: bounty_id".to_string(),
        };
        let body = body_string(err.into_response()).await;
        assert!(
            body.contains("missing field: bounty_id"),
            "400 body must contain the original message; got: {body}"
        );
    }

    // ---------------------------------------------------------------------
    // Tests — Idempotency-Key double-submit guard for self_claim
    // ---------------------------------------------------------------------

    /// A bounty whose staleness window has already elapsed, so `self_claim`
    /// succeeds without any extra setup.
    fn claimable_state(bounty_id: &str) -> Arc<AppState> {
        let db = crate::db::new_shared_db();
        {
            let mut guard = crate::db::acquire_db(&db);
            let bounty = Bounty {
                id: bounty_id.to_string(),
                creator: "creator-address".to_string(),
                amount: 100,
                expires_at: 0, // already in the past
            };
            guard.records.insert(
                bounty_id.to_string(),
                serde_json::to_string(&bounty).unwrap(),
            );
        }
        Arc::new(AppState {
            db,
            idempotency: crate::db::new_shared_idempotency_store(),
        })
    }

    fn idempotency_headers(key: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::HeaderName::from_static(IDEMPOTENCY_KEY_HEADER),
            axum::http::HeaderValue::from_str(key).unwrap(),
        );
        headers
    }

    /// Without an Idempotency-Key header, self_claim behaves exactly as
    /// before (no dedup) — the default must stay backward compatible.
    #[tokio::test]
    async fn self_claim_without_idempotency_key_works_as_before() {
        let state = claimable_state("bounty-1");
        let response = self_claim(
            State(Arc::clone(&state)),
            HeaderMap::new(),
            Json(SelfClaimRequest {
                bounty_id: "bounty-1".to_string(),
                claimant: "claimant-address".to_string(),
            }),
        )
        .await
        .expect("self_claim should succeed");

        assert_eq!(response.status(), StatusCode::OK);
    }

    /// A second request with the same Idempotency-Key must replay the first
    /// request's response instead of re-running the claim logic. We prove
    /// this by deleting the bounty record after the first call: if the
    /// second call actually re-executed, it would 404 instead of matching
    /// the first response's body.
    #[tokio::test]
    async fn self_claim_replays_cached_response_for_repeated_key() {
        let state = claimable_state("bounty-2");
        let req = SelfClaimRequest {
            bounty_id: "bounty-2".to_string(),
            claimant: "claimant-address".to_string(),
        };

        let first = self_claim(
            State(Arc::clone(&state)),
            idempotency_headers("retry-key-1"),
            Json(SelfClaimRequest {
                bounty_id: req.bounty_id.clone(),
                claimant: req.claimant.clone(),
            }),
        )
        .await
        .expect("first call should succeed");
        assert_eq!(first.status(), StatusCode::OK);
        let first_body = body_string(first).await;

        // Remove the bounty so a re-executed claim would fail with 404 —
        // proving the second response below came from the idempotency cache.
        crate::db::acquire_db(&state.db).records.remove("bounty-2");

        let second = self_claim(
            State(Arc::clone(&state)),
            idempotency_headers("retry-key-1"),
            Json(req),
        )
        .await
        .expect("replayed call must not error even though the bounty is now gone");
        assert_eq!(second.status(), StatusCode::OK);
        let second_body = body_string(second).await;

        assert_eq!(
            first_body, second_body,
            "retry with the same Idempotency-Key must replay the original response"
        );
    }

    /// A request that arrives while another request with the same key is
    /// still in flight must be rejected with 409 rather than racing the
    /// same transaction submission a second time.
    #[tokio::test]
    async fn self_claim_returns_409_for_in_flight_key() {
        let state = claimable_state("bounty-3");
        crate::db::acquire_idempotency(&state.idempotency)
            .entries
            .insert(
                "concurrent-key".to_string(),
                crate::db::IdempotencyEntry::InFlight,
            );

        let response = self_claim(
            State(Arc::clone(&state)),
            idempotency_headers("concurrent-key"),
            Json(SelfClaimRequest {
                bounty_id: "bounty-3".to_string(),
                claimant: "claimant-address".to_string(),
            }),
        )
        .await
        .expect("an in-flight key must short-circuit with a response, not an error");

        assert_eq!(response.status(), StatusCode::CONFLICT);
    }

    /// A failed claim (e.g. unknown bounty id) must not leave a stale
    /// reservation behind — the client should be able to retry the same key
    /// once it fixes the request.
    #[tokio::test]
    async fn self_claim_error_releases_the_idempotency_key() {
        let state = claimable_state("bounty-4");

        let err = self_claim(
            State(Arc::clone(&state)),
            idempotency_headers("error-key"),
            Json(SelfClaimRequest {
                bounty_id: "does-not-exist".to_string(),
                claimant: "claimant-address".to_string(),
            }),
        )
        .await
        .expect_err("unknown bounty id must error");
        assert_eq!(err.0, StatusCode::NOT_FOUND);

        assert!(
            !crate::db::read_idempotency(&state.idempotency)
                .entries
                .contains_key("error-key"),
            "a failed request must not leave the key reserved"
        );
    }
}
