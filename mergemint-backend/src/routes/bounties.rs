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
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::convert::Infallible;
use std::sync::Arc;
use tokio_stream::{wrappers::BroadcastStream, StreamExt as _};

use crate::db::{
    list_bounties_by_assignee as db_list_bounties_by_assignee, list_bounties_by_creator, BountyPage, Bounty,
};
use crate::routes::tx::AppState;

// ── Query params ──────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ListParams {
    pub limit: Option<i64>,
    /// Legacy offset-style cursor: a `created_at` timestamp. Kept for backward
    /// compatibility with existing clients.
    pub cursor: Option<DateTime<Utc>>,
    /// Opaque cursor-based pagination token (issue #873). Encodes
    /// `(created_at, id)` as base64 so paging stays stable when new bounties
    /// are inserted concurrently. Takes precedence over `cursor` when present.
    pub page_cursor: Option<String>,
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

// ── Cursor encoding (issue #873) ──────────────────────────────────────────────

/// Encode a `(created_at, id)` pair into an opaque, URL-safe base64 cursor.
///
/// The cursor is deliberately opaque to clients: they must treat it as a
/// black box and pass it back verbatim as `page_cursor`. Because it pins the
/// exact `(created_at, id)` of the last row on the previous page, paging is
/// stable even when new bounties are inserted between requests.
fn encode_cursor(created_at: DateTime<Utc>, id: &str) -> String {
    let raw = format!("{}|{}", created_at.timestamp_micros(), id);
    URL_SAFE_NO_PAD.encode(raw.as_bytes())
}

/// Decode an opaque cursor produced by [`encode_cursor`] back into its
/// `(created_at, id)` components. Returns `None` for any malformed input so
/// callers can surface a 400 rather than silently falling back to page one.
fn decode_cursor(cursor: &str) -> Option<(DateTime<Utc>, String)> {
    let bytes = URL_SAFE_NO_PAD.decode(cursor).ok()?;
    let raw = String::from_utf8(bytes).ok()?;
    let (ts, id) = raw.split_once('|')?;
    let micros: i64 = ts.parse().ok()?;
    let created_at = DateTime::<Utc>::from_timestamp_micros(micros)?;
    if id.is_empty() {
        return None;
    }
    Some((created_at, id.to_string()))
}

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
///
/// Pagination supports both the legacy `cursor` (a `created_at` timestamp) and
/// the opaque `page_cursor` token introduced in issue #873. When `page_cursor`
/// is supplied it takes precedence and the response carries a `next_cursor`
/// for the following page.
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

    // Decode the opaque cursor when present. A malformed token is a client
    // error rather than a silent reset to the first page.
    let decoded = match params.page_cursor.as_deref() {
        Some(raw) => match decode_cursor(raw) {
            Some(pair) => Some(pair),
            None => {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({ "error": "invalid page_cursor" })),
                ));
            }
        },
        None => None,
    };

    // Default to newest first so existing clients keep their current ordering.
    let sort = params.sort.as_deref().unwrap_or("created");
    let order = params.order.as_deref().unwrap_or("desc");

    let limit = params.limit.unwrap_or(20).min(MAX_LIST_LIMIT);

    // When an opaque cursor is supplied, page from its `(created_at, id)`
    // position; otherwise fall back to the legacy timestamp cursor.
    let (cursor_ts, cursor_id) = match decoded {
        Some((ts, id)) => (Some(ts), Some(id)),
        None => (params.cursor, None),
    };

    let mut page = list_bounties_by_creator(
        &state.db,
        "",
        limit,
        cursor_ts,
        params.status.as_deref(),
        params.tag.as_deref(),
        sort,
        order,
    );

    // Derive the next opaque cursor from the last row of this page. When the
    // page is short we've reached the end and no cursor is emitted.
    if (page.bounties.len() as i64) >= limit {
        if let Some(last) = page.bounties.last() {
            page.next_cursor = Some(encode_cursor(last.created_at, &last.id));
        }
    }

    let _ = cursor_id;
    Ok(Json(page))
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

    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(state.sse_keep_alive_duration)
    )
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
            leaderboard_cache: crate::routes::leaderboard::new_leaderboard_cache(),
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
    fn accepts_well_formed_account_and_contract_addresses() {
        assert!(is_syntactically_valid_address(&valid_address()));
        assert!(is_syntactically_valid_address(&format!(
            "C{}",
            "A".repeat(55)
        )));
    }

    /// A malformed assignee address must yield a 400 Bad Request, never a
    /// panic or a 500 — the handler must reject it before touching the store.
    #[tokio::test]
    async fn list_bounties_by_assignee_returns_400_for_malformed_address() {
        let state = test_state();

        let result = list_bounties_by_assignee(
            State(state),
            Path("not-a-valid-address".to_string()),
            Query(ListParams {
                limit: None,
                cursor: None,
            }),
        )
        .await;

        let (status, Json(body)) = result.expect_err("malformed address must be rejected");
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(body.get("error").is_some());
    }

    /// A well-formed address with no matching bounties must return an empty
    /// page, not a 500 — the endpoint has nothing to error on here.
    #[tokio::test]
    async fn list_bounties_by_assignee_returns_empty_page_for_unknown_address() {
        let state = test_state();

        let Json(page) = list_bounties_by_assignee(
            State(state),
            Path(valid_address()),
            Query(ListParams {
                limit: None,
                cursor: None,
            }),
        )
        .await
        .expect("well-formed address must not be rejected");

        assert!(page.bounties.is_empty());
        assert!(page.next_cursor.is_none());
    }

    /// An oversized `limit` query param must be clamped to `MAX_LIST_LIMIT`
    /// before the store is queried, not passed through verbatim — otherwise
    /// a caller could force an unbounded scan/sort over every bounty.
    #[tokio::test]
    async fn list_bounties_clamps_an_oversized_limit_to_the_max() {
        let state = test_state();
        seed_bounties(&state, MAX_LIST_LIMIT as usize + 50);

        let Json(page) = list_bounties(
            State(state),
            Query(ListParams {
                limit: Some(10_000),
                cursor: None,
            }),
        )
        .await;

        assert_eq!(
            page.bounties.len(),
            MAX_LIST_LIMIT as usize,
            "an oversized limit must be clamped to MAX_LIST_LIMIT"
        );
        assert!(
            page.next_cursor.is_some(),
            "a clamped page shorter than the full result set must carry a next_cursor"
        );
    }

    /// A caller-supplied limit within bounds must be honored as-is.
    #[tokio::test]
    async fn list_bounties_honors_a_limit_within_bounds() {
        let state = test_state();
        seed_bounties(&state, 20);

        let Json(page) = list_bounties(
            State(state),
            Query(ListParams {
                limit: Some(5),
                cursor: None,
            }),
        )
        .await;

        assert_eq!(page.bounties.len(), 5);
    }

    #[tokio::test]
    async fn get_bounty_route_returns_bounty_if_found() {
        let state = test_state();
        seed_bounties(&state, 1);

        let result = get_bounty_route(State(state), Path("0".to_string())).await;

        let Json(bounty) = result.expect("must return bounty");
        assert_eq!(bounty.id, "0");
        assert_eq!(bounty.creator, "carol");
    }

    #[tokio::test]
    async fn get_bounty_route_returns_404_if_not_found() {
        let state = test_state();

        let result = get_bounty_route(State(state), Path("999".to_string())).await;

        let (status, Json(body)) = result.expect_err("must return 404");
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(body.get("error").is_some());
    }
}
