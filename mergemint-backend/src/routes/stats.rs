use axum::{extract::State, Json};
use std::sync::Arc;
use std::time::Duration;

use crate::db::{get_stats, StatsResponse};
use crate::routes::tx::AppState;

const STATS_CACHE_TTL_SECS: u64 = 30;

fn cache_ttl() -> Duration {
    let seconds = std::env::var("STATS_CACHE_TTL_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(STATS_CACHE_TTL_SECS);
    Duration::from_secs(seconds.clamp(1, 3600))
}

/// `GET /stats`
#[utoipa::path(
    get,
    path = "/stats",
    responses(
        (status = 200, description = "Platform totals, with payout amounts separated by reward token", body = StatsResponse),
    ),
    tag = "stats"
)]
pub async fn get_stats_route(State(state): State<Arc<AppState>>) -> Json<StatsResponse> {
    Json(get_stats(&state.db, cache_ttl()))
}
