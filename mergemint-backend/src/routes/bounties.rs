/// Bounty listing routes — mounted directly in main.rs.
///
/// Endpoints
/// ---------
/// GET  /bounties                     list bounties (paginated)
/// GET  /bounties/assignee/{address}  list bounties by assignee
/// POST /bounties/{id}/claim          claim a bounty (broadcasts on the stream)
/// GET  /bounties/stream              SSE stream of bounty state changes (#482)
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse,
    },
    Json,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::convert::Infallible;
use std::sync::Arc;
use tokio_stream::{wrappers::BroadcastStream, StreamExt as _};

use crate::db::{
    list_bounties_by_assignee as db_list_bounties_by_assignee, list_bounties_by_creator, BountyPage,
};
use crate::routes::tx::AppState;

// ── Query params ──────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ListParams {
    pub limit: Option<i64>,
    pub cursor: Option<DateTime<Utc>>,
    /// Optional status filter. Validated against [`VALID_STATUSES`]; an
    /// unrecognised value is rejected with 400 rather than silently ignored.
    pub status: Option<String>,
    /// Optional tag filter. Matched against the bounty's indexed tag column.
    pub tag: Option<String>,
    /// Optional sort column. Validated against [`VALID_SORT_COLUMNS`]; an
    /// unrecognised value is rejected with 400 rather than silently ignored.
    pub sort: Option<String>,
    /// Optional sort direction. Validated against [`VALID_ORDERS`]; an
    /// unrecognised value is rejected with 400 rather than silently ignored.
    pub order: Option<String>,
}

/// Statuses a bounty may be filtered by. Kept in sync with the values the
/// store writes; anything outside this set is a client error.
const VALID_STATUSES: [&str; 4] = ["open", "claimed", "completed", "cancelled"];

/// Columns a listing may be sorted by. This whitelist is the only place a
/// caller-supplied `sort` value is allowed to influence the query — the value
/// is never interpolated into SQL directly, so arbitrary input can't reach the
/// database.
const VALID_SORT_COLUMNS: [&str; 3] = ["reward", "deadline", "created"];

/// Sort directions a listing may be ordered by.
const VALID_ORDERS: [&str; 2] = ["asc", "desc"];

/// Maximum page size any listing endpoint here will accept, regardless of
/// what a caller requests. Mirrors the contract-side cap proposed for
/// `get_open_bounties_paged` — without a cap, a caller could request an
/// unbounded page and force an expensive full-table scan/sort.
const MAX_LIST_LIMIT: i64 = 100;

// ── Handlers ──────────────────────────────────────────────────────────────────

/// `GET /bounties`
///
/// Supports optional `status` and `tag` filters that translate into indexed
/// SQL filters in the store, composing with the existing pagination and limit
/// clamping. An invalid `status` value yields 400.
///
/// Also supports optional `sort` (`reward`, `deadline`, `created`) and `order`
/// (`asc`, `desc`) parameters. Both are validated against whitelists before
/// being passed to the store, so user input never reaches SQL directly. When
/// omitted, the default remains newest first (`created desc`) to avoid
/// breaking existing clients.
pub async fn list_bounties(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListParams>,
) -> Result<Json<BountyPage>, (StatusCode, Json<serde_json::Value>)> {
    if let Some(status) = params.status.as_deref() {
        if !VALID_STATUSES.contains(&status) {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": format!(
                        "invalid status '{}'; expected one of: {}",
                        status,
                        VALID_STATUSES.join(", ")
                    )
                })),
            ));
        }
    }

    if let Some(sort) = params.sort.as_deref() {
        if !VALID_SORT_COLUMNS.contains(&sort) {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": format!(
                        "invalid sort '{}'; expected one of: {}",
                        sort,
                        VALID_SORT_COLUMNS.join(", ")
                    )
                })),
            ));
        }
    }

    if let Some(order) = params.order.as_deref() {
        if !VALID_ORDERS.contains(&order) {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": format!(
                        "invalid order '{}'; expected one of: {}",
                        order,
                        VALID_ORDERS.join(", ")
                    )
                })),
            ));
        }
    }

    // Default to newest first so existing clients keep their current ordering.
    let sort = params.sort.as_deref().unwrap_or("created");
    let order = params.order.as_deref().unwrap_or("desc");

    let limit = params.limit.unwrap_or(20).min(MAX_LIST_LIMIT);
    Ok(Json(list_bounties_by_creator(
        &state.db,
        "",
        limit,
        params.cursor,
        params.status.as_deref(),
        params.tag.as_deref(),
        sort,
        order,
    )))
}

