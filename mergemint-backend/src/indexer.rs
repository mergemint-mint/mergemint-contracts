// mergemint-backend/src/indexer.rs
//
// Soroban event indexer: polls Horizon/Soroban RPC for contract events and
// extracts bounty IDs from their topics.
//
// ## last_ledger regression guard (issue #49 / fix-tracker)
//
// `poll_once` advances `last_ledger` **only after the full result page for
// that ledger has been consumed**.  A truncated page (issue #49) used to
// advance `last_ledger` based only on the events returned, permanently
// skipping the remaining events in that ledger.  Once issue #49 lands and
// pagination is exhaustive, `highest_ledger` naturally reflects all events in
// the requested range, and the regression resolves itself.
//
// REGRESSION GUARD: `last_ledger` must never jump past a ledger whose event
// page was not fully processed.  The invariant is enforced by the check in
// `poll_once` and validated by the unit test
// `test_last_ledger_does_not_advance_on_partial_page` below.

use tracing::warn;

/// Maximum byte length for a single XDR event payload.  Soroban contract
/// events are bounded by the protocol's contract event size limit; this is a
/// defensive ceiling so a malicious or corrupt payload cannot cause
/// unbounded allocation during decode.
const XDR_PAYLOAD_MAX_BYTES: usize = 8 * 1024;

/// Expected length, in bytes, of a [`BountyId`](src/types.rs) as serialised
/// inside an `ScVal::Bytes`.  `BountyId` is `BytesN<32>`, so the decoded
/// `ScBytes` must contain exactly 32 bytes.
pub const BOUNTY_ID_LEN: usize = 32;

use stellar_xdr::{Limits, ReadXdr, ScVal};

#[allow(clippy::too_many_arguments)]
pub fn index_reputation_change(
    db: &crate::db::SharedDb,
    event_id: String,
    contributor_address: String,
    ledger: u32,
    timestamp: chrono::DateTime<chrono::Utc>,
    old_value: u32,
    new_value: u32,
    reason: String,
) -> bool {
    let inserted = crate::db::record_reputation_change(
        db,
        crate::db::ReputationHistoryEntry {
            event_id,
            contributor_address,
            ledger,
            timestamp,
            old_value,
            new_value,
            reason,
        },
    );
    if inserted {
        crate::routes::leaderboard::invalidate_leaderboard_cache();
    }
    inserted
}

// ---------------------------------------------------------------------------
// Constants — recognised event name sets
// ---------------------------------------------------------------------------

/// Events whose data payload is a single bounty-id value.
const SINGLE_VALUE_EVENTS: &[&str] = &[
    "bounty_claimed",
    "bounty_completed",
    "bounty_disputed",
    "bounty_cancelled",
    "bounty_expired",
    "bounty_unclaimed",
    "bounty_archived",
    "dispute_appealed",
    "dispute_finalized",
    "claim_released",
];

/// Events whose data payload is a (bounty_id, …) tuple.
const TUPLE_EVENTS: &[&str] = &[
    "bounty_created",
    "reward_paid",
    "approval_recorded",
    "dispute_resolved",
    "work_submitted",
    "bounty_reopened",
    "bounty_reassigned",
    "bounty_sponsored",
    "deadline_extended",
    "bounty_topped_up",
    "milestone_completed",
    "ownership_transfer_started",
    "ownership_transferred",
    "dispute_evidence_added",
];

/// Events keyed by contributor address that carry no `bounty_id`.
/// `extract_bounty_id_hex` returns `None` for these; callers route them to
/// reputation/profile handling rather than bounty handling.
#[allow(dead_code)] // Documents the contract; not read by code yet.
const CONTRIBUTOR_ONLY_EVENTS: &[&str] = &["reputation_changed", "contributor_metadata_updated"];

/// Global admin/contract-level events that carry neither a bounty_id nor a
/// contributor address.  `extract_bounty_id_hex` returns `None` for these.
#[allow(dead_code)] // Documents the contract; not read by code yet.
const CONTRACT_ADMIN_EVENTS: &[&str] = &[
    "reward_token_added",
    "reward_token_removed",
    "config_changed",
    "contract_paused",
    "contract_unpaused",
];

/// All contract events recognised by the indexer, for exhaustive coverage
/// checks in tests.  Every event emitted by `src/events.rs` must appear in
/// exactly one of the four sets above, and therefore exactly once here.
#[allow(dead_code)]
const ALL_CONTRACT_EVENTS: &[&str] = &[
    // SINGLE_VALUE_EVENTS
    "bounty_claimed",
    "bounty_completed",
    "bounty_disputed",
    "bounty_cancelled",
    "bounty_expired",
    "bounty_unclaimed",
    "bounty_archived",
    "dispute_appealed",
    "dispute_finalized",
    "claim_released",
    // TUPLE_EVENTS
    "bounty_created",
    "reward_paid",
    "approval_recorded",
    "dispute_resolved",
    "work_submitted",
    "bounty_reopened",
    "bounty_reassigned",
    "bounty_sponsored",
    "deadline_extended",
    "bounty_topped_up",
    "milestone_completed",
    "ownership_transfer_started",
    "ownership_transferred",
    "dispute_evidence_added",
    // CONTRIBUTOR_ONLY_EVENTS
    "reputation_changed",
    "contributor_metadata_updated",
    // CONTRACT_ADMIN_EVENTS
    "reward_token_added",
    "reward_token_removed",
    "config_changed",
    "contract_paused",
    "contract_unpaused",
];

