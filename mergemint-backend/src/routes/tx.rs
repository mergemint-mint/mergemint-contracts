// mergemint-backend/src/routes/tx.rs
//
// Transaction / bounty route handlers.

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use serde::{Deserialize, Serialize};
use soroban_client::{
    address::{Address, AddressTrait},
    contract::{ContractBehavior, Contracts},
    soroban_rpc::{SendTransactionResponse, SendTransactionStatus},
    transaction::{TransactionBehavior, TransactionBuilder, TransactionBuilderBehavior},
    xdr::{self, Limits, WriteXdr},
    Options, Server,
};
use sqlx::{Postgres, QueryBuilder, Row};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use utoipa::{IntoParams, ToSchema};

use crate::db::{
    acquire_idempotency, read_db, read_idempotency, write_audit_log, AuditLog, IdempotencyEntry,
    SharedDb, SharedIdempotencyStore,
};

// ---------------------------------------------------------------------------
// Shared application state
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct AppState {
    pub db: SharedDb,
    pub postgres: Option<sqlx::PgPool>,
    pub idempotency: SharedIdempotencyStore,
    /// In-memory fixed-window rate limiter for expensive, on-chain-submitting
    /// routes such as `self_claim` (issue #670). Keyed by an opaque client
    /// identifier (the claimant address). A production deployment should back
    /// this with a shared store (Redis) so the limit holds across replicas;
    /// the in-process map is sufficient for a single-instance stub backend.
    pub rate_limiter: SharedRateLimiter,
    /// Fan-out channel for bounty state-change notifications, consumed by the
    /// `GET /bounties/stream` SSE handler (see `routes::bounties`).
    pub bounty_broadcast: tokio::sync::broadcast::Sender<String>,
    /// In-memory cache for leaderboard data, with 60-second expiry to avoid
    /// expensive aggregation on every request (see `routes::leaderboard`).
    pub leaderboard_cache: crate::routes::leaderboard::LeaderboardCache,
    /// Interval between SSE keep-alive comments on `GET /bounties/stream`,
    /// so idle connections are not dropped by proxies (`SSE_KEEP_ALIVE_SECS`).
    pub sse_keep_alive_duration: std::time::Duration,
}

/// Maximum `self_claim` calls allowed per `SELF_CLAIM_RATE_WINDOW_SECS` window,
/// per claimant.
pub const SELF_CLAIM_RATE_LIMIT: u32 = 10;
/// Sliding-reset window length (seconds) for the `self_claim` rate limiter.
const SELF_CLAIM_RATE_WINDOW_SECS: u64 = 60;

#[doc(hidden)]
pub struct RateBucket {
    count: u32,
    window_start: u64,
}

pub type SharedRateLimiter = Arc<Mutex<HashMap<String, RateBucket>>>;

/// Construct a new in-process rate-limiter handle.
pub fn new_shared_rate_limiter() -> SharedRateLimiter {
    Arc::new(Mutex::new(HashMap::new()))
}

impl AppState {
    /// Fixed-window rate limiter for `self_claim`, keyed by `key` (the claimant
    /// address). Returns `true` if the request is allowed, `false` if the limit
    /// for the current window is already exhausted.
    pub fn allow_self_claim(&self, key: &str) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let mut buckets = self.rate_limiter.lock().unwrap();
        let bucket = buckets.entry(key.to_string()).or_insert(RateBucket {
            count: 0,
            window_start: now,
        });
        if now.saturating_sub(bucket.window_start) >= SELF_CLAIM_RATE_WINDOW_SECS {
            bucket.window_start = now;
            bucket.count = 0;
        }
        if bucket.count >= SELF_CLAIM_RATE_LIMIT {
            return false;
        }
        bucket.count += 1;
        true
    }
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
#[derive(Debug, Serialize, utoipa::ToSchema)]
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
    pub fn gateway_timeout() -> (StatusCode, Json<AppError>) {
        let err = AppError {
            code: StatusCode::GATEWAY_TIMEOUT.as_u16(),
            message: "request timed out".to_string(),
        };
        (StatusCode::GATEWAY_TIMEOUT, Json(err))
    }

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

