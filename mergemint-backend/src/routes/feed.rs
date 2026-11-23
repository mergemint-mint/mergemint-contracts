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

/// Maximum number of characters accepted for the `tag` filter.
const MAX_TAG_LEN: usize = 64;

#[derive(Debug, Deserialize)]
pub struct FeedParams {
    pub tag: Option<String>,
}

/// `GET /feed.xml` returns a short-cached Atom feed of the latest open bounties.
pub async fn open_bounties_feed(
    State(state): State<Arc<AppState>>,
    Query(params): Query<FeedParams>,
) -> Response {
    // Bound the tag filter length and reject control characters so the value
    // stays a safe, index-friendly equality filter rather than an unbounded
    // user-controlled string.
    let tag = match params.tag.as_deref() {
        Some(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                None
            } else if trimmed.chars().count() > MAX_TAG_LEN
                || trimmed.chars().any(|c| c.is_control())
            {
                return (
                    axum::http::StatusCode::BAD_REQUEST,
                    "invalid tag filter",
                )
                    .into_response();
            } else {
                Some(trimmed)
            }
        }
        None => None,
    };

    let bounties = list_bounties_export_page(
        &state.db,
        0,
        100,
        None,
        Some("open"),
        tag,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn validate_tag(raw: Option<&str>) -> Result<Option<&str>, ()> {
        match raw {
            Some(value) => {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    Ok(None)
                } else if trimmed.chars().count() > MAX_TAG_LEN
                    || trimmed.chars().any(|c| c.is_control())
                {
                    Err(())
                } else {
                    Ok(Some(trimmed))
                }
            }
            None => Ok(None),
        }
    }

    #[test]
    fn accepts_normal_tag() {
        assert_eq!(validate_tag(Some("rust")), Ok(Some("rust")));
    }

    #[test]
    fn trims_and_treats_blank_as_none() {
        assert_eq!(validate_tag(Some("   ")), Ok(None));
        assert_eq!(validate_tag(Some("  rust  ")), Ok(Some("rust")));
    }

    #[test]
    fn rejects_oversized_tag() {
        let long = "a".repeat(MAX_TAG_LEN + 1);
        assert!(validate_tag(Some(&long)).is_err());
    }

    #[test]
    fn rejects_control_characters() {
        assert!(validate_tag(Some("rust\n")).is_err());
        assert!(validate_tag(Some("rust\0")).is_err());
    }

    #[test]
    fn escape_xml_handles_meta_characters() {
        assert_eq!(escape_xml("<a & b>"), "&lt;a &amp; b&gt;");
    }
}
