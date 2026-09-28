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
    list_bounties_by_assignee as db_list_bounties_by_assignee, list_bounties_by_creator, Bounty,
    BountyPage, BountySortField, SortOrder,
};
use crate::routes::tx::AppState;

// ── Query params ──────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ListParams {
    pub limit: Option<i64>,
    pub cursor: Option<DateTime<Utc>>,
    /// Column to order results by. Whitelisted as a Rust enum so an unknown
    /// value is rejected by the extractor with a 400 rather than interpolated
    /// into a query. Defaults to `created`.
    pub sort: Option<BountySortField>,
    /// Sort direction. Whitelisted like `sort`; defaults to `desc`.
    pub order: Option<SortOrder>,
}

/// Maximum page size any listing endpoint here will accept, regardless of
/// what a caller requests. Mirrors the contract-side cap proposed for
/// `get_open_bounties_paged` — without a cap, a caller could request an
/// unbounded page and force an expensive full-table scan/sort.
const MAX_LIST_LIMIT: i64 = 100;

/// Cursor pagination is keyed on `created_at` (see `db::paginate`), so it is
/// only coherent with the default newest-first ordering. Combining a `cursor`
/// with any other `sort` / `order` would return a page that overlaps or skips
/// rows, so it is rejected instead of silently corrupting the listing.
fn cursor_supported(sort: BountySortField, order: SortOrder) -> bool {
    sort == BountySortField::Created && order == SortOrder::Desc
}

// ── Handlers ──────────────────────────────────────────────────────────────────

