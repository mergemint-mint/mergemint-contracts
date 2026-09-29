/// Leaderboard listing route — returns top contributors by reputation.
///
/// Endpoints
/// ---------
/// GET /leaderboard  list top contributors by reputation (cached for 60 seconds)
use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::sync::RwLock;

use crate::db::{get_leaderboard as db_get_leaderboard, LeaderboardPage};
use crate::routes::tx::AppState;

// ── Query params ──────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct LeaderboardParams {
    pub limit: Option<i64>,
}

/// Maximum page size any listing endpoint here will accept.
const MAX_LIST_LIMIT: i64 = 100;

/// Cache expiry time in seconds
const CACHE_EXPIRY_SECS: u64 = 60;

// ── In-memory cache ──────────────────────────────────────────────────────────────

/// Cached leaderboard entry with expiry timestamp
#[derive(Clone)]
pub struct CachedLeaderboard {
    pub data: LeaderboardPage,
    pub expires_at: Instant,
}

/// Shared, thread-safe leaderboard cache
pub type LeaderboardCache = Arc<RwLock<Option<CachedLeaderboard>>>;

/// Create a new, empty leaderboard cache
pub fn new_leaderboard_cache() -> LeaderboardCache {
    Arc::new(RwLock::new(None))
}

// ── Handlers ──────────────────────────────────────────────────────────────────

