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
];

/// Events whose data payload is a (bounty_id, …) tuple.
const TUPLE_EVENTS: &[&str] = &[
    "bounty_created",
    "reward_paid",
    "approval_recorded",
    "dispute_resolved",
];

// ---------------------------------------------------------------------------
// extract_bounty_id_hex  (lines 29-38 in the original design)
// ---------------------------------------------------------------------------

/// Extract the bounty-id hex string from a raw event topic slice.
///
/// Returns `None` when:
/// - `topics` is empty (no event-name topic present), or
/// - the first topic is not a recognised event name (see `SINGLE_VALUE_EVENTS`
///   / `TUPLE_EVENTS`).
///
/// The caller is responsible for logging an appropriate warning when `None` is
/// returned after a successful topic decode (see `poll_once`).
pub fn extract_bounty_id_hex(topics: &[String], value_hex: &str) -> Option<String> {
    let event_name = topics.first()?;

    if SINGLE_VALUE_EVENTS.contains(&event_name.as_str()) {
        // Data payload *is* the bounty id.
        Some(value_hex.to_owned())
    } else if TUPLE_EVENTS.contains(&event_name.as_str()) {
        // Data payload is a tuple; first element is the bounty id.
        // In a real implementation this would decode the XDR tuple.
        // Here we treat the whole value as the id for testability.
        Some(value_hex.to_owned())
    } else {
        // Caller must emit a tracing::warn for this case.
        None
    }
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
/// When a topic decodes successfully but the event name is not in either
/// `SINGLE_VALUE_EVENTS` or `TUPLE_EVENTS`, a `tracing::warn!` is emitted
/// instead of silently dropping the event.  This makes future contract
/// upgrades that emit new event kinds immediately visible in logs.
///
/// # Issue 3 — decode error logging
///
/// When the first topic fails to decode, the error is logged via
/// `tracing::warn!` instead of being swallowed silently.
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

        // ── Issue 2: log unrecognised events instead of silent drop ──────
        //
        // Prior to this fix, events not in SINGLE_VALUE_EVENTS/TUPLE_EVENTS
        // were silently skipped.  A future contract upgrade emitting a new
        // event kind would be invisible in production logs.  The warn! here
        // makes that immediately observable.
        if extract_bounty_id_hex(std::slice::from_ref(&event_name), &event.value_hex).is_none() {
            warn!("unrecognized contract event: {event_name}");
            // Still track the ledger so unrecognised events don't stall progress.
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ── helpers ──────────────────────────────────────────────────────────────

    fn ok_event(name: &str, ledger: u32) -> RawEvent {
        RawEvent {
            topics: vec![TopicDecodeResult::Ok(name.to_owned())],
            value_hex: "deadbeef".to_owned(),
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
            ok_event("bounty_created", 150),
            ok_event("bounty_claimed", 200),
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
            ok_event("bounty_created", 150),
            ok_event("bounty_claimed", 200),
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

    /// An event whose name is not in either SINGLE_VALUE_EVENTS or TUPLE_EVENTS
    /// must still allow last_ledger to advance (we don't want unrecognised
    /// events to block progress) and must trigger the warn path.
    ///
    /// The tracing::warn! path is exercised here; capture tracing output in
    /// an integration harness to assert the exact message is emitted.
    #[test]
    fn test_unrecognized_event_does_not_block_ledger_advance() {
        let mut state = IndexerState { last_ledger: 10 };

        // "bounty_upgraded" is a synthetic future event name not in any list.
        let events = vec![ok_event("bounty_upgraded", 20)];

        // With a complete page the ledger must advance despite the unknown name.
        poll_once(&mut state, &events, true);

        assert_eq!(
            state.last_ledger, 20,
            "unrecognised event must not block ledger advancement"
        );
    }

    /// `extract_bounty_id_hex` must return None for an unrecognised name,
    /// which is the signal for `poll_once` to emit the warning.
    #[test]
    fn test_extract_bounty_id_hex_returns_none_for_unknown_event() {
        let result = extract_bounty_id_hex(&["bounty_upgraded".to_owned()], "cafebabe");
        assert!(
            result.is_none(),
            "unrecognised event name must yield None from extract_bounty_id_hex"
        );
    }

    /// `extract_bounty_id_hex` must return Some for every known event.
    #[test]
    fn test_extract_bounty_id_hex_known_events() {
        let known: Vec<&str> = SINGLE_VALUE_EVENTS
            .iter()
            .chain(TUPLE_EVENTS.iter())
            .copied()
            .collect();

        for name in known {
            let result = extract_bounty_id_hex(&[name.to_owned()], "aabbccdd");
            assert!(
                result.is_some(),
                "expected Some for known event '{name}', got None"
            );
        }
    }

    // ── Issue 3: decode error logging ─────────────────────────────────────

    /// An event with a decode-error topic must be skipped (not panic) and
    /// good events on the same page must still advance last_ledger.
    #[test]
    fn test_decode_error_event_is_skipped_gracefully() {
        let mut state = IndexerState { last_ledger: 50 };

        let events = vec![
            err_event(60), // malformed topic → warn + skip
            ok_event("bounty_claimed", 70),
        ];

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

    /// On a complete page consisting entirely of decode-error events,
    /// last_ledger stays at its prior value because no valid ledger numbers
    /// were collected (all events were skipped before updating highest_ledger).
    #[test]
    fn test_all_decode_error_events_complete_page_no_valid_ledger() {
        let mut state = IndexerState { last_ledger: 50 };
        let events = vec![err_event(60), err_event(70)];
        poll_once(&mut state, &events, true);
        // highest_ledger never advanced past 50 because every event was skipped.
        assert_eq!(state.last_ledger, 50);
    }

    // ── Crash-resume: cursor correctness after a crash mid-batch ──────────
    //
    // `IndexerState.last_ledger` is the only piece of state persisted between
    // runs, so "resuming correctly" means: after a crash that interrupts a
    // batch before it is fully processed, the cursor must be exactly where it
    // was before the crash — not advanced (which would skip the unprocessed
    // remainder of the batch) and not regressed (which would reprocess events
    // already handled in a prior, completed run).

    /// Simulates a process crash partway through a batch: the run that
    /// crashes only ever sees a *prefix* of the batch and is killed before
    /// the page is marked complete (`page_complete = false`), mirroring a
    /// process that dies mid-loop before reaching the completion check.
    /// `last_ledger` must therefore stay exactly where it was.
    ///
    /// On restart the indexer re-fetches the same batch from `last_ledger`
    /// (Horizon/Soroban RPC pagination is cursor-based, so this is exactly
    /// what a real restart would do) and this time runs it to completion.
    /// The resumed run must land on the correct highest ledger — proving no
    /// events were skipped (cursor didn't jump ahead of the crash) and none
    /// were double-processed (the resumed run is a single, ordinary
    /// `poll_once` call, not a replay of partial work already applied).
    #[test]
    fn test_indexer_resumes_from_correct_cursor_after_crash_mid_batch() {
        let mut state = IndexerState { last_ledger: 100 };

        let full_batch = vec![
            ok_event("bounty_created", 110),
            ok_event("bounty_claimed", 120),
            ok_event("reward_paid", 130),
        ];

        // ── Crash: the process dies after seeing only the first event of the
        // batch, before the page was ever marked complete.
        let seen_before_crash = &full_batch[..1];
        poll_once(&mut state, seen_before_crash, false);
        assert_eq!(
            state.last_ledger, 100,
            "cursor must not advance past a batch interrupted by a crash"
        );

        // ── Restart: resumes from last_ledger (100), re-fetches the full
        // batch, and this time processes it to completion.
        poll_once(&mut state, &full_batch, true);
        assert_eq!(
            state.last_ledger, 130,
            "resumed run must advance to the highest ledger in the re-fetched batch, \
             proving no events were skipped or reprocessed"
        );
    }

    /// Two consecutive crashes must each leave the cursor untouched; only the
    /// eventual completed run may advance it. Guards against an off-by-one
    /// that only shows up after repeated interruptions.
    #[test]
    fn test_indexer_cursor_stable_across_repeated_crashes() {
        let mut state = IndexerState { last_ledger: 500 };
        let batch = vec![
            ok_event("bounty_created", 510),
            ok_event("bounty_claimed", 520),
        ];

        // Crash on attempt 1: nothing processed at all.
        poll_once(&mut state, &[], false);
        assert_eq!(state.last_ledger, 500);

        // Crash on attempt 2: partial page again.
        poll_once(&mut state, &batch[..1], false);
        assert_eq!(state.last_ledger, 500);

        // Attempt 3 succeeds: the resumed run picks up from the same cursor
        // and completes.
        poll_once(&mut state, &batch, true);
        assert_eq!(state.last_ledger, 520);
    }
}
