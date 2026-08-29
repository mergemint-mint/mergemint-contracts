// mergemint-backend/src/routes/tx.rs
//
// Transaction / bounty route handlers.

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::db::{read_db, SharedDb};

// ---------------------------------------------------------------------------
// Shared application state
// ---------------------------------------------------------------------------

pub struct AppState {
    pub db: SharedDb,
}

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

/// Substrings that mark a message as carrying internal/sensitive detail
/// (connection strings, credentials, auth headers) rather than a plain
/// caller-facing description. Any constructor whose input matches one of
/// these is redacted to a generic fallback before it reaches the client —
/// the same safety-net principle already applied to 500s below, extended to
/// 4xx constructors so caller-supplied or interpolated detail can never leak
/// through `bad_request`/`not_found` either.
const SENSITIVE_MARKERS: &[&str] = &[
    "postgres://",
    "postgresql://",
    "password",
    "secret",
    "authorization:",
    "bearer ",
];

/// Replace `msg` with `fallback` if it appears to contain internal detail
/// that must never be echoed back to a caller (see `SENSITIVE_MARKERS`).
/// Ordinary caller-facing messages (e.g. "bounty id is required") pass
/// through unchanged.
fn redact_if_sensitive(msg: String, fallback: &str) -> String {
    let lower = msg.to_lowercase();
    if SENSITIVE_MARKERS
        .iter()
        .any(|marker| lower.contains(marker))
    {
        fallback.to_string()
    } else {
        msg
    }
}

impl AppError {
    pub fn bad_request(msg: impl Into<String>) -> (StatusCode, Json<AppError>) {
        let err = AppError {
            code: 400,
            message: redact_if_sensitive(msg.into(), "bad request"),
        };
        (StatusCode::BAD_REQUEST, Json(err))
    }

    pub fn not_found(msg: impl Into<String>) -> (StatusCode, Json<AppError>) {
        let err = AppError {
            code: 404,
            message: redact_if_sensitive(msg.into(), "not found"),
        };
        (StatusCode::NOT_FOUND, Json(err))
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

#[derive(Debug, Deserialize)]
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
pub async fn self_claim(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SelfClaimRequest>,
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

    Ok(Json(ResolveDisputeResponse {
        ok: true,
        xdr: Some(xdr),
    }))
}

// ---------------------------------------------------------------------------
// Tests (#469) — 500 responses must never echo internal detail to client
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use axum::response::IntoResponse;

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

    /// `AppError::bad_request` must redact sensitive detail (e.g. a raw
    /// connection string) just like `AppError::internal` does, rather than
    /// echoing caller-supplied/interpolated internal state back to the
    /// client.
    #[tokio::test]
    async fn bad_request_body_does_not_contain_raw_sensitive_detail() {
        let raw_detail = "invalid config: postgres://internal-host:5432/db";

        let (status, Json(err)) = AppError::bad_request(raw_detail);
        assert_eq!(status, StatusCode::BAD_REQUEST);

        let body = body_string(err.into_response()).await;

        assert!(
            !body.contains(raw_detail),
            "bad_request body must NOT contain the raw sensitive detail; got: {body}"
        );
        assert!(
            body.contains("bad request"),
            "bad_request body must contain the generic fallback message; got: {body}"
        );
    }

    /// `AppError::not_found` must apply the same redaction rules as
    /// `bad_request`/`internal` for any sensitive caller-supplied data.
    #[tokio::test]
    async fn not_found_body_does_not_contain_raw_sensitive_detail() {
        let raw_detail = "lookup failed: password=hunter2";

        let (status, Json(err)) = AppError::not_found(raw_detail);
        assert_eq!(status, StatusCode::NOT_FOUND);

        let body = body_string(err.into_response()).await;

        assert!(
            !body.contains(raw_detail),
            "not_found body must NOT contain the raw sensitive detail; got: {body}"
        );
        assert!(
            body.contains("not found"),
            "not_found body must contain the generic fallback message; got: {body}"
        );
    }

    /// Ordinary, non-sensitive messages must pass through `not_found`
    /// unredacted (mirrors `client_errors_preserve_message` for `bad_request`).
    #[tokio::test]
    async fn not_found_preserves_ordinary_message() {
        let (status, Json(err)) = AppError::not_found("bounty not found");
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(err.message, "bounty not found");
    }
}