fn resolve_dispute_audit_fields<'a>(
    verifier: &'a str,
    bounty_id: &'a str,
    outcome: &'a str,
) -> (&'a str, &'a str, &'a str, u64) {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    (verifier, bounty_id, outcome, timestamp)
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

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CancelBountyRequest {
    pub bounty_id: String,
    pub creator: String,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TopUpBountyRequest {
    pub bounty_id: String,
    pub funder: String,
    /// Positive amount in the reward token's decimal units.
    pub amount: String,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExtendDeadlineRequest {
    pub bounty_id: String,
    pub creator: String,
    pub new_deadline: u32,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UnsignedTransactionResponse {
    pub xdr: String,
    pub estimated_fee: u32,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SubmitSignedTransactionRequest {
    pub source: String,
    pub xdr: String,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SubmitTransactionResponse {
    pub hash: String,
    pub status: String,
    pub latest_ledger: u32,
}

fn bounty_id_scval(bounty_id: &str) -> Result<xdr::ScVal, (StatusCode, Json<AppError>)> {
    let bytes = hex::decode(bounty_id)
        .map_err(|_| AppError::bad_request("bounty_id must be 64 hexadecimal characters"))?;
    if bytes.len() != 32 {
        return Err(AppError::bad_request(
            "bounty_id must be 64 hexadecimal characters",
        ));
    }
    let bytes =
        xdr::ScBytes::try_from(bytes).map_err(|_| AppError::bad_request("invalid bounty_id"))?;
    Ok(xdr::ScVal::Bytes(bytes))
}

fn address_scval(address: &str) -> Result<xdr::ScVal, (StatusCode, Json<AppError>)> {
    Address::new(address)
        .and_then(|parsed| parsed.to_sc_val())
        .map_err(|_| AppError::bad_request("invalid Stellar address"))
}

fn ensure_indexed_creator(
    state: &AppState,
    bounty_id: &str,
    creator: &str,
) -> Result<crate::db::Bounty, (StatusCode, Json<AppError>)> {
    let bounty = crate::db::get_bounty(&state.db, bounty_id)
        .ok_or_else(|| AppError::not_found("bounty not found in indexed data"))?;
    if bounty.creator != creator {
        return Err((
            StatusCode::FORBIDDEN,
            Json(AppError {
                code: StatusCode::FORBIDDEN.as_u16(),
                message: "only the bounty creator may perform this action".to_string(),
            }),
        ));
    }
    Ok(bounty)
}

async fn build_unsigned_transaction(
    source: &str,
    method: &str,
    args: Vec<xdr::ScVal>,
) -> Result<UnsignedTransactionResponse, (StatusCode, Json<AppError>)> {
    let rpc_url = std::env::var("RPC_URL")
        .map_err(|_| AppError::internal("RPC_URL is required to simulate contract transactions"))?;
    let network_passphrase = std::env::var("NETWORK_PASSPHRASE").map_err(|_| {
        AppError::internal("NETWORK_PASSPHRASE is required to build contract transactions")
    })?;
    let contract_id = std::env::var("CONTRACT_ID").map_err(|_| {
        AppError::internal("CONTRACT_ID is required to build contract transactions")
    })?;

    let rpc = Server::new(
        &rpc_url,
        Options {
            allow_http: rpc_url.starts_with("http://"),
            timeout: 30,
            ..Options::default()
        },
    )
    .map_err(|error| {
        tracing::error!(%error, "invalid Soroban RPC configuration");
        AppError::internal("invalid Soroban RPC configuration")
    })?;
    let contract = Contracts::new(&contract_id).map_err(|error| {
        tracing::error!(%error, "invalid MergeMint contract ID");
        AppError::internal("invalid MergeMint contract ID")
    })?;
    let mut account = rpc.get_account(source).await.map_err(|error| {
        tracing::warn!(%error, "failed to load transaction source account");
        AppError::bad_request("transaction source account is unavailable")
    })?;
    let operation = contract.call(method, Some(args));
    let mut builder = TransactionBuilder::new(&mut account, &network_passphrase, None);
    builder.fee(1000u32).add_operation(operation);
    builder
        .set_timeout(30)
        .map_err(|_| AppError::bad_request("unable to set transaction timeout"))?;

    let transaction = builder.build_for_simulation();
    let prepared = rpc
        .prepare_transaction(&transaction)
        .await
        .map_err(|error| {
            tracing::warn!(method, %error, "contract transaction simulation failed");
            (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(AppError {
                    code: StatusCode::UNPROCESSABLE_ENTITY.as_u16(),
                    message: "contract transaction simulation failed".to_string(),
                }),
            )
        })?;
    let estimated_fee = prepared.fee;
    let envelope = prepared.to_envelope().map_err(|error| {
        tracing::error!(%error, "failed to encode prepared transaction");
        AppError::internal("failed to encode prepared transaction")
    })?;
    let xdr = envelope.to_xdr_base64(Limits::none()).map_err(|error| {
        tracing::error!(%error, "failed to encode prepared transaction");
        AppError::internal("failed to encode prepared transaction")
    })?;

    Ok(UnsignedTransactionResponse { xdr, estimated_fee })
}

fn parse_token_amount(amount: &str) -> Result<i128, (StatusCode, Json<AppError>)> {
    let (whole, fraction) = amount.split_once('.').unwrap_or((amount, ""));
    if whole.is_empty()
        || !whole.bytes().all(|byte| byte.is_ascii_digit())
        || fraction.len() > 7
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(AppError::bad_request(
            "amount must be a positive decimal with at most 7 fractional digits",
        ));
    }
    let whole = whole
        .parse::<i128>()
        .map_err(|_| AppError::bad_request("amount is too large"))?;
    let fractional = if fraction.is_empty() {
        0
    } else {
        fraction
            .parse::<i128>()
            .map_err(|_| AppError::bad_request("invalid amount"))?
            * 10_i128.pow((7 - fraction.len()) as u32)
    };
    whole
        .checked_mul(10_000_000)
        .and_then(|value| value.checked_add(fractional))
        .filter(|value| *value > 0)
        .ok_or_else(|| AppError::bad_request("amount must be positive and in range"))
}

/// Build and simulate a creator-authorized `cancel_bounty` invocation.
#[utoipa::path(
    post,
    path = "/tx/cancel",
    request_body = CancelBountyRequest,
    responses(
        (status = 200, description = "Unsigned simulated transaction", body = UnsignedTransactionResponse),
        (status = 400, description = "Invalid request", body = AppError),
        (status = 403, description = "Caller is not the bounty creator", body = AppError),
        (status = 422, description = "Contract simulation failed", body = AppError),
    ),
    tag = "transactions"
)]
pub async fn cancel_bounty_transaction(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CancelBountyRequest>,
) -> Result<Json<UnsignedTransactionResponse>, (StatusCode, Json<AppError>)> {
    if !state.allow_self_claim(&req.creator) {
        return Err(AppError::too_many_requests(
            "transaction rate limit exceeded",
        ));
    }
    ensure_indexed_creator(&state, &req.bounty_id, &req.creator)?;
    let bounty_id = bounty_id_scval(&req.bounty_id)?;
    let creator = address_scval(&req.creator)?;
    build_unsigned_transaction(&req.creator, "cancel_bounty", vec![creator, bounty_id])
        .await
        .map(Json)
}

/// Build and simulate a `sponsor_bounty` invocation for a top-up.
#[utoipa::path(
    post,
    path = "/tx/top-up",
    request_body = TopUpBountyRequest,
    responses(
        (status = 200, description = "Unsigned simulated transaction", body = UnsignedTransactionResponse),
        (status = 400, description = "Invalid request", body = AppError),
        (status = 422, description = "Contract simulation failed", body = AppError),
    ),
    tag = "transactions"
)]
pub async fn top_up_bounty_transaction(
    State(state): State<Arc<AppState>>,
    Json(req): Json<TopUpBountyRequest>,
) -> Result<Json<UnsignedTransactionResponse>, (StatusCode, Json<AppError>)> {
    if !state.allow_self_claim(&req.funder) {
        return Err(AppError::too_many_requests(
            "transaction rate limit exceeded",
        ));
    }
    let amount = parse_token_amount(&req.amount)?;
    let bounty_id = bounty_id_scval(&req.bounty_id)?;
    let funder = address_scval(&req.funder)?;
    let amount = xdr::ScVal::I128(xdr::Int128Parts {
        hi: (amount >> 64) as i64,
        lo: amount as u64,
    });
    build_unsigned_transaction(
        &req.funder,
        "sponsor_bounty",
        vec![funder, bounty_id, amount],
    )
    .await
    .map(Json)
}

/// Build and simulate a creator-authorized `extend_deadline` invocation.
#[utoipa::path(
    post,
    path = "/tx/extend-deadline",
    request_body = ExtendDeadlineRequest,
    responses(
        (status = 200, description = "Unsigned simulated transaction", body = UnsignedTransactionResponse),
        (status = 400, description = "Invalid request", body = AppError),
        (status = 403, description = "Caller is not the bounty creator", body = AppError),
        (status = 422, description = "Contract simulation failed", body = AppError),
    ),
    tag = "transactions"
)]
pub async fn extend_deadline_transaction(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ExtendDeadlineRequest>,
) -> Result<Json<UnsignedTransactionResponse>, (StatusCode, Json<AppError>)> {
    if req.new_deadline == 0 {
        return Err(AppError::bad_request("new_deadline must be positive"));
    }
    if !state.allow_self_claim(&req.creator) {
        return Err(AppError::too_many_requests(
            "transaction rate limit exceeded",
        ));
    }
    let bounty = ensure_indexed_creator(&state, &req.bounty_id, &req.creator)?;
    if i64::from(req.new_deadline) <= chrono::Utc::now().timestamp()
        || bounty
            .deadline
            .is_some_and(|deadline| i64::from(req.new_deadline) <= deadline.timestamp())
    {
        return Err(AppError::bad_request(
            "new_deadline must be in the future and later than the current deadline",
        ));
    }
    let bounty_id = bounty_id_scval(&req.bounty_id)?;
    let creator = address_scval(&req.creator)?;
    let deadline = xdr::ScVal::U32(req.new_deadline);
    build_unsigned_transaction(
        &req.creator,
        "extend_deadline",
        vec![creator, bounty_id, deadline],
    )
    .await
    .map(Json)
}

