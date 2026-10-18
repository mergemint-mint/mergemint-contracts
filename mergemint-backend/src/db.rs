// mergemint-backend/src/db.rs
//
// Database connection pool and shared state helpers.
//
// ## Lock-poison recovery (#473)
//
// Rust's lock APIs return `Err(PoisonError)` if a thread panicked while
// holding the lock. Calling `.unwrap()` would re-panic every subsequent
// caller, effectively taking the whole service down for what is often a
// transient edge case.
//
// We use `.unwrap_or_else(|e| e.into_inner())` instead: when the lock is
// poisoned we recover the inner value and continue under the assumption that
// the data is still in a consistent-enough state to serve requests.  If the
// data truly is corrupt the next business-logic validation will catch it and
// return an error to the client rather than crashing the process.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

// ---------------------------------------------------------------------------
// Migrations
// ---------------------------------------------------------------------------
//
// `DbStore` above is the in-memory stand-in used today; `migrations/` holds
// the SQL that applies to the real Postgres-backed store production
// deployments run instead (see the module doc comment). It is not yet
// wired to a live connection pool or `sqlx::migrate!()` -- there is no
// Postgres integration in this crate yet -- but it is the source of truth
// for schema/index decisions so they aren't lost when that pool lands.
//
// `migrations/0001_add_bounties_indexes.sql` indexes `bounties.assignee`
// and `bounties.status`, the columns the indexer's writes are later
// filtered on by `list_bounties_by_assignee` and status-based listing
// queries. `BOUNTIES_INDEX_MIGRATION` below pins that file's content so an
// edit that silently drops one of the two indexes fails `cargo test`
// instead of only surfacing as a slow query in production.
#[cfg(test)]
const BOUNTIES_INDEX_MIGRATION: &str = include_str!("../migrations/0001_add_bounties_indexes.sql");
#[cfg(test)]
const AUDIT_LOGS_MIGRATION: &str = include_str!("../migrations/0002_create_audit_logs_table.sql");

/// Lightweight in-memory store used during development / integration tests.
/// Production deployments replace this with a real database pool.
#[derive(Debug, Default)]
pub struct DbStore {
    pub records: HashMap<String, String>,
    /// Bounty listing rows backing `list_bounties_by_creator` /
    /// `list_bounties_by_assignee`. Kept separate from `records` (which
    /// stores the flat id -> JSON blobs used by the dispute/self-claim
    /// flows) since it has its own queryable shape.
    pub bounties: Vec<Bounty>,
    /// Audit log entries tracking administrative actions like resolve_dispute.
    pub audit_logs: Vec<AuditLog>,
    /// Registered webhook subscriptions (see `webhooks.rs`).
    pub webhook_subscriptions: Vec<WebhookSubscription>,
    /// Set by `close_db` to simulate a lost connection pool; `ping_db` (and
    /// therefore `/ready`) reports the store as unavailable while it is set.
    pub is_closed: bool,
}

// ---------------------------------------------------------------------------
// Webhook subscriptions
// ---------------------------------------------------------------------------

/// A registered webhook endpoint that receives signed bounty event payloads.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WebhookSubscription {
    pub id: String,
    pub url: String,
    /// HMAC-SHA256 signing secret. Never returned to API clients.
    #[serde(skip_serializing, default)]
    pub secret: String,
    /// Event names this subscription receives; `"*"` or an empty list means all.
    pub event_types: Vec<String>,
    pub active: bool,
    pub created_at: DateTime<Utc>,
}

/// Register a new, active webhook subscription and return it.
pub fn add_webhook_subscription(
    db: &SharedDb,
    id: String,
    url: String,
    secret: String,
    event_types: Vec<String>,
) -> WebhookSubscription {
    let subscription = WebhookSubscription {
        id,
        url,
        secret,
        event_types,
        active: true,
        created_at: Utc::now(),
    };
    acquire_db(db)
        .webhook_subscriptions
        .push(subscription.clone());
    subscription
}

/// Return every registered webhook subscription, oldest first.
pub fn list_webhook_subscriptions(db: &SharedDb) -> Vec<WebhookSubscription> {
    read_db(db).webhook_subscriptions.clone()
}