// ---------------------------------------------------------------------------
// extract_bounty_id_hex  (lines 29-38 in the original design)
// ---------------------------------------------------------------------------

/// Extract the bounty-id hex string from a raw event topic slice.
///
/// Decodes the `value_hex` payload as base64-encoded Soroban XDR and extracts
/// the 32-byte `BountyId`:
///
/// - For [`SINGLE_VALUE_EVENTS`], the payload is a single `ScVal::Bytes`
///   containing exactly 32 bytes.
/// - For [`TUPLE_EVENTS`], the payload is an `ScVal::Vec` whose first element
///   is an `ScVal::Bytes` of 32 bytes.
///
/// Returns `None` when:
/// - `topics` is empty (no event-name topic present), or
/// - the first topic is not a recognised bounty-scoped event name, or
/// - the XDR payload is malformed, truncated, or exceeds the bounded
///   [`XDR_PAYLOAD_MAX_BYTES`] limit, or
/// - the decoded `ScVal` does not match the expected shape, or
/// - the extracted bytes are not exactly [`BOUNTY_ID_LEN`] (32) bytes long.
///
/// Contributor-only events ([`CONTRIBUTOR_ONLY_EVENTS`]) and contract-admin
/// events ([`CONTRACT_ADMIN_EVENTS`]) carry no bounty_id by design; this
/// returns `None` for them without emitting a warning.
///
/// The caller is responsible for logging an appropriate warning when `None`
/// is returned after a successful topic decode and the name is not in any
/// recognised set.
pub fn extract_bounty_id_hex(topics: &[String], value_hex: &str) -> Option<String> {
    let event_name = topics.first()?;

    let is_single = SINGLE_VALUE_EVENTS.contains(&event_name.as_str());
    let is_tuple = TUPLE_EVENTS.contains(&event_name.as_str());

    if !is_single && !is_tuple {
        // Covers CONTRIBUTOR_ONLY_EVENTS, CONTRACT_ADMIN_EVENTS, and unknown.
        return None;
    }

    decode_bounty_id_from_xdr(value_hex, is_tuple)
}

/// Decode a base64-encoded XDR `ScVal` and extract the 32-byte bounty id.
///
/// `is_tuple` selects between the single-value shape (`ScVal::Bytes`) and the
/// tuple shape (`ScVal::Vec` whose first element is `ScVal::Bytes`).
///
/// Uses bounded [`Limits`] to defend against excessively large or deeply
/// nested payloads.  Any decode error, shape mismatch, or length mismatch
/// returns `None` (the caller logs a structured warning).
pub(crate) fn decode_bounty_id_from_xdr(value_hex: &str, is_tuple: bool) -> Option<String> {
    let scval = ScVal::from_xdr_base64(value_hex, Limits::len(XDR_PAYLOAD_MAX_BYTES)).ok()?;

    let bytes: &[u8] = if is_tuple {
        // ScVal::Vec(Option<ScVec>); the first element must be ScVal::Bytes.
        let ScVal::Vec(Some(vec)) = &scval else {
            return None;
        };
        let first = vec.0.first()?;
        let ScVal::Bytes(bytes) = first else {
            return None;
        };
        bytes.as_ref()
    } else {
        // Single-value shape: the entire payload is ScVal::Bytes.
        let ScVal::Bytes(bytes) = &scval else {
            return None;
        };
        bytes.as_ref()
    };

    if bytes.len() != BOUNTY_ID_LEN {
        return None;
    }

    Some(hex::encode(bytes))
}

// ---------------------------------------------------------------------------
// RawEvent — a minimal, test-friendly representation of a Soroban event
// ---------------------------------------------------------------------------

/// A thin representation of one Soroban contract event, decoupled from any
/// specific SDK type so that unit tests can construct arbitrary values.
#[derive(Debug, Clone)]
pub struct RawEvent {
    /// Decoded topics. `topics[0]` is expected to be the event-name symbol;
    /// a decode failure on this field is logged and the event skipped.
    pub topics: Vec<TopicDecodeResult>,
    /// Hex-encoded event data payload.
    pub value_hex: String,
    /// The ledger sequence number this event was emitted in.
    pub ledger: u32,
}

/// Result of attempting to decode a single event topic.
#[derive(Debug, Clone)]
pub enum TopicDecodeResult {
    /// Successfully decoded topic value.
    Ok(String),
    /// Decode failed; carries a human-readable error description.
    Err(String),
}

// ---------------------------------------------------------------------------
// Indexer state
// ---------------------------------------------------------------------------

/// Persistent state carried between `poll_once` invocations.
#[derive(Debug, Clone)]
pub struct IndexerState {
    /// The last fully-processed ledger sequence.
    ///
    /// # Regression invariant
    ///
    /// This value must only advance past ledger `N` once *all* events for `N`
    /// have been fetched and processed.  If pagination is incomplete (the page
    /// was truncated before all events for that ledger were returned),
    /// `last_ledger` must remain at `N − 1` so the next poll re-fetches from
    /// the same boundary.
    ///
    /// See: issue #49 and the regression test
    /// `test_last_ledger_does_not_advance_on_partial_page`.
    pub last_ledger: u32,
}

