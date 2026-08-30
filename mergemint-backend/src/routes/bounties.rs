/// Bounty listing routes — mounted directly in main.rs.
///
/// Endpoints
/// ---------
/// GET /bounties                     list bounties (paginated)
/// GET /bounties/assignee/{address}  list bounties by assignee
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::sync::Arc;

use crate::db::{
    list_bounties_by_assignee as db_list_bounties_by_assignee, list_bounties_by_creator, BountyPage,
};
use crate::routes::tx::AppState;

// ── Query params ──────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ListParams {
    pub limit: Option<i64>,
    pub cursor: Option<DateTime<Utc>>,
}

/// Maximum page size any listing endpoint here will accept, regardless of
/// what a caller requests. Mirrors the contract-side cap proposed for
/// `get_open_bounties_paged` — without a cap, a caller could request an
/// unbounded page and force an expensive full-table scan/sort.
const MAX_LIST_LIMIT: i64 = 100;

// ── Handlers ──────────────────────────────────────────────────────────────────

/// `GET /bounties`
pub async fn list_bounties(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListParams>,
) -> Json<BountyPage> {
    let limit = params.limit.unwrap_or(20).min(MAX_LIST_LIMIT);
    Json(list_bounties_by_creator(
        &state.db,
        "",
        limit,
        params.cursor,
    ))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{acquire_db, new_shared_db, Bounty};

    fn test_state() -> Arc<AppState> {
        Arc::new(AppState {
            db: new_shared_db(),
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
}