/// Look up a webhook subscription by id.
pub fn get_webhook_subscription(db: &SharedDb, id: &str) -> Option<WebhookSubscription> {
    read_db(db)
        .webhook_subscriptions
        .iter()
        .find(|s| s.id == id)
        .cloned()
}

/// Remove a webhook subscription by id. Returns `true` if one was removed.
pub fn delete_webhook_subscription(db: &SharedDb, id: &str) -> bool {
    let mut guard = acquire_db(db);
    let before = guard.webhook_subscriptions.len();
    guard.webhook_subscriptions.retain(|s| s.id != id);
    guard.webhook_subscriptions.len() != before
}

// ---------------------------------------------------------------------------
// Audit Logging
// ---------------------------------------------------------------------------

/// An audit log entry recording an administrative action (e.g., resolve_dispute).
///
/// Fields match the database schema defined in migrations/0002_create_audit_logs_table.sql.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLog {
    pub id: Option<i64>,
    pub actor: String,
    pub action: String,
    pub target: String,
    pub timestamp: u64,
}

impl AuditLog {
    /// Create a new audit log entry with the given fields.
    pub fn new(actor: String, action: String, target: String, timestamp: u64) -> Self {
        AuditLog {
            id: None,
            actor,
            action,
            target,
            timestamp,
        }
    }
}

// ---------------------------------------------------------------------------
// Bounty listing
// ---------------------------------------------------------------------------

/// A bounty row as exposed by the listing endpoints (`list_bounties`,
/// `list_bounties_by_assignee`). Distinct from `routes::tx::Bounty`, which
/// models only the fields needed to build payout XDR for the dispute /
/// self-claim flows.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Bounty {
    pub id: String,
    pub creator: String,
    pub assignee: Option<String>,
    pub created_at: DateTime<Utc>,
    /// Reward amount, mirrored from the contract's `Bounty::reward_amount`.
    pub reward: i128,
    /// Optional expiry, mirrored from the contract's `Bounty::deadline`. `None`
    /// means the bounty never expires.
    pub deadline: Option<DateTime<Utc>>,
    /// Lifecycle status (`open`, `claimed`, `completed`, `cancelled`).
    #[serde(default = "default_bounty_status")]
    pub status: String,
    /// Tags attached to the bounty, used by the `tag` listing filter.
    #[serde(default)]
    pub tags: Vec<String>,
}

fn default_bounty_status() -> String {
    "open".to_string()
}

impl Default for Bounty {
    fn default() -> Self {
        Bounty {
            id: String::new(),
            creator: String::new(),
            assignee: None,
            created_at: DateTime::<Utc>::default(),
            reward: 0,
            deadline: None,
            status: default_bounty_status(),
            tags: Vec::new(),
        }
    }
}

/// Columns a bounty listing may be sorted by. Deserialization is restricted to
/// this whitelist (`#[serde(rename_all = "lowercase")]`), so an unknown
/// `sort=` value is rejected by the query extractor and can never reach the
/// query layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum BountySortField {
    #[default]
    Created,
    Reward,
    Deadline,
}

/// Direction applied to the selected [`BountySortField`]. Whitelisted the same
/// way, so only `asc` / `desc` are ever accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum SortOrder {
    Asc,
    #[default]
    Desc,
}

/// One page of a cursor-paginated bounty listing.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct BountyPage {
    pub bounties: Vec<Bounty>,
    pub next_cursor: Option<String>,
}

/// List bounties created by `creator`, optionally filtered by `status` and
/// `tag`, ordered by `sort` / `order` (newest
/// first by default), paginated by `cursor`. An empty `creator` matches every
/// bounty — used by the unfiltered `GET /bounties` listing.
///
/// `limit` is trusted to already be clamped by the caller (see the
/// max-limit clamp in `routes::bounties::list_bounties`); this function
/// does not re-validate it.
#[allow(clippy::too_many_arguments)]
pub fn list_bounties_by_creator(
    db: &SharedDb,
    creator: &str,
    limit: i64,
    cursor: Option<DateTime<Utc>>,
    status: Option<&str>,
    tag: Option<&str>,
    sort: BountySortField,
    order: SortOrder,
) -> BountyPage {
    let guard = read_db(db);
    let mut matches: Vec<Bounty> = guard
        .bounties
        .iter()
        .filter(|b| creator.is_empty() || b.creator == creator)
        .filter(|b| status.is_none_or(|s| b.status == s))
        .filter(|b| tag.is_none_or(|t| b.tags.iter().any(|bt| bt == t)))
        .filter(|b| cursor.is_none_or(|c| b.created_at < c))
        .cloned()
        .collect();
    apply_sort(&mut matches, sort, order);
    paginate(matches, limit)
}