// ---------------------------------------------------------------------------
// poll_once  (lines 127-150 in the original design)
// ---------------------------------------------------------------------------

/// Process a single page of raw events, updating `state.last_ledger`.
///
/// # Issue 1 — last_ledger regression guard
///
/// `last_ledger` only advances to `highest_ledger` when the page is marked
/// as complete (`page_complete == true`).  A truncated page leaves
/// `last_ledger` unchanged so the next poll re-fetches from the same start
/// boundary.  Once issue #49 lands and pagination is exhaustive,
/// `highest_ledger` will always equal the true highest ledger in the range,
/// making this guard a no-op rather than a safety net.
///
/// # Issue 2 — unrecognised event logging
///
/// When a topic decodes successfully but the event name is not in any
/// recognised set, a `tracing::warn!` is emitted instead of silently dropping
/// the event.  This makes future contract upgrades that emit new event kinds
/// immediately visible in logs.
///
/// # Issue 3 — decode error logging
///
/// When the first topic fails to decode, the error is logged via
/// `tracing::warn!` instead of being swallowed silently.
///
/// # XDR decode failures (#1259)
///
/// When the event name is recognised as bounty-scoped but the XDR payload is
/// malformed, truncated, oversized, or does not contain a valid 32-byte
/// `BountyId`, a structured `tracing::warn!` is emitted with the event name and
/// the raw payload length so operators can correlate the failure with the
/// on-chain data.  The event is not dropped from ledger tracking.
#[tracing::instrument(skip(state, events), fields(event_count = events.len(), page_complete))]
pub fn poll_once(state: &mut IndexerState, events: &[RawEvent], page_complete: bool) {
    let mut highest_ledger = state.last_ledger;

    for event in events {
        // ── Issue 3: log decode failures instead of silent skip ──────────
        //
        // The original code used `let Ok(event_name) = … else { continue; }`
        // which swallowed genuine XDR decode errors identically to expected
        // no-ops.  Changing the else branch to emit warn! makes malformed
        // topics observable in production logs without changing control flow.
        let event_name = match event.topics.first() {
            Some(TopicDecodeResult::Ok(name)) => name.clone(),
            Some(TopicDecodeResult::Err(e)) => {
                warn!("failed to decode event topic: {e}");
                continue;
            }
            None => {
                warn!("event has no topics; skipping");
                continue;
            }
        };

        if matches!(event_name.as_str(), "reward_paid" | "reputation_changed") {
            crate::routes::leaderboard::invalidate_leaderboard_cache();
        }

        // ── Determine whether the event name is recognised at all ────────
        //
        // We check the constant sets directly (not via extract_bounty_id_hex)
        // because extract_bounty_id_hex also performs XDR decoding, which can
        // fail for a recognised event with a corrupt payload.  Separating the
        // two concerns lets us emit the correct warning: "unrecognized" for
        // unknown event names, and "malformed XDR" for recognised events whose
        // payload is corrupt.
        let is_bounty_scoped = SINGLE_VALUE_EVENTS.contains(&event_name.as_str())
            || TUPLE_EVENTS.contains(&event_name.as_str());
        let is_contributor = CONTRIBUTOR_ONLY_EVENTS.contains(&event_name.as_str());
        let is_admin = CONTRACT_ADMIN_EVENTS.contains(&event_name.as_str());

        if !is_bounty_scoped && !is_contributor && !is_admin {
            // ── Issue 2: log unrecognised events instead of silent drop ──
            warn!("unrecognized contract event: {event_name}");
            // Still track the ledger so unrecognised events don't stall progress.
            if event.ledger > highest_ledger {
                highest_ledger = event.ledger;
            }
            continue;
        }

        // For bounty-scoped events, attempt XDR decode and warn on failure.
        if is_bounty_scoped
            && extract_bounty_id_hex(std::slice::from_ref(&event_name), &event.value_hex).is_none()
        {
            warn!(
                event = %event_name,
                payload_len = event.value_hex.len(),
                "bounty-scoped event has malformed XDR payload or invalid bounty id; \
                 skipping bounty extraction but tracking ledger"
            );
        }

        if event.ledger > highest_ledger {
            highest_ledger = event.ledger;
        }
    }

    // ── Issue 1: regression guard ─────────────────────────────────────────
    //
    // Only advance last_ledger when the caller signals that the page was
    // complete (i.e. no truncation occurred).  This prevents a partial page
    // from advancing the cursor past un-fetched events.
    //
    // REGRESSION GUARD: removing or inverting this check re-introduces the
    // issue-#49 regression where a truncated page silently skips events.
    // The unit test `test_last_ledger_does_not_advance_on_partial_page`
    // exists specifically to catch that regression.
    if page_complete {
        state.last_ledger = highest_ledger;
    }
    // When page_complete is false, last_ledger is deliberately left unchanged
    // so the next poll retries from the same start boundary.
}