/// Submit a wallet-signed transaction envelope to Soroban RPC.
#[utoipa::path(
    post,
    path = "/tx/submit",
    request_body = SubmitSignedTransactionRequest,
    responses(
        (status = 200, description = "Transaction accepted by Soroban RPC", body = SubmitTransactionResponse),
        (status = 400, description = "Invalid transaction envelope", body = AppError),
        (status = 422, description = "Soroban RPC rejected the transaction", body = AppError),
    ),
    tag = "transactions"
)]
pub async fn submit_signed_transaction(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SubmitSignedTransactionRequest>,
) -> Result<Json<SubmitTransactionResponse>, (StatusCode, Json<AppError>)> {
    let network_passphrase = std::env::var("NETWORK_PASSPHRASE")
        .map_err(|_| AppError::internal("NETWORK_PASSPHRASE is required to submit transactions"))?;
    let transaction = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        soroban_client::transaction::Transaction::from_xdr_envelope(&req.xdr, &network_passphrase)
    }))
    .map_err(|_| AppError::bad_request("invalid signed transaction envelope"))?;
    if transaction.signatures.is_empty()
        || transaction.source.as_deref() != Some(req.source.as_str())
    {
        return Err(AppError::bad_request(
            "signed transaction source does not match the request",
        ));
    }
    if !state.allow_self_claim(&req.source) {
        return Err(AppError::too_many_requests(
            "transaction rate limit exceeded",
        ));
    }

    let rpc_url = std::env::var("RPC_URL")
        .map_err(|_| AppError::internal("RPC_URL is required to submit transactions"))?;
    let request_body = serde_json::to_vec(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "sendTransaction",
        "params": { "transaction": req.xdr },
    }))
    .map_err(|_| AppError::internal("failed to encode transaction request"))?;
    let response = reqwest::Client::new()
        .post(&rpc_url)
        .header("Content-Type", "application/json")
        .body(request_body)
        .send()
        .await
        .map_err(|error| {
            tracing::warn!(%error, source = %req.source, "Soroban RPC submission failed");
            (
                StatusCode::BAD_GATEWAY,
                Json(AppError {
                    code: StatusCode::BAD_GATEWAY.as_u16(),
                    message: "Soroban RPC submission failed".to_string(),
                }),
            )
        })?;
    let response_body = response.bytes().await.map_err(|error| {
        tracing::warn!(%error, source = %req.source, "failed to read Soroban RPC response");
        (
            StatusCode::BAD_GATEWAY,
            Json(AppError {
                code: StatusCode::BAD_GATEWAY.as_u16(),
                message: "Soroban RPC submission failed".to_string(),
            }),
        )
    })?;
    let rpc_response: serde_json::Value =
        serde_json::from_slice(&response_body).map_err(|error| {
            tracing::warn!(%error, "invalid Soroban RPC response");
            AppError::internal("invalid Soroban RPC response")
        })?;
    let result = rpc_response.get("result").cloned().ok_or_else(|| {
        tracing::warn!(response = %rpc_response, "Soroban RPC returned an error");
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(AppError {
                code: StatusCode::UNPROCESSABLE_ENTITY.as_u16(),
                message: "Soroban RPC rejected the transaction".to_string(),
            }),
        )
    })?;
    let response: SendTransactionResponse = serde_json::from_value(result).map_err(|error| {
        tracing::warn!(%error, "invalid sendTransaction response from Soroban RPC");
        AppError::internal("invalid Soroban RPC response")
    })?;
    let status = format!("{:?}", response.status).to_ascii_lowercase();
    match response.status {
        SendTransactionStatus::Pending | SendTransactionStatus::Duplicate => {
            Ok(Json(SubmitTransactionResponse {
                hash: response.hash,
                status,
                latest_ledger: response.latest_ledger,
            }))
        }
        _ => Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(AppError {
                code: StatusCode::UNPROCESSABLE_ENTITY.as_u16(),
                message: "Soroban RPC rejected the transaction".to_string(),
            }),
        )),
    }
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
    let result = (async {
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
    })
    .await;

    match &result {
        Ok(_) => {
            let (verifier, bounty_id, outcome, timestamp) =
                resolve_dispute_audit_fields(&req.arbitrator, &req.bounty_id, "success");
            tracing::info!(
                verifier = %verifier,
                bounty_id = %bounty_id,
                outcome = %outcome,
                timestamp = timestamp,
                "resolve_dispute succeeded"
            );
            // Write to persistent audit log
            let audit_log = AuditLog::new(
                verifier.to_string(),
                "resolve_dispute".to_string(),
                bounty_id.to_string(),
                timestamp,
            );
            write_audit_log(&state.db, audit_log);
        }
        Err(_) => {
            let (verifier, bounty_id, outcome, timestamp) =
                resolve_dispute_audit_fields(&req.arbitrator, &req.bounty_id, "failure");
            tracing::warn!(
                verifier = %verifier,
                bounty_id = %bounty_id,
                outcome = %outcome,
                timestamp = timestamp,
                "resolve_dispute failed"
            );
        }
    }

    result
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

    // Rate-limit expensive on-chain-submitting routes (#670). Keyed by the
    // claimant so one user cannot hammer `self_claim`; a 429 is returned once
    // the per-window allowance is exhausted.
    if !state.allow_self_claim(&req.claimant) {
        return Err(AppError::too_many_requests(
            "rate limit exceeded for self_claim; retry after the window resets",
        ));
    }

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
// query_audit_logs handler
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct QueryAuditLogsRequest {
    pub actor: Option<String>,
    pub action: Option<String>,
    pub target: Option<String>,
    pub from: Option<u64>,
    pub to: Option<u64>,
    pub cursor: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct QueryAuditLogsResponse {
    pub logs: Vec<AuditLog>,
    pub next_cursor: Option<String>,
}

const MAX_AUDIT_LOG_PAGE_SIZE: i64 = 100;

fn encode_audit_log_cursor(timestamp: u64, id: i64) -> String {
    URL_SAFE_NO_PAD.encode(format!("{timestamp}|{id}").as_bytes())
}

fn decode_audit_log_cursor(cursor: &str) -> Option<(u64, i64)> {
    let decoded = URL_SAFE_NO_PAD.decode(cursor).ok()?;
    let decoded = String::from_utf8(decoded).ok()?;
    let (timestamp, id) = decoded.split_once('|')?;
    let timestamp = timestamp.parse().ok()?;
    let id = id.parse::<i64>().ok().filter(|id| *id > 0)?;
    Some((timestamp, id))
}

/// Query audit logs with optional filtering by actor, action, or target.
/// This is a read-only admin endpoint.
///
/// Supported query parameters:
/// - `actor`: filter by the actor who performed the action (e.g., arbitrator address)
/// - `action`: filter by action type (e.g., "resolve_dispute")
/// - `target`: filter by target (e.g., bounty_id)
/// - `from` / `to`: inclusive Unix timestamps in seconds
/// - `cursor`: opaque cursor returned by the previous page
/// - `limit`: maximum number of results to return (default: 50, max: 100)
#[utoipa::path(
    get,
    path = "/admin/audit-logs",
    params(QueryAuditLogsRequest),
    responses(
        (status = 200, description = "Paginated audit logs", body = QueryAuditLogsResponse),
        (status = 400, description = "Invalid cursor or time range", body = AppError),
    ),
    tag = "admin"
)]
pub async fn query_audit_logs(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<QueryAuditLogsRequest>,
) -> Result<Json<QueryAuditLogsResponse>, (StatusCode, Json<AppError>)> {
    if params
        .from
        .zip(params.to)
        .is_some_and(|(from, to)| from > to)
    {
        return Err(AppError::bad_request(
            "from must be less than or equal to to",
        ));
    }
    if params
        .from
        .into_iter()
        .chain(params.to)
        .any(|timestamp| timestamp > i64::MAX as u64)
    {
        return Err(AppError::bad_request("timestamp is out of range"));
    }

    let cursor = match params.cursor.as_deref() {
        Some(raw) => Some(
            decode_audit_log_cursor(raw).ok_or_else(|| AppError::bad_request("invalid cursor"))?,
        ),
        None => None,
    };
    let limit = params.limit.unwrap_or(50).clamp(1, MAX_AUDIT_LOG_PAGE_SIZE);

    let mut logs = if let Some(pool) = &state.postgres {
        let mut query = QueryBuilder::<Postgres>::new(
            "SELECT id::BIGINT AS id, actor, action, target, timestamp FROM audit_logs WHERE TRUE",
        );
        if let Some(actor) = params.actor.as_deref() {
            query.push(" AND actor = ").push_bind(actor.to_string());
        }
        if let Some(action) = params.action.as_deref() {
            query.push(" AND action = ").push_bind(action.to_string());
        }
        if let Some(target) = params.target.as_deref() {
            query.push(" AND target = ").push_bind(target.to_string());
        }
        if let Some(from) = params.from {
            query.push(" AND timestamp >= ").push_bind(from as i64);
        }
        if let Some(to) = params.to {
            query.push(" AND timestamp <= ").push_bind(to as i64);
        }
        if let Some((timestamp, id)) = cursor {
            query
                .push(" AND (timestamp, id) < (")
                .push_bind(timestamp as i64)
                .push(", ")
                .push_bind(id)
                .push(")");
        }
        let rows = query
            .push(" ORDER BY timestamp DESC, id DESC LIMIT ")
            .push_bind(limit + 1)
            .build()
            .fetch_all(pool)
            .await
            .map_err(|error| {
                tracing::error!(%error, "failed to query persistent audit logs");
                AppError::internal("failed to query audit logs")
            })?;
        rows.into_iter()
            .map(|row| {
                Ok(AuditLog {
                    id: Some(row.try_get("id").map_err(|error| {
                        tracing::error!(%error, "invalid audit log row");
                        AppError::internal("failed to read audit log")
                    })?),
                    actor: row.try_get("actor").map_err(|error| {
                        tracing::error!(%error, "invalid audit log row");
                        AppError::internal("failed to read audit log")
                    })?,
                    action: row.try_get("action").map_err(|error| {
                        tracing::error!(%error, "invalid audit log row");
                        AppError::internal("failed to read audit log")
                    })?,
                    target: row.try_get("target").map_err(|error| {
                        tracing::error!(%error, "invalid audit log row");
                        AppError::internal("failed to read audit log")
                    })?,
                    timestamp: row.try_get::<i64, _>("timestamp").map_err(|error| {
                        tracing::error!(%error, "invalid audit log row");
                        AppError::internal("failed to read audit log")
                    })? as u64,
                })
            })
            .collect::<Result<Vec<_>, (StatusCode, Json<AppError>)>>()?
    } else {
        crate::db::query_audit_logs(
            &state.db,
            params.actor.as_deref(),
            params.action.as_deref(),
            params.target.as_deref(),
            params.from,
            params.to,
            cursor,
            limit + 1,
        )
    };
    let has_more = logs.len() as i64 > limit;
    if has_more {
        logs.truncate(limit as usize);
    }
    let next_cursor = if has_more {
        logs.last()
            .and_then(|log| log.id.map(|id| encode_audit_log_cursor(log.timestamp, id)))
    } else {
        None
    };

    Ok(Json(QueryAuditLogsResponse { logs, next_cursor }))
}

// ---------------------------------------------------------------------------
// Tests (#469) — 500 responses must never echo internal detail to client
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use axum::extract::State;
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

    #[test]
    fn resolve_dispute_audit_fields_include_required_structured_fields() {
        let (verifier, bounty_id, outcome, timestamp) =
            resolve_dispute_audit_fields("arb-123", "bounty-456", "success");

        assert_eq!(verifier, "arb-123");
        assert_eq!(bounty_id, "bounty-456");
        assert_eq!(outcome, "success");
        assert!(timestamp > 0, "timestamp must be unix seconds since epoch");
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
            postgres: None,
            idempotency: crate::db::new_shared_idempotency_store(),
            rate_limiter: new_shared_rate_limiter(),
            bounty_broadcast: tokio::sync::broadcast::channel(16).0,
            leaderboard_cache: crate::routes::leaderboard::new_leaderboard_cache(),
            sse_keep_alive_duration: std::time::Duration::from_secs(15),
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
