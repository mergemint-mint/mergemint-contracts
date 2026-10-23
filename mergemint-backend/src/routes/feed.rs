use axum::{
    extract::{Query, State},
    http::{header, HeaderValue},
    response::{IntoResponse, Response},
};
use chrono::Utc;
use serde::Deserialize;
use std::sync::Arc;

use crate::{
    db::{list_bounties_export_page, BountySortField, SortOrder},
    routes::tx::AppState,
};

#[derive(Debug, Deserialize)]
pub struct FeedParams {
    pub tag: Option<String>,
}

/// `GET /feed.xml` returns a short-cached Atom feed of the latest open bounties.
pub async fn open_bounties_feed(
    State(state): State<Arc<AppState>>,
    Query(params): Query<FeedParams>,
) -> Response {
    let bounties = list_bounties_export_page(
        &state.db,
        0,
        100,
        None,
        Some("open"),
        params.tag.as_deref(),
        BountySortField::Created,
        SortOrder::Desc,
    );
    let updated = bounties
        .first()
        .map(|bounty| bounty.created_at)
        .unwrap_or_else(Utc::now);
    let mut xml = format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<feed xmlns=\"http://www.w3.org/2005/Atom\"><id>urn:mergemint:open-bounties</id><title>Open MergeMint bounties</title><updated>{}</updated>",
        updated.to_rfc3339()
    );

    for bounty in bounties {
        let token = bounty.reward_token.as_deref().unwrap_or("unknown token");
        xml.push_str(&format!(
            "<entry><id>urn:mergemint:bounty:{}</id><title>Bounty {}</title><updated>{}</updated><summary>Reward: {} {}; Status: {}</summary></entry>",
            escape_xml(&bounty.id),
            escape_xml(&bounty.id),
            bounty.created_at.to_rfc3339(),
            bounty.reward,
            escape_xml(token),
            escape_xml(&bounty.status),
        ));
    }
    xml.push_str("</feed>");

    (
        [
            (
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/atom+xml; charset=utf-8"),
            ),
            (
                header::CACHE_CONTROL,
                HeaderValue::from_static("public, max-age=60"),
            ),
        ],
        xml,
    )
        .into_response()
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