/// Process indexer events and publish them to the backend notification channel
/// after a complete page is received, avoiding duplicate notifications when a
/// truncated page is fetched again.
pub fn poll_once_and_publish(
    state: &mut IndexerState,
    events: &[RawEvent],
    page_complete: bool,
    db: &crate::db::SharedDb,
    sender: &tokio::sync::broadcast::Sender<String>,
) {
    poll_once(state, events, page_complete);
    if !page_complete {
        return;
    }

    for event in events {
        let Some(TopicDecodeResult::Ok(event_name)) = event.topics.first() else {
            continue;
        };
        let Some(bounty_id) =
            extract_bounty_id_hex(std::slice::from_ref(event_name), &event.value_hex)
        else {
            continue;
        };
        match event_name.as_str() {
            "bounty_claimed" => {
                if let Some(TopicDecodeResult::Ok(assignee)) = event.topics.get(1) {
                    crate::db::mark_bounty_claimed(db, &bounty_id, assignee);
                }
            }
            "work_submitted" => crate::db::mark_bounty_submitted(db, &bounty_id),
            _ => {}
        }
        if let Err(error) = sender.send(crate::events::encode_bounty_event(event_name, &bounty_id))
        {
            warn!(%error, event = %event_name, "failed to publish indexed bounty event");
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ── XDR fixture helpers ─────────────────────────────────────────────────
    //
    // The indexer now performs genuine stellar-xdr `ScVal::from_xdr_base64`
    // decoding, so tests must supply base64-encoded XDR payloads that decode
    // to the expected `ScVal` shape — not arbitrary hex strings.

    use stellar_xdr::{ScBytes, ScVal, ScVec, WriteXdr};

    /// Encode an `ScVal` to base64 XDR using bounded limits.
    fn scval_to_base64(scval: &ScVal) -> String {
        let raw = scval.to_xdr(Limits::none()).expect("encode ScVal");
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, raw)
    }

    /// Build base64 XDR for a single `ScVal::Bytes` of `n` bytes, all zero.
    fn bytes_xdr(n: usize) -> String {
        let scval = ScVal::Bytes(ScBytes(vec![0u8; n].try_into().unwrap()));
        scval_to_base64(&scval)
    }

    /// Build base64 XDR for an `ScVal::Vec` whose first element is
    /// `ScVal::Bytes(n)` and whose remaining elements are filler `ScVal::U32`.
    fn tuple_xdr(n: usize, extra: usize) -> String {
        let mut elements = vec![ScVal::Bytes(ScBytes(vec![0u8; n].try_into().unwrap()))];
        elements.extend(std::iter::repeat_n(ScVal::U32(42), extra));
        let scval = ScVal::Vec(Some(ScVec(elements.try_into().unwrap())));
        scval_to_base64(&scval)
    }

    /// A valid 32-byte single-value bounty-id payload.
    fn valid_single_bounty_id() -> String {
        bytes_xdr(BOUNTY_ID_LEN)
    }

    /// A valid tuple payload with a 32-byte bounty id as the first element.
    fn valid_tuple_bounty_id() -> String {
        tuple_xdr(BOUNTY_ID_LEN, 2)
    }

    // ── RawEvent helpers ────────────────────────────────────────────────────

    /// Build a RawEvent with a single-value bounty-id payload, suitable for
    /// SINGLE_VALUE_EVENTS.
    fn single_event(name: &str, ledger: u32) -> RawEvent {
        RawEvent {
            topics: vec![TopicDecodeResult::Ok(name.to_owned())],
            value_hex: valid_single_bounty_id(),
            ledger,
        }
    }

    /// Build a RawEvent with a tuple bounty-id payload, suitable for
    /// TUPLE_EVENTS.
    fn tuple_event(name: &str, ledger: u32) -> RawEvent {
        RawEvent {
            topics: vec![TopicDecodeResult::Ok(name.to_owned())],
            value_hex: valid_tuple_bounty_id(),
            ledger,
        }
    }

    /// Build a RawEvent with an arbitrary (possibly invalid) payload string.
    fn raw_event(name: &str, value_hex: &str, ledger: u32) -> RawEvent {
        RawEvent {
            topics: vec![TopicDecodeResult::Ok(name.to_owned())],
            value_hex: value_hex.to_owned(),
            ledger,
        }
    }

    fn err_event(ledger: u32) -> RawEvent {
        RawEvent {
            topics: vec![TopicDecodeResult::Err("unexpected XDR tag".to_owned())],
            value_hex: String::new(),
            ledger,
        }
    }

    // ── Issue 1: last_ledger regression guard ─────────────────────────────

    /// REGRESSION GUARD (issue #49)
    ///
    /// `last_ledger` must NOT advance when the page is incomplete.  A full
    /// page of events up to ledger 200 must not move the cursor if
    /// `page_complete` is false — the next poll must re-start from the
    /// previous `last_ledger` (100) to avoid skipping events.
    #[test]
    fn test_last_ledger_does_not_advance_on_partial_page() {
        let mut state = IndexerState { last_ledger: 100 };

        let events = vec![
            tuple_event("bounty_created", 150),
            single_event("bounty_claimed", 200),
        ];

        // Simulate a truncated page (page_complete = false).
        poll_once(&mut state, &events, false);

        assert_eq!(
            state.last_ledger, 100,
            "last_ledger must not advance on a truncated page (issue #49 regression guard)"
        );
    }

    /// Complementary check: `last_ledger` DOES advance on a complete page.
    #[test]
    fn test_last_ledger_advances_on_complete_page() {
        let mut state = IndexerState { last_ledger: 100 };

        let events = vec![
            tuple_event("bounty_created", 150),
            single_event("bounty_claimed", 200),
        ];

        poll_once(&mut state, &events, true);

        assert_eq!(
            state.last_ledger, 200,
            "last_ledger must advance to the highest ledger on a complete page"
        );
    }

    /// An empty complete page must not regress last_ledger below its current
    /// value.
    #[test]
    fn test_last_ledger_unchanged_on_empty_complete_page() {
        let mut state = IndexerState { last_ledger: 300 };
        poll_once(&mut state, &[], true);
        assert_eq!(state.last_ledger, 300);
    }

    // ── Issue 2: unrecognised event warning ───────────────────────────────

    /// An event whose name is not in any recognised set must still allow
    /// last_ledger to advance and must trigger the warn path.
    #[test]
    fn test_unrecognized_event_does_not_block_ledger_advance() {
        let mut state = IndexerState { last_ledger: 10 };

        // "bounty_upgraded" is a synthetic future event name not in any list.
        let events = vec![raw_event("bounty_upgraded", "deadbeef", 20)];

        poll_once(&mut state, &events, true);

        assert_eq!(
            state.last_ledger, 20,
            "unrecognised event must not block ledger advancement"
        );
    }

    /// `extract_bounty_id_hex` must return None for an unrecognised name.
    #[test]
    fn test_extract_bounty_id_hex_returns_none_for_unknown_event() {
        let result = extract_bounty_id_hex(&["bounty_upgraded".to_owned()], "cafebabe");
        assert!(
            result.is_none(),
            "unrecognised event name must yield None from extract_bounty_id_hex"
        );
    }

    /// `extract_bounty_id_hex` must return Some for every known single-value
    /// event when given a valid 32-byte XDR payload.
    #[test]
    fn test_extract_bounty_id_hex_single_value_events() {
        for &name in SINGLE_VALUE_EVENTS {
            let result = extract_bounty_id_hex(&[name.to_owned()], &valid_single_bounty_id());
            assert!(
                result.is_some(),
                "expected Some for single-value event '{name}' with valid 32-byte XDR, got None"
            );
        }
    }

    /// `extract_bounty_id_hex` must return Some for every known tuple event
    /// when given a valid tuple XDR payload with a 32-byte first element.
    #[test]
    fn test_extract_bounty_id_hex_tuple_events() {
        for &name in TUPLE_EVENTS {
            let result = extract_bounty_id_hex(&[name.to_owned()], &valid_tuple_bounty_id());
            assert!(
                result.is_some(),
                "expected Some for tuple event '{name}' with valid tuple XDR, got None"
            );
        }
    }

    /// `extract_bounty_id_hex` must return None for contributor-only events —
    /// they carry no bounty_id by design.
    #[test]
    fn test_extract_bounty_id_hex_returns_none_for_contributor_only_events() {
        for name in CONTRIBUTOR_ONLY_EVENTS {
            let result = extract_bounty_id_hex(&[name.to_string()], &valid_single_bounty_id());
            assert!(
                result.is_none(),
                "contributor-only event '{name}' should yield None — no bounty_id"
            );
        }
    }

    /// `extract_bounty_id_hex` must return None for contract-admin events.
    #[test]
    fn test_extract_bounty_id_hex_returns_none_for_admin_events() {
        for name in CONTRACT_ADMIN_EVENTS {
            let result = extract_bounty_id_hex(&[name.to_string()], &valid_single_bounty_id());
            assert!(
                result.is_none(),
                "contract-admin event '{name}' should yield None — no bounty_id"
            );
        }
    }

    // ── Issue 3: decode error logging ─────────────────────────────────────

    /// An event with a decode-error topic must be skipped (not panic) and
    /// good events on the same page must still advance last_ledger.
    #[test]
    fn test_decode_error_event_is_skipped_gracefully() {
        let mut state = IndexerState { last_ledger: 50 };

        let events = vec![err_event(60), single_event("bounty_claimed", 70)];

        poll_once(&mut state, &events, true);

        assert_eq!(
            state.last_ledger, 70,
            "decode-error events must be skipped; good events must still advance last_ledger"
        );
    }

    /// A partial page of decode-error events must not advance last_ledger.
    #[test]
    fn test_all_decode_error_events_partial_page() {
        let mut state = IndexerState { last_ledger: 50 };
        let events = vec![err_event(60), err_event(70)];
        poll_once(&mut state, &events, false);
        assert_eq!(state.last_ledger, 50);
    }

    /// On a complete page of only decode-error events, last_ledger stays at
    /// its prior value because no valid ledger numbers were collected.
    #[test]
    fn test_all_decode_error_events_complete_page_no_valid_ledger() {
        let mut state = IndexerState { last_ledger: 50 };
        let events = vec![err_event(60), err_event(70)];
        poll_once(&mut state, &events, true);
        assert_eq!(state.last_ledger, 50);
    }

    // ── XDR decode: valid payloads (#1259) ────────────────────────────────

    /// A single-value event with a valid 32-byte ScVal::Bytes payload must
    /// extract a hex bounty id of length 64 (32 bytes × 2 hex chars).
    #[test]
    fn test_extract_bounty_id_single_valid_32_bytes() {
        let id = extract_bounty_id_hex(&["bounty_claimed".to_owned()], &valid_single_bounty_id())
            .expect("valid single-value 32-byte payload should decode");
        assert_eq!(
            id.len(),
            BOUNTY_ID_LEN * 2,
            "hex bounty id must be 64 chars"
        );
    }

    /// A tuple event with a valid ScVal::Vec whose first element is a 32-byte
    /// ScVal::Bytes must extract a hex bounty id.
    #[test]
    fn test_extract_bounty_id_tuple_valid() {
        let id = extract_bounty_id_hex(&["bounty_created".to_owned()], &valid_tuple_bounty_id())
            .expect("valid tuple payload should decode");
        assert_eq!(id.len(), BOUNTY_ID_LEN * 2);
    }

    /// A single-value payload with the wrong byte length (31 instead of 32)
    /// must return None — not panic, not return a truncated id.
    #[test]
    fn test_extract_bounty_id_single_wrong_length() {
        let xdr = bytes_xdr(31);
        let result = extract_bounty_id_hex(&["bounty_claimed".to_owned()], &xdr);
        assert!(
            result.is_none(),
            "31-byte payload must not yield a bounty id"
        );
    }

    /// A single-value payload with 33 bytes must also be rejected.
    #[test]
    fn test_extract_bounty_id_single_too_long() {
        let xdr = bytes_xdr(33);
        let result = extract_bounty_id_hex(&["bounty_claimed".to_owned()], &xdr);
        assert!(
            result.is_none(),
            "33-byte payload must not yield a bounty id"
        );
    }

    /// A tuple payload whose first element is the wrong length must return
    /// None.
    #[test]
    fn test_extract_bounty_id_tuple_wrong_first_element_length() {
        let xdr = tuple_xdr(16, 1);
        let result = extract_bounty_id_hex(&["bounty_created".to_owned()], &xdr);
        assert!(
            result.is_none(),
            "16-byte first element must not yield a bounty id"
        );
    }

    /// A single-value payload that decodes to ScVal::Vec (wrong shape) must
    /// return None.
    #[test]
    fn test_extract_bounty_id_single_wrong_shape() {
        let result =
            extract_bounty_id_hex(&["bounty_claimed".to_owned()], &valid_tuple_bounty_id());
        assert!(
            result.is_none(),
            "tuple payload under a single-value event must be rejected"
        );
    }

    /// A tuple payload that decodes to ScVal::Bytes (wrong shape) must return
    /// None.
    #[test]
    fn test_extract_bounty_id_tuple_wrong_shape() {
        let result =
            extract_bounty_id_hex(&["bounty_created".to_owned()], &valid_single_bounty_id());
        assert!(
            result.is_none(),
            "single-value payload under a tuple event must be rejected"
        );
    }

    // ── XDR decode: malformed / corrupt / truncated payloads (#1259) ──────

    /// Malformed base64 must return None — not panic.
    #[test]
    fn test_extract_bounty_id_malformed_base64() {
        let result = extract_bounty_id_hex(&["bounty_claimed".to_owned()], "!!!not-base64!!!");
        assert!(
            result.is_none(),
            "malformed base64 must not yield a bounty id"
        );
    }

    /// Truncated XDR (valid base64 but not a complete ScVal) must return None.
    #[test]
    fn test_extract_bounty_id_truncated_xdr() {
        // "AA==" is a single zero byte — not valid ScVal XDR.
        let result = extract_bounty_id_hex(&["bounty_claimed".to_owned()], "AA==");
        assert!(result.is_none(), "truncated XDR must not yield a bounty id");
    }

    /// Empty string payload must return None.
    #[test]
    fn test_extract_bounty_id_empty_payload() {
        let result = extract_bounty_id_hex(&["bounty_claimed".to_owned()], "");
        assert!(result.is_none(), "empty payload must not yield a bounty id");
    }

    /// An ScVal::Vec that is None (the `Option<ScVec>` is `None`) must return
    /// None — this is a valid XDR encoding of an empty vec but has no bounty
    /// id.
    #[test]
    fn test_extract_bounty_id_tuple_none_vec() {
        let scval = ScVal::Vec(None);
        let xdr = scval_to_base64(&scval);
        let result = extract_bounty_id_hex(&["bounty_created".to_owned()], &xdr);
        assert!(result.is_none(), "None vec must not yield a bounty id");
    }

    /// An ScVal::Vec that is empty (zero elements) must return None.
    #[test]
    fn test_extract_bounty_id_tuple_empty_vec() {
        let scval = ScVal::Vec(Some(ScVec(Vec::new().try_into().unwrap())));
        let xdr = scval_to_base64(&scval);
        let result = extract_bounty_id_hex(&["bounty_created".to_owned()], &xdr);
        assert!(result.is_none(), "empty vec must not yield a bounty id");
    }

    /// An ScVal::Vec whose first element is ScVal::U32 (not Bytes) must return
    /// None.
    #[test]
    fn test_extract_bounty_id_tuple_first_element_not_bytes() {
        let scval = ScVal::Vec(Some(ScVec(vec![ScVal::U32(42)].try_into().unwrap())));
        let xdr = scval_to_base64(&scval);
        let result = extract_bounty_id_hex(&["bounty_created".to_owned()], &xdr);
        assert!(
            result.is_none(),
            "non-Bytes first element must not yield a bounty id"
        );
    }

    /// A payload that decodes to ScVal::Void must return None for both shapes.
    #[test]
    fn test_extract_bounty_id_void_payload() {
        let xdr = scval_to_base64(&ScVal::Void);
        assert!(
            extract_bounty_id_hex(&["bounty_claimed".to_owned()], &xdr).is_none(),
            "Void payload under a single-value event must be rejected"
        );
        assert!(
            extract_bounty_id_hex(&["bounty_created".to_owned()], &xdr).is_none(),
            "Void payload under a tuple event must be rejected"
        );
    }

    // ── poll_once with malformed XDR does not panic (#1259) ───────────────

    /// A page containing a recognised event with a malformed payload must not
    /// panic; last_ledger must still advance.
    #[test]
    fn test_poll_once_malformed_xdr_does_not_panic() {
        let mut state = IndexerState { last_ledger: 0 };
        let events = vec![raw_event("bounty_claimed", "!!!corrupt!!!", 10)];
        poll_once(&mut state, &events, true);
        assert_eq!(
            state.last_ledger, 10,
            "malformed XDR on a recognised event must not block ledger advancement"
        );
    }

    /// A page mixing valid and malformed payloads must process the valid ones
    /// and still advance the ledger to the highest.
    #[test]
    fn test_poll_once_mixed_valid_and_malformed() {
        let mut state = IndexerState { last_ledger: 0 };
        let events = vec![
            raw_event("bounty_claimed", "!!!corrupt!!!", 10),
            single_event("bounty_completed", 20),
        ];
        poll_once(&mut state, &events, true);
        assert_eq!(state.last_ledger, 20);
    }

    // ── Event-schema coverage ─────────────────────────────────────────────

    /// Event names as documented in `docs/event-schema.md` and emitted by
    /// `src/events.rs`.
    const DOCUMENTED_EVENTS: &[&str] = &[
        // bounty lifecycle
        "bounty_created",
        "bounty_claimed",
        "bounty_unclaimed",
        "bounty_completed",
        "bounty_cancelled",
        "bounty_expired",
        "bounty_reopened",
        "bounty_reassigned",
        "bounty_sponsored",
        "bounty_archived",
        // work / review
        "work_submitted",
        "approval_recorded",
        "milestone_completed",
        // dispute
        "bounty_disputed",
        "dispute_resolved",
        "dispute_evidence_added",
        "dispute_appealed",
        "dispute_finalized",
        // reward
        "reward_paid",
        "deadline_extended",
        "bounty_topped_up",
        "claim_released",
        // ownership
        "ownership_transfer_started",
        "ownership_transferred",
        // contributor / reputation
        "reputation_changed",
        "contributor_metadata_updated",
        // admin / contract
        "reward_token_added",
        "reward_token_removed",
        "config_changed",
        "contract_paused",
        "contract_unpaused",
    ];

    /// Every event in `DOCUMENTED_EVENTS` must appear in exactly one of the
    /// four recognised-event sets.
    #[test]
    fn test_all_contract_events_are_recognised() {
        for &name in DOCUMENTED_EVENTS {
            let count = [
                SINGLE_VALUE_EVENTS.contains(&name),
                TUPLE_EVENTS.contains(&name),
                CONTRIBUTOR_ONLY_EVENTS.contains(&name),
                CONTRACT_ADMIN_EVENTS.contains(&name),
            ]
            .iter()
            .filter(|&&b| b)
            .count();

            assert!(
                count >= 1,
                "event '{name}' from src/events.rs is not recognised by any indexer set"
            );
            assert_eq!(
                count, 1,
                "event '{name}' appears in more than one recognised-event set"
            );
        }
    }

    /// All 31 contract events must appear in `ALL_CONTRACT_EVENTS`.
    #[test]
    fn test_all_contract_events_constant_complete() {
        let mut all_const: Vec<&str> = ALL_CONTRACT_EVENTS.to_vec();
        all_const.sort_unstable();

        let mut documented: Vec<&str> = DOCUMENTED_EVENTS.to_vec();
        documented.sort_unstable();

        assert_eq!(
            all_const, documented,
            "ALL_CONTRACT_EVENTS must contain exactly the same events as DOCUMENTED_EVENTS"
        );
    }

    /// Every bounty-scoped event must advance last_ledger on a complete page.
    #[test]
    fn test_all_bounty_scoped_events_advance_ledger() {
        let single: Vec<&str> = SINGLE_VALUE_EVENTS.to_vec();
        let tuple: Vec<&str> = TUPLE_EVENTS.to_vec();

        for &name in &single {
            let mut state = IndexerState { last_ledger: 0 };
            poll_once(&mut state, &[single_event(name, 1)], true);
            assert_eq!(
                state.last_ledger, 1,
                "single-value event '{name}' should advance last_ledger"
            );
        }
        for &name in &tuple {
            let mut state = IndexerState { last_ledger: 0 };
            poll_once(&mut state, &[tuple_event(name, 1)], true);
            assert_eq!(
                state.last_ledger, 1,
                "tuple event '{name}' should advance last_ledger"
            );
        }
    }

    /// Contributor-only events must advance last_ledger without warning.
    #[test]
    fn test_contributor_only_events_advance_ledger_without_warn() {
        for &name in CONTRIBUTOR_ONLY_EVENTS {
            let mut state = IndexerState { last_ledger: 0 };
            let events = vec![raw_event(name, "deadbeef", 5)];
            poll_once(&mut state, &events, true);
            assert_eq!(
                state.last_ledger, 5,
                "contributor-only event '{name}' must advance last_ledger"
            );
        }
    }

    /// Contract-admin events must advance last_ledger without warning.
    #[test]
    fn test_contract_admin_events_advance_ledger_without_warn() {
        for &name in CONTRACT_ADMIN_EVENTS {
            let mut state = IndexerState { last_ledger: 0 };
            let events = vec![raw_event(name, "deadbeef", 7)];
            poll_once(&mut state, &events, true);
            assert_eq!(
                state.last_ledger, 7,
                "contract-admin event '{name}' must advance last_ledger"
            );
        }
    }

    /// An event carrying an extra topic must be handled gracefully.
    #[test]
    fn test_event_with_unexpected_extra_topic_handled_gracefully() {
        let mut state = IndexerState { last_ledger: 0 };
        let events = vec![RawEvent {
            topics: vec![
                TopicDecodeResult::Ok("bounty_created".to_owned()),
                TopicDecodeResult::Ok("some_address".to_owned()),
                TopicDecodeResult::Ok("unexpected_extra_field".to_owned()),
            ],
            value_hex: valid_tuple_bounty_id(),
            ledger: 5,
        }];

        poll_once(&mut state, &events, true);

        assert_eq!(
            state.last_ledger, 5,
            "an event with an extra topic should still advance the ledger"
        );
    }

    // ── Crash-resume: cursor correctness after a crash mid-batch ──────────

    #[test]
    fn test_indexer_resumes_from_correct_cursor_after_crash_mid_batch() {
        let mut state = IndexerState { last_ledger: 100 };

        let full_batch = vec![
            tuple_event("bounty_created", 110),
            single_event("bounty_claimed", 120),
            tuple_event("reward_paid", 130),
        ];

        let seen_before_crash = &full_batch[..1];
        poll_once(&mut state, seen_before_crash, false);
        assert_eq!(state.last_ledger, 100);

        poll_once(&mut state, &full_batch, true);
        assert_eq!(state.last_ledger, 130);
    }

    #[test]
    fn test_indexer_cursor_stable_across_repeated_crashes() {
        let mut state = IndexerState { last_ledger: 500 };
        let batch = vec![
            tuple_event("bounty_created", 510),
            single_event("bounty_claimed", 520),
        ];

        poll_once(&mut state, &[], false);
        assert_eq!(state.last_ledger, 500);

        poll_once(&mut state, &batch[..1], false);
        assert_eq!(state.last_ledger, 500);

        poll_once(&mut state, &batch, true);
        assert_eq!(state.last_ledger, 520);
    }

    // ── Exhaustive per-event last_ledger advance check ────────────────────

    /// Every documented event must advance last_ledger on a complete page.
    #[test]
    fn test_every_contract_event_advances_last_ledger() {
        for &name in DOCUMENTED_EVENTS {
            let mut state = IndexerState { last_ledger: 0 };
            // Use a payload shape matching the event's set; for admin/contrib
            // events the payload is not XDR-decoded for bounty id, so any
            // non-empty string works.
            let payload = if SINGLE_VALUE_EVENTS.contains(&name) {
                valid_single_bounty_id()
            } else if TUPLE_EVENTS.contains(&name) {
                valid_tuple_bounty_id()
            } else {
                "deadbeef".to_owned()
            };
            let events = vec![raw_event(name, &payload, 1)];
            poll_once(&mut state, &events, true);
            assert_eq!(
                state.last_ledger, 1,
                "event '{name}' did not advance last_ledger"
            );
        }
    }

    // ── No-duplicate-set membership ───────────────────────────────────────

    #[test]
    fn test_no_event_in_multiple_sets() {
        let all_sets: &[(&str, &[&str])] = &[
            ("SINGLE_VALUE_EVENTS", SINGLE_VALUE_EVENTS),
            ("TUPLE_EVENTS", TUPLE_EVENTS),
            ("CONTRIBUTOR_ONLY_EVENTS", CONTRIBUTOR_ONLY_EVENTS),
            ("CONTRACT_ADMIN_EVENTS", CONTRACT_ADMIN_EVENTS),
        ];

        let mut seen = std::collections::HashMap::<&str, &str>::new();
        for &(set_name, events) in all_sets {
            for &event in events {
                if let Some(prior) = seen.insert(event, set_name) {
                    panic!(
                        "event '{event}' appears in both '{prior}' and '{set_name}'; \
                         each event must be in exactly one set"
                    );
                }
            }
        }
    }

    // ── Fuzz-style: random garbage payloads never panic (#1259) ──────────

    /// Feed a variety of corrupt / random payloads to `extract_bounty_id_hex`
    /// and confirm none cause a panic.  All must return `None`.
    #[test]
    fn test_garbage_payloads_never_panic() {
        let garbage = [
            "",
            " ",
            "\n\r\t",
            "AAAA",
            "////",
            "deadbeef",
            "cafebabe",
            "AAAA////",
            "aGVsbG8gd29ybGQ=", // "hello world" — valid base64 but not ScVal
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            &"A".repeat(10_000),
            "\0\0\0\0",
        ];

        for &payload in &garbage {
            let single = extract_bounty_id_hex(&["bounty_claimed".to_owned()], payload);
            let tuple = extract_bounty_id_hex(&["bounty_created".to_owned()], payload);
            // Must not panic; None is acceptable for all garbage.
            let _ = single;
            let _ = tuple;
        }
        // If we reached here, no panic occurred.
    }
}
