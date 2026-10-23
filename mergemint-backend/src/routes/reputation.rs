use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::{IntoParams, ToSchema};

use crate::db::{query_reputation_history, ReputationHistoryEntry};
use crate::routes::{bounties::is_syntactically_valid_address, tx::AppState};

const DEFAULT_LIMIT: usize = 20;
const MAX_LIMIT: usize = 100;

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ReputationHistoryParams {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ReputationHistoryPage {
    pub entries: Vec<ReputationHistoryEntry>,
    pub next_offset: Option<usize>,
}

#[utoipa::path(
    get,
    path = "/contributors/{address}/reputation-history",
    params(
        ("address" = String, Path, description = "Stellar contributor address"),
        ReputationHistoryParams,
    ),
    responses(
        (status = 200, description = "Contributor reputation changes, newest first", body = ReputationHistoryPage),
        (status = 400, description = "Malformed contributor address", body = crate::routes::tx::AppError),
    ),
    tag = "contributors"
)]
pub async fn get_reputation_history(
    State(state): State<Arc<AppState>>,
    Path(address): Path<String>,
    Query(params): Query<ReputationHistoryParams>,
) -> Result<Json<ReputationHistoryPage>, (StatusCode, Json<crate::routes::tx::AppError>)> {
    if !is_syntactically_valid_address(&address) {
        return Err(crate::routes::tx::AppError::bad_request(
            "contributor address is not a syntactically valid Stellar address",
        ));
    }

    let limit = params.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
    let offset = params.offset.unwrap_or(0);
    let mut entries = query_reputation_history(&state.db, &address, offset, limit + 1)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(crate::routes::tx::AppError::internal(error)),
            )
        })?;
    let has_more = entries.len() > limit;
    entries.truncate(limit);

    Ok(Json(ReputationHistoryPage {
        entries,
        next_offset: has_more.then_some(offset.saturating_add(limit)),
    }))
}