/// List bounties where `assignee` matches the recorded assignee, newest-first
/// by default, paginated by `cursor`.
pub fn list_bounties_by_assignee(
    db: &SharedDb,
    assignee: &str,
    limit: i64,
    cursor: Option<DateTime<Utc>>,
    sort: BountySortField,
    order: SortOrder,
) -> BountyPage {
    let guard = read_db(db);
    let mut matches: Vec<Bounty> = guard
        .bounties
        .iter()
        .filter(|b| b.assignee.as_deref() == Some(assignee))
        .filter(|b| cursor.is_none_or(|c| b.created_at < c))
        .cloned()
        .collect();
    apply_sort(&mut matches, sort, order);
    paginate(matches, limit)
}

/// Order `bounties` in place by `sort` / `order`. Sorting is stable, and the
/// default (`Created` + `Desc`) reproduces the historical newest-first order.
/// Bounties without a `deadline` sort after those that have one, in both
/// directions, so "soonest expiring first" lists never lead with non-expiring
/// bounties.
fn apply_sort(bounties: &mut [Bounty], sort: BountySortField, order: SortOrder) {
    bounties.sort_by(|a, b| {
        if sort == BountySortField::Deadline {
            return match (a.deadline, b.deadline) {
                (Some(x), Some(y)) => match order {
                    SortOrder::Asc => x.cmp(&y),
                    SortOrder::Desc => y.cmp(&x),
                },
                // Bounties without a deadline always trail those that have
                // one, regardless of direction.
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            };
        }
        let ord = match sort {
            BountySortField::Created => a.created_at.cmp(&b.created_at),
            BountySortField::Reward => a.reward.cmp(&b.reward),
            BountySortField::Deadline => unreachable!("handled above"),
        };
        match order {
            SortOrder::Asc => ord,
            SortOrder::Desc => ord.reverse(),
        }
    });
}

/// Trim `bounties` to at most `limit` entries, returning the next cursor
/// (the `created_at` of the last row) when more results exist beyond it.
fn paginate(mut bounties: Vec<Bounty>, limit: i64) -> BountyPage {
    let limit = usize::try_from(limit).unwrap_or(0);
    let has_more = bounties.len() > limit;
    bounties.truncate(limit);
    let next_cursor = if has_more {
        bounties.last().map(|b| b.created_at.to_rfc3339())
    } else {
        None
    };
    BountyPage {
        bounties,
        next_cursor,
    }
}

// ---------------------------------------------------------------------------
// Audit log queries
// ---------------------------------------------------------------------------

/// Query audit logs, optionally filtering by actor, action, or target.
/// Results are sorted by timestamp descending (newest first).
pub fn query_audit_logs(
    db: &SharedDb,
    actor: Option<&str>,
    action: Option<&str>,
    target: Option<&str>,
    limit: i64,
) -> Vec<AuditLog> {
    let guard = read_db(db);
    let mut logs: Vec<AuditLog> = guard
        .audit_logs
        .iter()
        .filter(|log| actor.is_none_or(|a| log.actor == a))
        .filter(|log| action.is_none_or(|a| log.action == a))
        .filter(|log| target.is_none_or(|t| log.target == t))
        .cloned()
        .collect();

    // Sort by timestamp descending (newest first)
    logs.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

    let limit = usize::try_from(limit).unwrap_or(usize::MAX);
    logs.truncate(limit);
    logs
}

/// Add an audit log entry to the store.
pub fn write_audit_log(db: &SharedDb, log: AuditLog) {
    let mut guard = acquire_db(db);
    guard.audit_logs.push(log);
}