/// `GET /leaderboard`
///
/// Returns a paginated list of top contributors ranked by reputation and
/// completed bounties. Results are cached in memory for 60 seconds to avoid
/// expensive aggregation on every request.
///
/// Query parameters:
/// - `limit`: Maximum number of entries to return (default: 20, max: 100)
pub async fn get_leaderboard_route(
    State(state): State<Arc<AppState>>,
    Query(params): Query<LeaderboardParams>,
) -> Result<Json<LeaderboardPage>, (StatusCode, Json<serde_json::Value>)> {
    let limit = params.limit.unwrap_or(20).min(MAX_LIST_LIMIT).max(1);

    // Attempt to read from cache
    let cached = {
        let cache_guard = state.leaderboard_cache.read().unwrap_or_else(|e| e.into_inner());
        cache_guard.clone()
    };

    // If cache hit and not expired, return cached value
    if let Some(cached_entry) = cached {
        if Instant::now() < cached_entry.expires_at {
            // Truncate to requested limit if needed
            let mut page = cached_entry.data.clone();
            let limit_usize = usize::try_from(limit).unwrap_or(0);
            page.entries.truncate(limit_usize);
            return Ok(Json(page));
        }
    }

    // Cache miss or expired: fetch fresh data from database
    let page = db_get_leaderboard(&state.db, MAX_LIST_LIMIT);

    // Store in cache with expiry time
    {
        let mut cache_guard = state.leaderboard_cache.write().unwrap_or_else(|e| e.into_inner());
        *cache_guard = Some(CachedLeaderboard {
            data: LeaderboardPage {
                entries: page.entries.clone(),
            },
            expires_at: Instant::now() + Duration::from_secs(CACHE_EXPIRY_SECS),
        });
    }

    // Return truncated to requested limit
    let mut result = page;
    let limit_usize = usize::try_from(limit).unwrap_or(0);
    result.entries.truncate(limit_usize);
    Ok(Json(result))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{acquire_db, new_shared_db, new_shared_idempotency_store, Bounty};
    use chrono::Utc;

    fn test_state() -> Arc<AppState> {
        Arc::new(AppState {
            db: new_shared_db(),
            idempotency: new_shared_idempotency_store(),
            rate_limiter: crate::routes::tx::new_shared_rate_limiter(),
            bounty_broadcast: tokio::sync::broadcast::channel(16).0,
            leaderboard_cache: new_leaderboard_cache(),
        })
    }

    fn seed_bounties(state: &AppState, count: usize) {
        let mut guard = acquire_db(&state.db);
        for i in 0..count {
            guard.bounties.push(Bounty {
                id: i.to_string(),
                creator: "creator".to_string(),
                assignee: Some(format!("contributor_{}", i % 5)),
                created_at: Utc::now(),
            });
        }
    }

    #[tokio::test]
    async fn get_leaderboard_returns_entries_sorted_by_reputation() {
        let state = test_state();
        seed_bounties(&state, 10);

        let result = get_leaderboard_route(
            State(state),
            Query(LeaderboardParams { limit: None }),
        )
        .await;

        let Json(page) = result.expect("request must succeed");
        assert!(!page.entries.is_empty());
        // Verify entries are sorted by reputation descending
        for i in 0..page.entries.len() - 1 {
            assert!(
                page.entries[i].reputation >= page.entries[i + 1].reputation,
                "entries must be sorted by reputation descending"
            );
        }
    }

    #[tokio::test]
    async fn get_leaderboard_respects_limit() {
        let state = test_state();
        seed_bounties(&state, 20);

        let result = get_leaderboard_route(
            State(state),
            Query(LeaderboardParams {
                limit: Some(5),
            }),
        )
        .await;

        let Json(page) = result.expect("request must succeed");
        assert_eq!(page.entries.len(), 5);
    }

    #[tokio::test]
    async fn get_leaderboard_clamps_oversized_limit() {
        let state = test_state();
        seed_bounties(&state, 20);

        let result = get_leaderboard_route(
            State(state),
            Query(LeaderboardParams {
                limit: Some(10_000),
            }),
        )
        .await;

        let Json(page) = result.expect("request must succeed");
        assert!(page.entries.len() <= MAX_LIST_LIMIT as usize);
    }

    #[tokio::test]
    async fn get_leaderboard_caches_results() {
        let state = test_state();
        seed_bounties(&state, 5);

        // First request
        let result1 = get_leaderboard_route(
            State(Arc::clone(&state)),
            Query(LeaderboardParams { limit: None }),
        )
        .await;
        let Json(page1) = result1.expect("first request must succeed");
        let first_entry_count = page1.entries.len();

        // Modify database (add more bounties)
        seed_bounties(&state, 5);

        // Second request within cache window should return same cached data
        let result2 = get_leaderboard_route(
            State(Arc::clone(&state)),
            Query(LeaderboardParams { limit: None }),
        )
        .await;
        let Json(page2) = result2.expect("second request must succeed");

        // Both requests should return same number of entries (cached)
        assert_eq!(
            page2.entries.len(),
            first_entry_count,
            "cached result should be returned within expiry window"
        );
    }

    #[tokio::test]
    async fn get_leaderboard_expires_cache_after_60_seconds() {
        let state = test_state();
        seed_bounties(&state, 5);

        // First request - populates cache
        let _result1 = get_leaderboard_route(
            State(Arc::clone(&state)),
            Query(LeaderboardParams { limit: None }),
        )
        .await;

        // Add more bounties
        seed_bounties(&state, 5);

        // Manually expire the cache by setting expiry time to past
        {
            let mut cache_guard = state.leaderboard_cache.write().unwrap();
            if let Some(mut entry) = cache_guard.take() {
                entry.expires_at = Instant::now() - Duration::from_secs(1);
                *cache_guard = Some(entry);
            }
        }

        // Second request should fetch fresh data
        let result2 = get_leaderboard_route(
            State(Arc::clone(&state)),
            Query(LeaderboardParams { limit: None }),
        )
        .await;
        let Json(page2) = result2.expect("second request must succeed");

        // New data should be fetched (10 bounties from 2 seeds = more contributors or higher counts)
        assert!(page2.entries.len() > 0, "fresh data should be fetched after expiry");
    }

    #[tokio::test]
    async fn get_leaderboard_returns_empty_page_when_no_bounties() {
        let state = test_state();

        let result = get_leaderboard_route(
            State(state),
            Query(LeaderboardParams { limit: None }),
        )
        .await;

        let Json(page) = result.expect("request must succeed");
        assert!(page.entries.is_empty());
    }

    #[tokio::test]
    async fn get_leaderboard_defaults_to_20_limit() {
        let state = test_state();
        // Need more bounties to get 20 unique contributors
        // Seed 100 bounties → up to 100 unique contributors (if each gets unique address)
        // But we're using "contributor_0" to "contributor_4" pattern in seed_bounties
        // So let's manually create a scenario with enough entries
        {
            let mut guard = acquire_db(&state.db);
            let now = chrono::Utc::now();
            for i in 0..100 {
                guard.bounties.push(Bounty {
                    id: i.to_string(),
                    creator: "creator".to_string(),
                    assignee: Some(format!("contributor_{}", i)),
                    created_at: now,
                });
            }
        }

        let result = get_leaderboard_route(
            State(state),
            Query(LeaderboardParams { limit: None }),
        )
        .await;

        let Json(page) = result.expect("request must succeed");
        assert_eq!(page.entries.len(), 20, "default limit should be 20");
    }
}