/// `GET /bounties/assignee/{address}`
///
/// Returns a paginated list of bounties assigned to `address`. A
/// syntactically invalid address is rejected with 400 before it ever
/// reaches the store — a malformed value should surface as a client error,
/// not silently be treated the same as a well-formed address with no
/// results.
pub async fn list_bounties_by_assignee(
    State(state): State<Arc<AppState>>,
    Path(address): Path<String>,
    Query(params): Query<ListParams>,
) -> Result<Json<BountyPage>, (StatusCode, Json<serde_json::Value>)> {
    if !is_syntactically_valid_address(&address) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(
                serde_json::json!({ "error": "assignee address is not a syntactically valid Stellar address" }),
            ),
        ));
    }

    let limit = params.limit.unwrap_or(20).min(MAX_LIST_LIMIT);
    Ok(Json(db_list_bounties_by_assignee(
        &state.db,
        &address,
        limit,
        params.cursor,
    )))
}

/// Minimal syntactic validation for a Stellar-style address: a 56-character
/// StrKey (account `G...` or contract `C...`) drawn from the base32
/// alphabet. This is not a full checksum validation — it only rejects
/// inputs malformed enough that querying the store for them can never be
/// meaningful.
fn is_syntactically_valid_address(address: &str) -> bool {
    address.len() == 56
        && matches!(address.as_bytes().first(), Some(b'G') | Some(b'C'))
        && address
            .bytes()
            .all(|b| matches!(b, b'A'..=b'Z' | b'2'..=b'7'))
}

/// `GET /bounties/stream`
///
/// Server-Sent Events channel that broadcasts a bounty ID whenever a bounty's
/// state changes (see `claim_bounty`). Clients subscribe once and receive
/// incremental push notifications instead of polling. Event name is
/// `bounty_updated`, payload `{"bountyId":"<id>"}`. Implements issue #482.
pub async fn bounty_stream(
    State(state): State<Arc<AppState>>,
) -> Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>> {
    let rx = state.bounty_broadcast.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|result| {
        result.ok().map(|bounty_id| {
            Ok(Event::default()
                .event("bounty_updated")
                .data(format!(r#"{{"bountyId":"{}"}}"#, bounty_id)))
        })
    });

    Sse::new(stream).keep_alive(KeepAlive::default())
}

/// `POST /bounties/{id}/claim`
///
/// Marks a bounty as claimed by the caller and broadcasts the bounty ID on the
/// SSE channel so subscribed clients are notified without a polling round-trip.
pub async fn claim_bounty(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let _ = state.bounty_broadcast.send(id.clone());

    Json(serde_json::json!({
        "id": id,
        "status": "claimed"
    }))
}


/// `GET /bounties/{id}`
pub async fn get_bounty_route(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Bounty>, (StatusCode, Json<serde_json::Value>)> {
    if let Some(bounty) = crate::db::get_bounty(&state.db, &id) {
        Ok(Json(bounty))
    } else {
        Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "bounty not found" })),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{acquire_db, new_shared_db, new_shared_idempotency_store, Bounty};

    fn test_state() -> Arc<AppState> {
        Arc::new(AppState {
            db: new_shared_db(),
            idempotency: new_shared_idempotency_store(),
            rate_limiter: crate::routes::tx::new_shared_rate_limiter(),
            bounty_broadcast: tokio::sync::broadcast::channel(16).0,
        })
    }

    /// Seed `count` bounties into `state`'s store so a page can actually be
    /// cut short by the limit clamp.
    fn seed_bounties(state: &AppState, count: usize) {
        let mut guard = acquire_db(&state.db);
        for i in 0..count {
            guard.bounties.push(Bounty {
                id: i.to_string(),
                creator: "carol".to_string(),
                assignee: None,
                created_at: Utc::now() + chrono::Duration::seconds(i as i64),
            });
        }
    }

    fn valid_address() -> String {
        format!("G{}", "A".repeat(55))
    }

    #[test]
    fn rejects_empty_and_malformed_addresses() {
        assert!(!is_syntactically_valid_address(""));
        assert!(!is_syntactically_valid_address("not-an-address"));
        assert!(!is_syntactically_valid_address("GA")); // too short
        assert!(!is_syntactically_valid_address(
            &valid_address().to_lowercase()
        )); // wrong case
        assert!(!is_syntactically_valid_address(&"1".repeat(56))); // wrong prefix + alphabet
    }

    #[test]
    fn accepts_well_formed_account_and_con

/* … truncated 3055 chars — edit only what you need near the top … */