/// `GET /bounties`
pub async fn list_bounties(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListParams>,
) -> Result<Json<BountyPage>, (StatusCode, Json<serde_json::Value>)> {
    let sort = params.sort.unwrap_or_default();
    let order = params.order.unwrap_or_default();
    if params.cursor.is_some() && !cursor_supported(sort, order) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "cursor pagination is only supported with the default ordering (sort=created&order=desc)"
            })),
        ));
    }
    let limit = params.limit.unwrap_or(20).min(MAX_LIST_LIMIT);
    Ok(Json(list_bounties_by_creator(
        &state.db,
        "",
        limit,
        params.cursor,
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
    let sort = params.sort.unwrap_or_default();
    let order = params.order.unwrap_or_default();
    if params.cursor.is_some() && !cursor_supported(sort, order) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "cursor pagination is only supported with the default ordering (sort=created&order=desc)"
            })),
        ));
    }
    Ok(Json(db_list_bounties_by_assignee(
        &state.db,
        &address,
        limit,
        params.cursor,
        sort,
        order,
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
                reward: i as i128,
                deadline: None,
            });
        }
    }

    fn valid_address() -> String {
        format!("G{}", "A".repeat(55))
    }

    /// Seed a single bounty with explicit sort keys.
    fn seed_bounty(
        state: &AppState,
        id: &str,
        reward: i128,
        deadline: Option<DateTime<Utc>>,
        created_at: DateTime<Utc>,
    ) {
        acquire_db(&state.db).bounties.push(Bounty {
            id: id.to_string(),
            creator: "carol".to_string(),
            assignee: None,
            created_at,
            reward,
            deadline,
        });
    }

    fn params(sort: Option<BountySortField>, order: Option<SortOrder>) -> ListParams {
        ListParams {
            limit: None,
            cursor: None,
            sort,
            order,
        }
    }

    fn ids(page: &BountyPage) -> Vec<String> {
        page.bounties.iter().map(|b| b.id.clone()).collect()
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
                sort: None,
                order: None,
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
                sort: None,
                order: None,
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
                sort: None,
                order: None,
            }),
        )
        .await
        .expect("an oversized limit is still a valid listing request");

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
                sort: None,
                order: None,
            }),
        )
        .await
        .expect("a within-bounds limit is a valid listing request");

        assert_eq!(page.bounties.len(), 5);
    }

    /// No `sort` / `order` supplied must keep the historical newest-first
    /// (created, descending) ordering so existing clients are unaffected.
    #[tokio::test]
    async fn list_bounties_defaults_to_newest_first() {
        let state = test_state();
        seed_bounties(&state, 3);

        let page = list_bounties(State(state), Query(params(None, None)))
            .await
            .expect("default listing must succeed");

        assert_eq!(ids(&page), vec!["2", "1", "0"]);
    }

    /// `sort=reward` orders by reward amount in the requested direction.
    #[tokio::test]
    async fn list_bounties_sorts_by_reward() {
        let state = test_state();
        let now = Utc::now();
        seed_bounty(&state, "a", 10, None, now);
        seed_bounty(&state, "b", 300, None, now);
        seed_bounty(&state, "c", 50, None, now);

        let desc = list_bounties(
            State(state.clone()),
            Query(params(Some(BountySortField::Reward), Some(SortOrder::Desc))),
        )
        .await
        .expect("reward sort must succeed");
        assert_eq!(ids(&desc), vec!["b", "c", "a"]);

        let asc = list_bounties(
            State(state),
            Query(params(Some(BountySortField::Reward), Some(SortOrder::Asc))),
        )
        .await
        .expect("reward sort must succeed");
        assert_eq!(ids(&asc), vec!["a", "c", "b"]);
    }

    /// `sort=deadline` orders soonest-expiring first; bounties with no deadline
    /// sort last in both directions rather than leading the list.
    #[tokio::test]
    async fn list_bounties_sorts_by_deadline_with_none_last() {
        let state = test_state();
        let now = Utc::now();
        seed_bounty(
            &state,
            "soon",
            0,
            Some(now + chrono::Duration::seconds(10)),
            now,
        );
        seed_bounty(
            &state,
            "late",
            0,
            Some(now + chrono::Duration::seconds(100)),
            now,
        );
        seed_bounty(&state, "never", 0, None, now);

        let asc = list_bounties(
            State(state.clone()),
            Query(params(
                Some(BountySortField::Deadline),
                Some(SortOrder::Asc),
            )),
        )
        .await
        .expect("deadline sort must succeed");
        assert_eq!(ids(&asc), vec!["soon", "late", "never"]);

        let desc = list_bounties(
            State(state),
            Query(params(
                Some(BountySortField::Deadline),
                Some(SortOrder::Desc),
            )),
        )
        .await
        .expect("deadline sort must succeed");
        assert_eq!(ids(&desc), vec!["late", "soon", "never"]);
    }

    /// `sort=created&order=asc` reverses the default newest-first order.
    #[tokio::test]
    async fn list_bounties_sorts_by_created_ascending() {
        let state = test_state();
        seed_bounties(&state, 3);

        let page = list_bounties(
            State(state),
            Query(params(Some(BountySortField::Created), Some(SortOrder::Asc))),
        )
        .await
        .expect("created sort must succeed");

        assert_eq!(ids(&page), vec!["0", "1", "2"]);
    }

    /// An unknown `sort` value must be rejected with 400 by the query extractor
    /// before the handler runs, so user input can never reach the query layer.
    #[tokio::test]
    async fn list_bounties_rejects_unknown_sort_and_order_values() {
        use axum::body::Body;
        use axum::http::Request;
        use tower::ServiceExt;

        fn app(state: Arc<AppState>) -> axum::Router {
            axum::Router::new()
                .route("/bounties", axum::routing::get(list_bounties))
                .with_state(state)
        }

        let res = app(test_state())
            .oneshot(
                Request::builder()
                    .uri("/bounties?sort=drop_table")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);

        let res = app(test_state())
            .oneshot(
                Request::builder()
                    .uri("/bounties?order=sideways")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }

    /// A whitelisted `sort` / `order` pair is accepted by the extractor and
    /// returns 200.
    #[tokio::test]
    async fn list_bounties_accepts_whitelisted_sort_params() {
        use axum::body::Body;
        use axum::http::Request;
        use tower::ServiceExt;

        let app = axum::Router::new()
            .route("/bounties", axum::routing::get(list_bounties))
            .with_state(test_state());

        let res = app
            .oneshot(
                Request::builder()
                    .uri("/bounties?sort=reward&order=asc")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    /// `cursor` pagination is only coherent with the default newest-first
    /// ordering; combining it with a non-default sort must 400 rather than
    /// return a page that skips or repeats rows.
    #[tokio::test]
    async fn list_bounties_rejects_cursor_with_non_default_sort() {
        use axum::body::Body;
        use axum::http::Request;
        use tower::ServiceExt;

        let app = axum::Router::new()
            .route("/bounties", axum::routing::get(list_bounties))
            .with_state(test_state());

        let res = app
            .oneshot(
                Request::builder()
                    .uri("/bounties?sort=reward&cursor=2020-01-01T00:00:00Z")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }

    /// A `cursor` with the default ordering is still accepted.
    #[tokio::test]
    async fn list_bounties_accepts_cursor_with_default_order() {
        use axum::body::Body;
        use axum::http::Request;
        use tower::ServiceExt;

        let app = axum::Router::new()
            .route("/bounties", axum::routing::get(list_bounties))
            .with_state(test_state());

        let res = app
            .oneshot(
                Request::builder()
                    .uri("/bounties?cursor=2020-01-01T00:00:00Z")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
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
