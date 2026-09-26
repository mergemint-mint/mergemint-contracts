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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bounty {
    pub id: String,
    pub creator: String,
    pub assignee: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// One page of a cursor-paginated bounty listing.
#[derive(Debug, Serialize)]
pub struct BountyPage {
    pub bounties: Vec<Bounty>,
    pub next_cursor: Option<String>,
}

/// List bounties created by `creator`, newest-first, paginated by `cursor`.
/// An empty `creator` matches every bounty — used by the unfiltered
/// `GET /bounties` listing.
///
/// `limit` is trusted to already be clamped by the caller (see the
/// max-limit clamp in `routes::bounties::list_bounties`); this function
/// does not re-validate it.
pub fn list_bounties_by_creator(
    db: &SharedDb,
    creator: &str,
    limit: i64,
    cursor: Option<DateTime<Utc>>,
) -> BountyPage {
    let guard = read_db(db);
    let mut matches: Vec<Bounty> = guard
        .bounties
        .iter()
        .filter(|b| creator.is_empty() || b.creator == creator)
        .filter(|b| cursor.is_none_or(|c| b.created_at < c))
        .cloned()
        .collect();
    matches.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    paginate(matches, limit)
}

/// List bounties where `assignee` matches the recorded assignee, newest-first,
/// paginated by `cursor`.
pub fn list_bounties_by_assignee(
    db: &SharedDb,
    assignee: &str,
    limit: i64,
    cursor: Option<DateTime<Utc>>,
) -> BountyPage {
    let guard = read_db(db);
    let mut matches: Vec<Bounty> = guard
        .bounties
        .iter()
        .filter(|b| b.assignee.as_deref() == Some(assignee))
        .filter(|b| cursor.is_none_or(|c| b.created_at < c))
        .cloned()
        .collect();
    matches.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    paginate(matches, limit)
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
/// poison.
pub fn read_idempotency(
    store: &SharedIdempotencyStore,
) -> std::sync::RwLockReadGuard<'_, IdempotencyStore> {
    store.read().unwrap_or_else(|e| e.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Placeholder test — verifies that the test harness compiles and is wired
    /// correctly.  See issue #487.
    #[test]
    fn it_compiles() {}

    #[test]
    fn test_acquire_db_normal() {
        let db = new_shared_db();
        let mut guard = acquire_db(&db);
        guard.records.insert("key".to_string(), "value".to_string());
        assert_eq!(guard.records.get("key").map(|s| s.as_str()), Some("value"));
    }

    #[test]
    fn test_acquire_db_poison_recovery() {
        let db = new_shared_db();

        // Simulate a panic while holding the lock.
        let db_clone = Arc::clone(&db);
        let _ = std::panic::catch_unwind(move || {
            let _guard = db_clone.write().unwrap();
            panic!("simulated panic");
        });

        // The lock is now poisoned; acquire_db must not propagate the poison.
        let guard = acquire_db(&db);
        assert!(guard.records.is_empty(), "recovered store should be intact");
    }

    #[test]
    fn test_concurrent_read_guards_are_allowed() {
        let db = new_shared_db();
        let read_a = read_db(&db);
        let read_b = read_db(&db);

        assert!(read_a.records.is_empty());
        assert!(read_b.records.is_empty());
    }

    #[test]
    fn test_bounties_index_migration_covers_assignee_and_status() {
        let migration = BOUNTIES_INDEX_MIGRATION.to_lowercase();

        assert!(
            migration
                .contains("create index if not exists idx_bounties_assignee on bounties (assignee)"),
            "migration must index bounties.assignee (filtered by list_bounties_by_assignee); got:\n{migration}"
        );
        assert!(
            migration
                .contains("create index if not exists idx_bounties_status on bounties (status)"),
            "migration must index bounties.status (filtered by status-based listing queries); got:\n{migration}"
        );
    }

    #[test]
    fn test_idempotency_store_starts_empty() {
        let store = new_shared_idempotency_store();
        assert!(read_idempotency(&store).entries.is_empty());
    }

    #[test]
    fn test_idempotency_store_records_in_flight_then_completed() {
        let store = new_shared_idempotency_store();

        acquire_idempotency(&store)
            .entries
            .insert("key-1".to_string(), IdempotencyEntry::InFlight);
        assert!(matches!(
            read_idempotency(&store).entries.get("key-1"),
            Some(IdempotencyEntry::InFlight)
        ));

        acquire_idempotency(&store).entries.insert(
            "key-1".to_string(),
            IdempotencyEntry::Completed(r#"{"ok":true}"#.to_string()),
        );
        assert!(matches!(
            read_idempotency(&store).entries.get("key-1"),
            Some(IdempotencyEntry::Completed(body)) if body == r#"{"ok":true}"#
        ));
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
    fn test_audit_logs_migration_has_required_indexes() {
        let migration = AUDIT_LOGS_MIGRATION.to_lowercase();

        assert!(
            migration.contains("create table if not exists audit_logs"),
            "migration must create audit_logs table; got:\n{migration}"
        );
        assert!(
            migration.contains("timestamp bigint not null"),
            "migration must have timestamp field; got:\n{migration}"
        );
        assert!(
            migration.contains("actor varchar"),
            "migration must have actor field; got:\n{migration}"
        );
        assert!(
            migration.contains("action varchar"),
            "migration must have action field; got:\n{migration}"
        );
        assert!(
            migration.contains("target varchar"),
            "migration must have target field; got:\n{migration}"
        );
        assert!(
            migration.contains("create index if not exists idx_audit_logs_timestamp on audit_logs (timestamp desc)"),
            "migration must index audit_logs.timestamp DESC; got:\n{migration}"
        );
        assert!(
            migration.contains("create index if not exists idx_audit_logs_actor on audit_logs (actor)"),
            "migration must index audit_logs.actor; got:\n{migration}"
        );
        assert!(
            migration.contains("create index if not exists idx_audit_logs_action on audit_logs (action)"),
            "migration must index audit_logs.action; got:\n{migration}"
        );
    }

    #[test]
    fn test_write_and_query_audit_logs() {
        let db = new_shared_db();

        let log1 = AuditLog::new(
            "arbitrator-1".to_string(),
            "resolve_dispute".to_string(),
            "bounty-123".to_string(),
            1000,
        );
        let log2 = AuditLog::new(
            "arbitrator-2".to_string(),
            "resolve_dispute".to_string(),
            "bounty-456".to_string(),
            2000,
        );

        write_audit_log(&db, log1.clone());
        write_audit_log(&db, log2.clone());

        let logs = query_audit_logs(&db, None, None, None, 100);
        assert_eq!(logs.len(), 2);
        // Should be sorted by timestamp descending (newest first)
        assert_eq!(logs[0].timestamp, 2000);
        assert_eq!(logs[1].timestamp, 1000);
    }

    #[test]
    fn test_query_audit_logs_by_actor() {
        let db = new_shared_db();

        write_audit_log(&db, AuditLog::new("alice".to_string(), "resolve_dispute".to_string(), "bounty-1".to_string(), 100));
        write_audit_log(&db, AuditLog::new("bob".to_string(), "resolve_dispute".to_string(), "bounty-2".to_string(), 200));
        write_audit_log(&db, AuditLog::new("alice".to_string(), "resolve_dispute".to_string(), "bounty-3".to_string(), 300));

        let alice_logs = query_audit_logs(&db, Some("alice"), None, None, 100);
        assert_eq!(alice_logs.len(), 2);
        assert!(alice_logs.iter().all(|log| log.actor == "alice"));
    }

    #[test]
    fn test_query_audit_logs_by_target() {
        let db = new_shared_db();

        write_audit_log(&db, AuditLog::new("alice".to_string(), "resolve_dispute".to_string(), "bounty-1".to_string(), 100));
        write_audit_log(&db, AuditLog::new("bob".to_string(), "resolve_dispute".to_string(), "bounty-2".to_string(), 200));
        write_audit_log(&db, AuditLog::new("charlie".to_string(), "resolve_dispute".to_string(), "bounty-1".to_string(), 300));

        let bounty1_logs = query_audit_logs(&db, None, None, Some("bounty-1"), 100);
        assert_eq!(bounty1_logs.len(), 2);
        assert!(bounty1_logs.iter().all(|log| log.target == "bounty-1"));
    }

    #[test]
    fn test_query_audit_logs_respects_limit() {
        let db = new_shared_db();

        for i in 0..10 {
            write_audit_log(&db, AuditLog::new("actor".to_string(), "action".to_string(), format!("target-{}", i), i as u64));
        }

        let limited = query_audit_logs(&db, None, None, None, 3);
        assert_eq!(limited.len(), 3);
    }
}

/// Get a single bounty by id
pub fn get_bounty(
    db: &SharedDb,
    id: &str,
) -> Option<Bounty> {
    let guard = read_db(db);
    guard.bounties.iter().find(|b| b.id == id).cloned()
}