/// Shared, thread-safe handle to the database store.
///
/// The store uses an `RwLock` so API read paths can proceed concurrently while
/// writes still take exclusive access. This mirrors the production goal of
/// avoiding a single mutex-guarded connection that serializes every DB access.
pub type SharedDb = Arc<RwLock<DbStore>>;

/// Create a new, empty shared database handle.
pub fn new_shared_db() -> SharedDb {
    Arc::new(RwLock::new(DbStore::default()))
}

/// Acquire the database write lock, recovering gracefully from lock poison.
///
/// If a previous thread panicked while holding this lock, `.into_inner()`
/// extracts the guarded value so the service can keep running instead of
/// propagating the panic to every subsequent request.
#[allow(dead_code)]
pub fn acquire_db(db: &SharedDb) -> std::sync::RwLockWriteGuard<'_, DbStore> {
    db.write().unwrap_or_else(|e| e.into_inner())
}

/// Acquire the database read lock, recovering gracefully from lock poison.
pub fn read_db(db: &SharedDb) -> std::sync::RwLockReadGuard<'_, DbStore> {
    db.read().unwrap_or_else(|e| e.into_inner())
}

/// Ping the database connection (#870).
///
/// Backs the `/ready` readiness probe: orchestrators call it before routing
/// traffic to an instance, so a broken connection surfaces here rather than
/// as a 500 on the first real request. Lock poison is recovered by `read_db`,
/// so a poisoned lock still counts as reachable.
///
/// Returns `Ok(())` if the database is open and reachable, or `Err("database connection is closed")`
/// if the database has been closed or disconnected.
pub fn ping_db(db: &SharedDb) -> Result<(), &'static str> {
    let guard = read_db(db);
    if guard.is_closed {
        Err("database connection is closed")
    } else {
        Ok(())
    }
}

/// Mark the database connection pool as closed (simulates database downtime/disconnection, #870).
#[allow(dead_code)]
pub fn close_db(db: &SharedDb) {
    let mut guard = acquire_db(db);
    guard.is_closed = true;
}

// ---------------------------------------------------------------------------
// Idempotency-key store
// ---------------------------------------------------------------------------

/// Outcome recorded for a client-supplied `Idempotency-Key`.
///
/// `InFlight` is written *before* the transaction-submitting work begins, so
/// a concurrent duplicate request (the client retried before the first
/// response came back) sees the reservation rather than racing the same
/// submission a second time. `Completed` replays the original response body
/// once the first request finishes successfully.
#[derive(Debug, Clone)]
pub enum IdempotencyEntry {
    InFlight,
    Completed(String),
}

/// In-memory record of recently-seen idempotency keys, keyed by the raw
/// `Idempotency-Key` header value.
///
/// Mirrors `DbStore`: a plain `HashMap` guarded by an `RwLock` kept separate
/// from `DbStore` so idempotency bookkeeping never contends with the
/// business-data lock.
#[derive(Debug, Default)]
pub struct IdempotencyStore {
    pub entries: HashMap<String, IdempotencyEntry>,
}

/// Shared, thread-safe handle to the idempotency store.
pub type SharedIdempotencyStore = Arc<RwLock<IdempotencyStore>>;

/// Create a new, empty shared idempotency-store handle.
pub fn new_shared_idempotency_store() -> SharedIdempotencyStore {
    Arc::new(RwLock::new(IdempotencyStore::default()))
}

/// Acquire the idempotency-store write lock, recovering gracefully from lock
/// poison (see the module-level note on lock-poison recovery, #473).
pub fn acquire_idempotency(
    store: &SharedIdempotencyStore,
) -> std::sync::RwLockWriteGuard<'_, IdempotencyStore> {
    store.write().unwrap_or_else(|e| e.into_inner())
}

/// Acquire the idempotency-store read lock, recovering gracefully from lock
/// poison (see the module-level note on lock-poison recovery, #473).
pub fn read_idempotency(
    store: &SharedIdempotencyStore,
) -> std::sync::RwLockReadGuard<'_, IdempotencyStore> {
    store.read().unwrap_or_else(|e| e.into_inner())
}

/// Get a single bounty by id
pub fn get_bounty(db: &SharedDb, id: &str) -> Option<Bounty> {
    let guard = read_db(db);
    guard.bounties.iter().find(|b| b.id == id).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounties_migration_indexes_assignee_and_status() {
        assert!(BOUNTIES_INDEX_MIGRATION.contains("ON bounties (assignee)"));
        assert!(BOUNTIES_INDEX_MIGRATION.contains("ON bounties (status)"));
    }

    #[test]
    fn audit_logs_migration_indexes_filter_columns() {
        for column in ["timestamp DESC", "actor", "action"] {
            assert!(
                AUDIT_LOGS_MIGRATION.contains(&format!("ON audit_logs ({column})")),
                "audit_logs migration must index {column}"
            );
        }
    }

    #[test]
    fn webhook_subscription_crud_round_trip() {
        let db = new_shared_db();
        let sub = add_webhook_subscription(
            &db,
            "sub-1".to_string(),
            "https://example.com/hook".to_string(),
            "secret".to_string(),
            vec!["*".to_string()],
        );
        assert!(sub.active);
        assert_eq!(list_webhook_subscriptions(&db).len(), 1);
        assert_eq!(get_webhook_subscription(&db, "sub-1"), Some(sub));
        assert!(delete_webhook_subscription(&db, "sub-1"));
        assert!(!delete_webhook_subscription(&db, "sub-1"));
        assert!(get_webhook_subscription(&db, "sub-1").is_none());
    }

    #[test]
    fn ping_succeeds_on_open_pool() {
        let db = new_shared_db();
        assert!(ping_db(&db).is_ok());
    }

    #[test]
    fn ping_recovers_from_poisoned_lock() {
        let db = new_shared_db();
        // Poison the lock by panicking while holding the write guard.
        let poisoned = db.clone();
        let _ = std::thread::spawn(move || {
            let _guard = poisoned.write().unwrap();
            panic!("poison the lock");
        })
        .join();
        // `read_db` recovers from poison, so the probe still reports ready.
        assert!(ping_db(&db).is_ok());
    }

    #[test]
    fn test_idempotency_store_poison_recovery() {
        let store = new_shared_idempotency_store();

        let store_clone = Arc::clone(&store);
        let _ = std::panic::catch_unwind(move || {
            let _guard = store_clone.write().unwrap();
            panic!("simulated panic");
        });

        let guard = acquire_idempotency(&store);
        assert!(guard.entries.is_empty(), "recovered store should be intact");
    }

    #[test]
    fn test_leaderboard_aggregates_bounties_by_assignee() {
        let db = new_shared_db();
        let time = Utc::now();

        {
            let mut guard = acquire_db(&db);
            guard.bounties.push(Bounty {
                id: "1".to_string(),
                creator: "alice".to_string(),
                assignee: Some("bob".to_string()),
                created_at: time,
                ..Default::default()
            });
            guard.bounties.push(Bounty {
                id: "2".to_string(),
                creator: "alice".to_string(),
                assignee: Some("bob".to_string()),
                created_at: time,
                ..Default::default()
            });
            guard.bounties.push(Bounty {
                id: "3".to_string(),
                creator: "alice".to_string(),
                assignee: Some("carol".to_string()),
                created_at: time,
                ..Default::default()
            });
        }

        let page = get_leaderboard(&db, 100);
        assert_eq!(page.entries.len(), 2);

        let bob_entry = page.entries.iter().find(|e| e.address == "bob").unwrap();
        assert_eq!(bob_entry.completed_bounties, 2);
        assert_eq!(bob_entry.reputation, 2);

        let carol_entry = page.entries.iter().find(|e| e.address == "carol").unwrap();
        assert_eq!(carol_entry.completed_bounties, 1);
        assert_eq!(carol_entry.reputation, 1);
    }

    #[test]
    fn test_leaderboard_sorts_by_reputation_descending() {
        let db = new_shared_db();
        let time = Utc::now();

        {
            let mut guard = acquire_db(&db);
            // alice has 3 completed bounties
            guard.bounties.push(Bounty {
                id: "1".to_string(),
                creator: "creator".to_string(),
                assignee: Some("alice".to_string()),
                created_at: time,
                ..Default::default()
            });
            guard.bounties.push(Bounty {
                id: "2".to_string(),
                creator: "creator".to_string(),
                assignee: Some("alice".to_string()),
                created_at: time,
                ..Default::default()
            });
            guard.bounties.push(Bounty {
                id: "3".to_string(),
                creator: "creator".to_string(),
                assignee: Some("alice".to_string()),
                created_at: time,
                ..Default::default()
            });
            // bob has 1 completed bounty
            guard.bounties.push(Bounty {
                id: "4".to_string(),
                creator: "creator".to_string(),
                assignee: Some("bob".to_string()),
                created_at: time,
                ..Default::default()
            });
        }

        let page = get_leaderboard(&db, 100);
        assert_eq!(page.entries.len(), 2);
        assert_eq!(page.entries[0].address, "alice");
        assert_eq!(page.entries[1].address, "bob");
    }

    #[test]
    fn test_leaderboard_respects_limit() {
        let db = new_shared_db();
        let time = Utc::now();

        {
            let mut guard = acquire_db(&db);
            for i in 0..10 {
                guard.bounties.push(Bounty {
                    id: i.to_string(),
                    creator: "creator".to_string(),
                    assignee: Some(format!("contributor_{}", i)),
                    created_at: time,
                    ..Default::default()
                });
            }
        }

        let page = get_leaderboard(&db, 5);
        assert_eq!(page.entries.len(), 5);
    }

    #[test]
    fn test_leaderboard_returns_empty_page_when_no_bounties() {
        let db = new_shared_db();

        let page = get_leaderboard(&db, 100);
        assert!(page.entries.is_empty());
    }

    #[test]
    fn test_ping_db_healthy_and_closed() {
        let db = new_shared_db();
        assert!(ping_db(&db).is_ok());

        close_db(&db);
        assert_eq!(ping_db(&db), Err("database connection is closed"));
    }
}

// ---------------------------------------------------------------------------
// Leaderboard
// ---------------------------------------------------------------------------

/// A contributor entry in the leaderboard, ranked by reputation and completed
/// bounties. Aggregates stats across the bounties table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaderboardEntry {
    pub address: String,
    pub completed_bounties: i64,
    pub reputation: i64,
}

/// A page of leaderboard entries, ranked by reputation (descending) and
/// completed bounties (descending).
#[derive(Debug, Clone, Serialize)]
pub struct LeaderboardPage {
    pub entries: Vec<LeaderboardEntry>,
}

/// Aggregate contributor statistics from completed bounties and compute a
/// leaderboard, ranked by reputation (descending), then by completed bounties
/// (descending).
///
/// `limit` is trusted to already be clamped by the caller; this function does
/// not re-validate it.
pub fn get_leaderboard(db: &SharedDb, limit: i64) -> LeaderboardPage {
    let guard = read_db(db);

    // Aggregate completed bounties per assignee
    let mut contributor_stats: HashMap<String, (i64, i64)> = HashMap::new();

    for bounty in &guard.bounties {
        if let Some(assignee) = &bounty.assignee {
            let (count, reputation) = contributor_stats.entry(assignee.clone()).or_insert((0, 0));
            *count += 1;
            // Simple reputation model: 1 point per completed bounty
            *reputation += 1;
        }
    }

    // Convert to leaderboard entries and sort by reputation (descending),
    // then by completed bounties (descending)
    let mut entries: Vec<LeaderboardEntry> = contributor_stats
        .into_iter()
        .map(
            |(address, (completed_bounties, reputation))| LeaderboardEntry {
                address,
                completed_bounties,
                reputation,
            },
        )
        .collect();

    entries.sort_by(|a, b| match b.reputation.cmp(&a.reputation) {
        std::cmp::Ordering::Equal => b.completed_bounties.cmp(&a.completed_bounties),
        other => other,
    });

    let limit = usize::try_from(limit).unwrap_or(0);
    entries.truncate(limit);

    LeaderboardPage { entries }
}
