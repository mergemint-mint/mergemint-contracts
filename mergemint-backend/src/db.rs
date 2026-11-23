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
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

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
#[cfg(test)]
const AUDIT_LOGS_PAGINATION_MIGRATION: &str =
    include_str!("../migrations/0003_add_audit_log_pagination_index.sql");
#[cfg(test)]
const IDEMPOTENCY_MIGRATION: &str = include_str!("../migrations/0010_create_idempotency_keys.sql");

/// Lightweight in-memory store used during development / integration tests.
/// Production deployments replace this with a real database pool.
#[derive(Debug, Default)]
pub struct DbStore {
    pub postgres: Option<sqlx::PgPool>,
    pub records: HashMap<String, String>,
    /// Bounty listing rows backing `list_bounties_by_creator` /
    /// `list_bounties_by_assignee`. Kept separate from `records` (which
    /// stores the flat id -> JSON blobs used by the dispute/self-claim
    /// flows) since it has its own queryable shape.
    pub bounties: Vec<Bounty>,
    /// Audit log entries tracking administrative actions like resolve_dispute.
    pub audit_logs: Vec<AuditLog>,
    pub reputation_history: Vec<ReputationHistoryEntry>,
    /// Registered webhook subscriptions (see `webhooks.rs`).
    pub webhook_subscriptions: Vec<WebhookSubscription>,
    stats_cache: Option<StatsCacheEntry>,
    /// Email notification subscriptions waiting for verification or delivery.
    pub email_subscriptions: Vec<EmailSubscription>,
    /// Bounty ids for which the deadline reminder has already been published.
    pub deadline_reminders_sent: HashSet<String>,
    /// Set by `close_db` to simulate a lost connection pool; `ping_db` (and
    /// therefore `/ready`) reports the store as unavailable while it is set.
    pub is_closed: bool,
}

// ---------------------------------------------------------------------------
// Webhook subscriptions
// ---------------------------------------------------------------------------

/// A registered webhook endpoint that receives signed bounty event payloads.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct WebhookSubscription {
    pub id: String,
    pub url: String,
    /// HMAC-SHA256 signing secret. Never returned to API clients.
    #[serde(skip_serializing, default)]
    #[schema(ignore)]
    pub secret: String,
    /// Event names this subscription receives; `"*"` or an empty list means all.
    pub event_types: Vec<String>,
    pub active: bool,
    pub created_at: DateTime<Utc>,
}

/// An opt-in email address associated with a contributor wallet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EmailSubscription {
    pub id: String,
    pub contributor_address: String,
    pub email: String,
    #[serde(skip_serializing)]
    pub verification_token: Option<String>,
    #[serde(skip_serializing)]
    pub unsubscribe_token: String,
    pub verified: bool,
    pub active: bool,
    pub created_at: DateTime<Utc>,
}

/// Create an unverified email subscription.
pub fn add_email_subscription(
    db: &SharedDb,
    contributor_address: String,
    email: String,
    verification_token: String,
    unsubscribe_token: String,
) -> EmailSubscription {
    let subscription = EmailSubscription {
        id: uuid::Uuid::new_v4().to_string(),
        contributor_address,
        email,
        verification_token: Some(verification_token),
        unsubscribe_token,
        verified: false,
        active: true,
        created_at: Utc::now(),
    };
    let mut guard = acquire_db(db);
    if let Some(existing) = guard.email_subscriptions.iter().find(|existing| {
        existing.contributor_address == subscription.contributor_address
            && existing.email == subscription.email
            && existing.verified
            && existing.active
    }) {
        return existing.clone();
    }
    guard.email_subscriptions.retain(|existing| {
        existing.contributor_address != subscription.contributor_address
            || existing.email != subscription.email
    });
    guard.email_subscriptions.push(subscription.clone());
    subscription
}

/// Verify an email address using its single-use verification token.
pub fn verify_email_subscription(db: &SharedDb, token: &str) -> bool {
    let mut guard = acquire_db(db);
    if let Some(subscription) = guard.email_subscriptions.iter_mut().find(|subscription| {
        subscription.verification_token.as_deref() == Some(token) && subscription.active
    }) {
        subscription.verified = true;
        subscription.verification_token = None;
        true
    } else {
        false
    }
}

/// Disable an email subscription using its per-address unsubscribe token.
pub fn unsubscribe_email_subscription(db: &SharedDb, token: &str) -> bool {
    let mut guard = acquire_db(db);
    if let Some(subscription) = guard
        .email_subscriptions
        .iter_mut()
        .find(|subscription| subscription.unsubscribe_token == token && subscription.active)
    {
        subscription.active = false;
        true
    } else {
        false
    }
}

/// Return verified, active email subscriptions for a contributor wallet.
pub fn verified_email_subscriptions_for(
    db: &SharedDb,
    contributor_address: &str,
) -> Vec<EmailSubscription> {
    read_db(db)
        .email_subscriptions
        .iter()
        .filter(|subscription| {
            subscription.active
                && subscription.verified
                && subscription.contributor_address == contributor_address
        })
        .cloned()
        .collect()
}

/// Change a bounty's status after the indexer observes a work submission.
#[allow(dead_code)] // Called by the indexer, which the binary does not compile.
pub fn mark_bounty_submitted(db: &SharedDb, bounty_id: &str) {
    if let Some(bounty) = acquire_db(db)
        .bounties
        .iter_mut()
        .find(|bounty| bounty.id == bounty_id)
    {
        bounty.status = "submitted".to_string();
    }
}

/// Record an indexed claim so subscribers can be matched to the assignee.
#[allow(dead_code)] // Called by the indexer, which the binary does not compile.
pub fn mark_bounty_claimed(db: &SharedDb, bounty_id: &str, assignee: &str) {
    if let Some(bounty) = acquire_db(db)
        .bounties
        .iter_mut()
        .find(|bounty| bounty.id == bounty_id)
    {
        bounty.assignee = Some(assignee.to_string());
        bounty.status = "claimed".to_string();
    }
}

/// Return claimed, unsubmitted bounties whose deadlines are within the lead window.
pub fn bounties_due_for_deadline_reminder(
    db: &SharedDb,
    now: DateTime<Utc>,
    reminder_at: DateTime<Utc>,
) -> Vec<Bounty> {
    let guard = read_db(db);
    guard
        .bounties
        .iter()
        .filter(|bounty| {
            bounty.status == "claimed"
                && bounty
                    .deadline
                    .is_some_and(|deadline| deadline > now && deadline <= reminder_at)
                && !guard.deadline_reminders_sent.contains(&bounty.id)
        })
        .cloned()
        .collect()
}

/// Update a bounty's status to `cancelled` after the indexer observes a cancellation.
#[allow(dead_code)] // Called by the indexer, which the binary does not compile.
pub fn mark_bounty_cancelled(db: &SharedDb, bounty_id: &str) {
    if let Some(bounty) = acquire_db(db)
        .bounties
        .iter_mut()
        .find(|bounty| bounty.id == bounty_id)
    {
        bounty.status = "cancelled".to_string();
    }
}

/// Update a bounty's status to `expired` after the indexer observes an expiry.
#[allow(dead_code)] // Called by the indexer, which the binary does not compile.
pub fn mark_bounty_expired(db: &SharedDb, bounty_id: &str) {
    if let Some(bounty) = acquire_db(db)
        .bounties
        .iter_mut()
        .find(|bounty| bounty.id == bounty_id)
    {
        bounty.status = "expired".to_string();
    }
}

/// Update a bounty's status to `completed` after the indexer observes completion.
#[allow(dead_code)] // Called by the indexer, which the binary does not compile.
pub fn mark_bounty_completed(db: &SharedDb, bounty_id: &str) {
    if let Some(bounty) = acquire_db(db)
        .bounties
        .iter_mut()
        .find(|bounty| bounty.id == bounty_id)
    {
        bounty.status = "completed".to_string();
    }
}

/// Update a bounty's status to `disputed` after the indexer observes a dispute.
#[allow(dead_code)] // Called by the indexer, which the binary does not compile.
pub fn mark_bounty_disputed(db: &SharedDb, bounty_id: &str) {
    if let Some(bounty) = acquire_db(db)
        .bounties
        .iter_mut()
        .find(|bounty| bounty.id == bounty_id)
    {
        bounty.status = "disputed".to_string();
    }
}

/// Update a bounty's assignee after a reassignment observed by the indexer.
#[allow(dead_code)] // Called by the indexer, which the binary does not compile.
pub fn mark_bounty_reassigned(db: &SharedDb, bounty_id: &str, new_assignee: &str) {
    if let Some(bounty) = acquire_db(db)
        .bounties
        .iter_mut()
        .find(|bounty| bounty.id == bounty_id)
    {
        bounty.assignee = Some(new_assignee.to_string());
    }
}

/// Update a bounty's status to `open` after the indexer observes a reopen.
#[allow(dead_code)] // Called by the indexer, which the binary does not compile.
pub fn mark_bounty_reopened(db: &SharedDb, bounty_id: &str) {
    if let Some(bounty) = acquire_db(db)
        .bounties
        .iter_mut()
        .find(|bounty| bounty.id == bounty_id)
    {
        bounty.status = "open".to_string();
        bounty.assignee = None;
    }
}

/// Update a bounty's status to `archived` after the indexer observes archival.
#[allow(dead_code)] // Called by the indexer, which the binary does not compile.
pub fn mark_bounty_archived(db: &SharedDb, bounty_id: &str) {
    if let Some(bounty) = acquire_db(db)
        .bounties
        .iter_mut()
        .find(|bounty| bounty.id == bounty_id)
    {
        bounty.status = "archived".to_string();
    }
}

/// Update a bounty's assignee to None after a stale claim is released.
#[allow(dead_code)] // Called by the indexer, which the binary does not compile.
pub fn mark_claim_released(db: &SharedDb, bounty_id: &str) {
    if let Some(bounty) = acquire_db(db)
        .bounties
        .iter_mut()
        .find(|bounty| bounty.id == bounty_id)
    {
        bounty.status = "open".to_string();
        bounty.assignee = None;
    }
}

/// Update a bounty's owner (creator field) after an ownership transfer.
#[allow(dead_code)] // Called by the indexer, which the binary does not compile.
pub fn mark_ownership_transferred(db: &SharedDb, bounty_id: &str, new_owner: &str) {
    if let Some(bounty) = acquire_db(db)
        .bounties
        .iter_mut()
        .find(|bounty| bounty.id == bounty_id)
    {
        bounty.creator = new_owner.to_string();
    }
}

/// Record that a reminder was published. Returns false if already recorded.
pub fn mark_deadline_reminder_sent(db: &SharedDb, bounty_id: String) -> bool {
    acquire_db(db).deadline_reminders_sent.insert(bounty_id)
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
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
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
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub assignee: Option<String>,
    pub created_at: DateTime<Utc>,
    /// Reward amount, mirrored from the contract's `Bounty::reward_amount`.
    pub reward: i128,
    /// Contract token address and precision used to format the reward. Also
    /// keeps aggregate totals in a single unit; older indexed rows may not
    /// have this value yet.
    #[serde(default)]
    pub reward_token: Option<String>,
    #[serde(default = "default_reward_decimals")]
    pub reward_decimals: u32,
    /// Optional expiry, mirrored from the contract's `Bounty::deadline`. `None`
    /// means the bounty never expires.
    pub deadline: Option<DateTime<Utc>>,
    /// Lifecycle status (`open`, `claimed`, `submitted`, `completed`, `cancelled`).
    #[serde(default = "default_bounty_status")]
    pub status: String,
    /// Tags attached to the bounty, used by the `tag` listing filter.
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ReputationHistoryEntry {
    pub event_id: String,
    pub contributor_address: String,
    pub ledger: u32,
    pub timestamp: DateTime<Utc>,
    pub old_value: u32,
    pub new_value: u32,
    pub reason: String,
}

#[allow(dead_code)] // Called by the indexer, which the binary does not compile.
pub fn record_reputation_change(db: &SharedDb, entry: ReputationHistoryEntry) -> bool {
    let mut guard = acquire_db(db);
    if guard
        .reputation_history
        .iter()
        .any(|existing| existing.event_id == entry.event_id)
    {
        return false;
    }
    guard.reputation_history.push(entry);
    true
}

pub async fn query_reputation_history(
    db: &SharedDb,
    contributor_address: &str,
    offset: usize,
    limit: usize,
) -> Result<Vec<ReputationHistoryEntry>, sqlx::Error> {
    let pool = read_db(db).postgres.clone();
    if let Some(pool) = pool {
        let rows = sqlx::query(
            "SELECT event_id, contributor_address, ledger, timestamp, old_value, new_value, reason \
             FROM reputation_history WHERE contributor_address = $1 \
             ORDER BY ledger DESC, event_id DESC OFFSET $2 LIMIT $3",
        )
        .bind(contributor_address)
        .bind(i64::try_from(offset).unwrap_or(i64::MAX))
        .bind(i64::try_from(limit).unwrap_or(i64::MAX))
        .fetch_all(&pool)
        .await?;

        return rows
            .into_iter()
            .map(|row| {
                use sqlx::Row;
                Ok(ReputationHistoryEntry {
                    event_id: row.try_get("event_id")?,
                    contributor_address: row.try_get("contributor_address")?,
                    ledger: u32::try_from(row.try_get::<i64, _>("ledger")?)
                        .map_err(|error| sqlx::Error::Decode(Box::new(error)))?,
                    timestamp: row.try_get("timestamp")?,
                    old_value: u32::try_from(row.try_get::<i64, _>("old_value")?)
                        .map_err(|error| sqlx::Error::Decode(Box::new(error)))?,
                    new_value: u32::try_from(row.try_get::<i64, _>("new_value")?)
                        .map_err(|error| sqlx::Error::Decode(Box::new(error)))?,
                    reason: row.try_get("reason")?,
                })
            })
            .collect();
    }

    let guard = read_db(db);
    let mut entries: Vec<_> = guard
        .reputation_history
        .iter()
        .filter(|entry| entry.contributor_address == contributor_address)
        .cloned()
        .collect();
    entries
        .sort_by(|left, right| (right.ledger, &right.event_id).cmp(&(left.ledger, &left.event_id)));
    Ok(entries.into_iter().skip(offset).take(limit).collect())
}

pub fn set_postgres_pool(db: &SharedDb, pool: sqlx::PgPool) {
    acquire_db(db).postgres = Some(pool);
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct TokenTotal {
    pub token_address: String,
    pub amount: i128,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct StatsResponse {
    pub open_bounties: u64,
    pub total_paid_out: Vec<TokenTotal>,
    pub active_contributors: u64,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct LeaderboardEntry {
    pub address: String,
    pub completed_bounties: u32,
    pub reputation: u32,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct LeaderboardPage {
    pub entries: Vec<LeaderboardEntry>,
}

pub fn get_leaderboard(db: &SharedDb, limit: i64) -> LeaderboardPage {
    let guard = read_db(db);
    let mut scores = HashMap::<String, u32>::new();
    for bounty in &guard.bounties {
        if let Some(address) = bounty
            .assignee
            .as_deref()
            .filter(|address| !address.is_empty())
        {
            *scores.entry(address.to_owned()).or_default() += 1;
        }
    }
    let mut entries: Vec<_> = scores
        .into_iter()
        .map(|(address, reputation)| LeaderboardEntry {
            address,
            completed_bounties: reputation,
            reputation,
        })
        .collect();
    entries.sort_by(|left, right| {
        right
            .reputation
            .cmp(&left.reputation)
            .then_with(|| left.address.cmp(&right.address))
    });
    entries.truncate(usize::try_from(limit.max(0)).unwrap_or(usize::MAX));
    LeaderboardPage { entries }
}

#[derive(Debug, Clone)]
struct StatsCacheEntry {
    data: StatsResponse,
    expires_at: Instant,
}

/// Return platform aggregates, retaining the result for the requested TTL.
/// Payout totals are separated by reward token so unlike units are never added.
pub fn get_stats(db: &SharedDb, ttl: Duration) -> StatsResponse {
    let mut guard = acquire_db(db);
    if let Some(cached) = &guard.stats_cache {
        if Instant::now() < cached.expires_at {
            return cached.data.clone();
        }
    }

    let mut totals = std::collections::BTreeMap::<String, i128>::new();
    let mut contributors = std::collections::HashSet::new();
    let mut open_bounties = 0_u64;
    for bounty in &guard.bounties {
        if bounty.status == "open" {
            open_bounties += 1;
        }
        if let Some(assignee) = bounty.assignee.as_deref().filter(|value| !value.is_empty()) {
            contributors.insert(assignee.to_string());
        }
        if bounty.status == "completed" {
            if let Some(token) = bounty.reward_token.as_deref() {
                *totals.entry(token.to_string()).or_default() += bounty.reward;
            }
        }
    }

    let data = StatsResponse {
        open_bounties,
        total_paid_out: totals
            .into_iter()
            .map(|(token_address, amount)| TokenTotal {
                token_address,
                amount,
            })
            .collect(),
        active_contributors: contributors.len() as u64,
    };
    guard.stats_cache = Some(StatsCacheEntry {
        data: data.clone(),
        expires_at: Instant::now() + ttl,
    });
    data
}

fn default_bounty_status() -> String {
    "open".to_string()
}

fn default_reward_decimals() -> u32 {
    7
}

impl Default for Bounty {
    fn default() -> Self {
        Bounty {
            id: String::new(),
            creator: String::new(),
            title: String::new(),
            description: String::new(),
            assignee: None,
            created_at: DateTime::<Utc>::default(),
            reward: 0,
            reward_token: None,
            reward_decimals: default_reward_decimals(),
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
#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
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
#[tracing::instrument(skip(db, creator), fields(limit, sort = ?sort, order = ?order))]
pub fn list_bounties_by_creator(
    db: &SharedDb,
    creator: &str,
    limit: i64,
    cursor: Option<DateTime<Utc>>,
    status: Option<&str>,
    tag: Option<&str>,
    query: Option<&str>,
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
        .filter(|b| {
            query.is_none_or(|q| {
                let searchable = format!("{} {}", b.title, b.description).to_lowercase();
                q.split_whitespace()
                    .all(|term| searchable.contains(&term.to_lowercase()))
            })
        })
        .filter(|b| cursor.is_none_or(|c| b.created_at < c))
        .cloned()
        .collect();
    apply_sort(&mut matches, sort, order);
    paginate(matches, limit)
}

/// Return a sorted export window without cloning the complete matching set.
/// The temporary selection is bounded by `offset + limit`, which the route
/// caps before calling this function.
#[allow(clippy::too_many_arguments)]
pub fn list_bounties_export_page(
    db: &SharedDb,
    offset: usize,
    limit: usize,
    cursor: Option<DateTime<Utc>>,
    status: Option<&str>,
    tag: Option<&str>,
    sort: BountySortField,
    order: SortOrder,
) -> Vec<Bounty> {
    let target = offset.saturating_add(limit);
    if target == 0 {
        return Vec::new();
    }

    let guard = read_db(db);
    let mut selected = Vec::with_capacity(target);
    for (source_index, bounty) in guard.bounties.iter().enumerate() {
        if cursor.is_some_and(|value| bounty.created_at >= value)
            || status.is_some_and(|value| bounty.status != value)
            || tag.is_some_and(|value| !bounty.tags.iter().any(|item| item == value))
        {
            continue;
        }

        let position = selected.partition_point(|(index, existing)| {
            let comparison = compare_bounties(existing, bounty, sort, order);
            comparison == std::cmp::Ordering::Less
                || (comparison == std::cmp::Ordering::Equal && *index < source_index)
        });
        if position < target {
            selected.insert(position, (source_index, bounty.clone()));
            if selected.len() > target {
                selected.pop();
            }
        }
    }

    selected
        .into_iter()
        .skip(offset)
        .map(|(_, bounty)| bounty)
        .collect()
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
    bounties.sort_by(|a, b| compare_bounties(a, b, sort, order));
}

fn compare_bounties(
    left: &Bounty,
    right: &Bounty,
    sort: BountySortField,
    order: SortOrder,
) -> std::cmp::Ordering {
    let primary = if sort == BountySortField::Deadline {
        match (left.deadline, right.deadline) {
            (Some(a), Some(b)) => match order {
                SortOrder::Asc => a.cmp(&b),
                SortOrder::Desc => b.cmp(&a),
            },
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        }
    } else {
        let value = match sort {
            BountySortField::Created => left.created_at.cmp(&right.created_at),
            BountySortField::Reward => left.reward.cmp(&right.reward),
            BountySortField::Deadline => unreachable!("handled above"),
        };
        match order {
            SortOrder::Asc => value,
            SortOrder::Desc => value.reverse(),
        }
    };

    primary
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

/// Query audit logs with inclusive timestamp filters and a stable descending
/// `(timestamp, id)` cursor. The caller requests one extra row to determine
/// whether another page exists.
#[allow(clippy::too_many_arguments)]
pub fn query_audit_logs(
    db: &SharedDb,
    actor: Option<&str>,
    action: Option<&str>,
    target: Option<&str>,
    from: Option<u64>,
    to: Option<u64>,
    cursor: Option<(u64, i64)>,
    limit: i64,
) -> Vec<AuditLog> {
    let guard = read_db(db);
    let mut logs: Vec<AuditLog> = guard
        .audit_logs
        .iter()
        .filter(|log| actor.is_none_or(|a| log.actor == a))
        .filter(|log| action.is_none_or(|a| log.action == a))
        .filter(|log| target.is_none_or(|t| log.target == t))
        .filter(|log| from.is_none_or(|timestamp| log.timestamp >= timestamp))
        .filter(|log| to.is_none_or(|timestamp| log.timestamp <= timestamp))
        .filter(|log| {
            cursor.is_none_or(|(timestamp, id)| {
                (log.timestamp, log.id.unwrap_or_default()) < (timestamp, id)
            })
        })
        .cloned()
        .collect();

    logs.sort_by(|a, b| {
        (b.timestamp, b.id.unwrap_or_default()).cmp(&(a.timestamp, a.id.unwrap_or_default()))
    });

    let limit = usize::try_from(limit).unwrap_or(usize::MAX);
    logs.truncate(limit);
    logs
}

/// Add an audit log entry to the store.
pub fn write_audit_log(db: &SharedDb, log: AuditLog) {
    let mut guard = acquire_db(db);
    let mut log = log;
    if log.id.is_none() {
        log.id = Some(
            guard
                .audit_logs
                .iter()
                .filter_map(|entry| entry.id)
                .max()
                .unwrap_or_default()
                + 1,
        );
    }
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
// Idempotency-key store (#1263)
// ---------------------------------------------------------------------------

/// How long a Postgres `in_flight` reservation remains valid before a retry
/// may reclaim it. The RAII-free route handler can be cancelled or crash
/// mid-flight, so this TTL is the backstop that prevents a permanently stuck
/// key.
pub const IDEMPOTENCY_IN_FLIGHT_TTL: Duration = Duration::from_secs(60);

/// How long a cached `completed` response is replayable before it is removed.
pub const IDEMPOTENCY_COMPLETED_TTL: Duration = Duration::from_secs(60 * 60);

/// Proof that a request owns an `in_flight` reservation.
///
/// The token is generated when the key is claimed and must be presented when
/// the result is finalized. Without it, a slow request whose reservation
/// expired could overwrite a newer request's outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdempotencyReservation {
    pub key: String,
    pub token: String,
}

/// Result of claiming an `Idempotency-Key`, shared by both store backends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdempotencyOutcome {
    /// The caller owns the key and may submit the transaction.
    Proceed(IdempotencyReservation),
    /// A prior successful request with the same key and payload was replayed.
    Replay(String),
    /// The same key is already being processed by another request.
    Conflict,
    /// The key was previously used with a different request payload.
    PayloadMismatch,
}

/// Outcome recorded for a client-supplied `Idempotency-Key`.
///
/// Both variants keep the SHA-256 digest of the canonical request payload so
/// a replay can verify that the retry carries the same body. `created_at`
/// drives the state-specific TTLs; `Instant` is only used by the in-memory
/// fallback, while Postgres persists `expires_at` as `TIMESTAMPTZ`.
#[derive(Debug, Clone)]
pub enum IdempotencyEntry {
    InFlight {
        request_hash: String,
        token: String,
        created_at: Instant,
    },
    Completed {
        request_hash: String,
        body: String,
        created_at: Instant,
    },
}

impl IdempotencyEntry {
    /// When this entry was first written to the store.
    pub fn created_at(&self) -> Instant {
        match self {
            Self::InFlight { created_at, .. } | Self::Completed { created_at, .. } => *created_at,
        }
    }

    /// Digest of the payload that first claimed this key.
    pub fn request_hash(&self) -> &str {
        match self {
            Self::InFlight { request_hash, .. } | Self::Completed { request_hash, .. } => {
                request_hash
            }
        }
    }

    /// Whether this entry's age has reached the TTL for its state.
    pub fn is_expired(
        &self,
        now: Instant,
        in_flight_ttl: Duration,
        completed_ttl: Duration,
    ) -> bool {
        let ttl = match self {
            Self::InFlight { .. } => in_flight_ttl,
            Self::Completed { .. } => completed_ttl,
        };
        now.saturating_duration_since(self.created_at()) >= ttl
    }
}

/// In-memory record of recently-seen idempotency keys, keyed by the raw
/// `Idempotency-Key` header value.
///
/// Mirrors `DbStore`: a plain `HashMap` guarded by an `RwLock` kept separate
/// from `DbStore` so idempotency bookkeeping never contends with the
/// business-data lock. This is the fallback for single-instance deployments;
/// the Postgres backend in [`PostgresIdempotencyStore`] is authoritative when
/// a pool is configured.
#[derive(Debug)]
pub struct IdempotencyStore {
    pub entries: HashMap<String, IdempotencyEntry>,
    /// TTL for `InFlight` reservations (overridable in tests).
    pub in_flight_ttl: Duration,
    /// TTL for cached `Completed` responses (overridable in tests).
    pub completed_ttl: Duration,
}

impl Default for IdempotencyStore {
    fn default() -> Self {
        Self {
            entries: HashMap::new(),
            in_flight_ttl: IDEMPOTENCY_IN_FLIGHT_TTL,
            completed_ttl: IDEMPOTENCY_COMPLETED_TTL,
        }
    }
}

impl IdempotencyStore {
    /// Remove every entry whose age has reached the TTL for its state.
    pub fn prune_expired(&mut self, now: Instant) {
        let (in_flight_ttl, completed_ttl) = (self.in_flight_ttl, self.completed_ttl);
        self.entries
            .retain(|_, entry| !entry.is_expired(now, in_flight_ttl, completed_ttl));
    }

    /// Atomically claim `key` for `request_hash` or resolve what a retry
    /// should receive. Expired entries are reclaimed first, mirroring the
    /// Postgres backend's `expires_at` check.
    pub fn claim(&mut self, key: &str, request_hash: &str, now: Instant) -> IdempotencyOutcome {
        self.prune_expired(now);
        match self.entries.get(key) {
            Some(entry) if entry.request_hash() != request_hash => {
                IdempotencyOutcome::PayloadMismatch
            }
            Some(IdempotencyEntry::Completed { body, .. }) => {
                IdempotencyOutcome::Replay(body.clone())
            }
            Some(IdempotencyEntry::InFlight { .. }) => IdempotencyOutcome::Conflict,
            None => {
                let token = uuid::Uuid::new_v4().to_string();
                self.entries.insert(
                    key.to_string(),
                    IdempotencyEntry::InFlight {
                        request_hash: request_hash.to_string(),
                        token: token.clone(),
                        created_at: now,
                    },
                );
                IdempotencyOutcome::Proceed(IdempotencyReservation {
                    key: key.to_string(),
                    token,
                })
            }
        }
    }

    /// Cache a successful response for `key`. Returns `false` unless the
    /// caller still owns the matching in-flight reservation.
    pub fn complete(&mut self, key: &str, token: &str, body: String, now: Instant) -> bool {
        let Some(entry) = self.entries.get(key).cloned() else {
            return false;
        };
        let IdempotencyEntry::InFlight {
            request_hash,
            token: current_token,
            ..
        } = entry
        else {
            return false;
        };
        if current_token != token {
            return false;
        }
        self.entries.insert(
            key.to_string(),
            IdempotencyEntry::Completed {
                request_hash,
                body,
                created_at: now,
            },
        );
        true
    }

    /// Release a failed or cancelled reservation so the client can retry.
    /// A stale token cannot delete a newer request's reservation.
    pub fn release(&mut self, key: &str, token: &str) {
        let owns_reservation = matches!(
            self.entries.get(key),
            Some(IdempotencyEntry::InFlight {
                token: current_token,
                ..
            }) if current_token == token
        );
        if owns_reservation {
            self.entries.remove(key);
        }
    }
}

/// Shared, thread-safe handle to the in-memory idempotency store.
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

/// PostgreSQL-backed idempotency store shared by every backend replica.
///
/// All state transitions are expressed as a single SQL statement or a short
/// `SELECT ... FOR UPDATE` transaction so two replicas can never both win the
/// same key. Parameters are always bound, never interpolated into SQL.
#[derive(Debug, Clone)]
pub struct PostgresIdempotencyStore {
    pub pool: sqlx::PgPool,
    pub in_flight_ttl: Duration,
    pub completed_ttl: Duration,
}

impl PostgresIdempotencyStore {
    /// Use the standard TTLs with an existing connection pool.
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self {
            pool,
            in_flight_ttl: IDEMPOTENCY_IN_FLIGHT_TTL,
            completed_ttl: IDEMPOTENCY_COMPLETED_TTL,
        }
    }

    /// Claim a key, or report why this request cannot proceed.
    ///
    /// `INSERT ... ON CONFLICT DO NOTHING` is the fast path: exactly one
    /// replica inserts the reservation. On conflict the existing row is
    /// locked, its payload hash is checked, and an expired row is deleted
    /// inside the same transaction so a fresh insert can be attempted. This
    /// keeps "check hash + inspect status + reclaim TTL" atomic across
    /// replicas.
    pub async fn claim(
        &self,
        key: &str,
        request_hash: &str,
    ) -> Result<IdempotencyOutcome, sqlx::Error> {
        use sqlx::Row;

        let token = uuid::Uuid::new_v4().to_string();

        // Two attempts are enough: one insert plus at most one expired-row
        // reclaim. A third concurrent reclainer would itself win an insert.
        for _ in 0..2 {
            let mut tx = self.pool.begin().await?;

            let inserted = sqlx::query(
                "INSERT INTO idempotency_keys \
                 (key, request_hash, status, reservation_token, expires_at) \
                 VALUES ($1, $2, 'in_flight', $3, NOW() + make_interval(secs => $4)) \
                 ON CONFLICT (key) DO NOTHING",
            )
            .bind(key)
            .bind(request_hash)
            .bind(&token)
            .bind(self.in_flight_ttl.as_secs_f64())
            .execute(&mut *tx)
            .await?;

            if inserted.rows_affected() == 1 {
                tx.commit().await?;
                return Ok(IdempotencyOutcome::Proceed(IdempotencyReservation {
                    key: key.to_string(),
                    token: token.clone(),
                }));
            }

            let existing = sqlx::query(
                "SELECT request_hash, status, response_body, expires_at \
                 FROM idempotency_keys WHERE key = $1 FOR UPDATE",
            )
            .bind(key)
            .fetch_optional(&mut *tx)
            .await?;

            // The conflicting row was deleted between our insert and the
            // lock; retry the insert inside a fresh transaction.
            let Some(row) = existing else {
                tx.commit().await?;
                continue;
            };

            let stored_hash: String = row.try_get("request_hash")?;
            if stored_hash != request_hash {
                tx.commit().await?;
                return Ok(IdempotencyOutcome::PayloadMismatch);
            }

            let expires_at: DateTime<Utc> = row.try_get("expires_at")?;
            let outcome = if expires_at <= Utc::now() {
                sqlx::query("DELETE FROM idempotency_keys WHERE key = $1 AND expires_at <= NOW()")
                    .bind(key)
                    .execute(&mut *tx)
                    .await?;
                None
            } else {
                let status: String = row.try_get("status")?;
                if status == "completed" {
                    let body: Option<String> = row.try_get("response_body")?;
                    Some(
                        body.map(IdempotencyOutcome::Replay)
                            .unwrap_or(IdempotencyOutcome::Conflict),
                    )
                } else {
                    Some(IdempotencyOutcome::Conflict)
                }
            };

            tx.commit().await?;
            if let Some(outcome) = outcome {
                return Ok(outcome);
            }
        }

        Err(sqlx::Error::Protocol(
            "idempotency key reservation was contended too many times".to_string(),
        ))
    }

    /// Atomically transition an owned reservation to `completed` and cache
    /// its response body for replay. Returns `false` if the reservation was
    /// already released, so a stale request cannot overwrite a newer one.
    pub async fn complete(&self, key: &str, token: &str, body: &str) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            "UPDATE idempotency_keys \
             SET status = 'completed', response_body = $3, updated_at = NOW(), \
                 expires_at = NOW() + make_interval(secs => $4) \
             WHERE key = $1 AND status = 'in_flight' AND reservation_token = $2",
        )
        .bind(key)
        .bind(token)
        .bind(body)
        .bind(self.completed_ttl.as_secs_f64())
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    /// Release a failed or cancelled reservation so the key can be reused.
    /// The token guard prevents a stale request from deleting a newer claim.
    pub async fn release(&self, key: &str, token: &str) -> Result<(), sqlx::Error> {
        sqlx::query(
            "DELETE FROM idempotency_keys \
             WHERE key = $1 AND status = 'in_flight' AND reservation_token = $2",
        )
        .bind(key)
        .bind(token)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Delete every expired row. Intended for a periodic maintenance task;
    /// per-key reclaim during `claim` already keeps hot keys unblocked.
    pub async fn cleanup_expired(&self) -> Result<u64, sqlx::Error> {
        let result = sqlx::query("DELETE FROM idempotency_keys WHERE expires_at <= NOW()")
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }
}

/// The backend chosen for a request: shared Postgres in production, with the
/// in-memory map as the zero-configuration fallback.
#[derive(Clone)]
pub enum IdempotencyBackend {
    InMemory(SharedIdempotencyStore),
    Postgres(PostgresIdempotencyStore),
}

impl IdempotencyBackend {
    /// Claim `key` on the configured backend.
    pub async fn claim(
        &self,
        key: &str,
        request_hash: &str,
    ) -> Result<IdempotencyOutcome, sqlx::Error> {
        match self {
            Self::InMemory(store) => {
                Ok(acquire_idempotency(store).claim(key, request_hash, Instant::now()))
            }
            Self::Postgres(store) => store.claim(key, request_hash).await,
        }
    }

    /// Cache a successful response on the configured backend.
    pub async fn complete(
        &self,
        reservation: &IdempotencyReservation,
        body: &str,
    ) -> Result<bool, sqlx::Error> {
        match self {
            Self::InMemory(store) => Ok(acquire_idempotency(store).complete(
                &reservation.key,
                &reservation.token,
                body.to_string(),
                Instant::now(),
            )),
            Self::Postgres(store) => {
                store
                    .complete(&reservation.key, &reservation.token, body)
                    .await
            }
        }
    }

    /// Release a failed or cancelled reservation on the configured backend.
    pub async fn release(&self, reservation: &IdempotencyReservation) -> Result<(), sqlx::Error> {
        match self {
            Self::InMemory(store) => {
                acquire_idempotency(store).release(&reservation.key, &reservation.token);
                Ok(())
            }
            Self::Postgres(store) => store.release(&reservation.key, &reservation.token).await,
        }
    }
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
        assert!(AUDIT_LOGS_PAGINATION_MIGRATION
            .contains("ON audit_logs (action, timestamp DESC, id DESC)"));
    }

    #[test]
    fn idempotency_migration_declares_shared_key_ledger() {
        for fragment in [
            "CREATE TABLE IF NOT EXISTS idempotency_keys",
            "key TEXT PRIMARY KEY",
            "request_hash TEXT NOT NULL",
            "status TEXT NOT NULL",
            "reservation_token TEXT NOT NULL",
            "response_body TEXT",
            "expires_at TIMESTAMPTZ NOT NULL",
            "idx_idempotency_keys_expires_at",
        ] {
            assert!(
                IDEMPOTENCY_MIGRATION.contains(fragment),
                "idempotency migration must contain `{fragment}`"
            );
        }
    }

    fn assert_proceed(outcome: IdempotencyOutcome) -> IdempotencyReservation {
        match outcome {
            IdempotencyOutcome::Proceed(reservation) => reservation,
            other => panic!("expected a fresh reservation, got {other:?}"),
        }
    }

    #[test]
    fn in_memory_idempotency_store_replays_and_rejects_mismatched_payload() {
        let mut store = IdempotencyStore::default();
        let now = Instant::now();

        let reservation = assert_proceed(store.claim("key", "hash-a", now));
        assert_eq!(
            store.claim("key", "hash-b", now),
            IdempotencyOutcome::PayloadMismatch
        );
        assert!(store.complete("key", &reservation.token, "cached body".to_string(), now));
        assert_eq!(
            store.claim("key", "hash-a", now),
            IdempotencyOutcome::Replay("cached body".to_string())
        );
        assert_eq!(
            store.claim("key", "hash-b", now),
            IdempotencyOutcome::PayloadMismatch
        );
    }

    #[test]
    fn in_memory_idempotency_store_conflicts_while_in_flight() {
        let mut store = IdempotencyStore::default();
        let now = Instant::now();

        let reservation = assert_proceed(store.claim("key", "hash", now));
        assert_eq!(
            store.claim("key", "hash", now),
            IdempotencyOutcome::Conflict
        );

        store.release("key", &reservation.token);
        assert_proceed(store.claim("key", "hash", now));
    }

    #[test]
    fn stale_token_cannot_finalize_a_newer_in_memory_reservation() {
        let mut store = IdempotencyStore::default();
        store.in_flight_ttl = Duration::ZERO;
        let now = Instant::now();

        let stale = assert_proceed(store.claim("key", "hash", now));
        let fresh = assert_proceed(store.claim("key", "hash", now));

        assert!(!store.complete("key", &stale.token, "stale body".to_string(), now));
        store.release("key", &stale.token);
        assert!(store.complete("key", &fresh.token, "fresh body".to_string(), now));
        assert_eq!(
            store.claim("key", "hash", now),
            IdempotencyOutcome::Replay("fresh body".to_string())
        );
    }

    #[test]
    fn in_memory_idempotency_store_reclaims_expired_entries() {
        let mut store = IdempotencyStore::default();
        store.in_flight_ttl = Duration::from_secs(1);
        store.completed_ttl = Duration::from_secs(2);
        let now = Instant::now();

        assert_proceed(store.claim("in-flight", "hash", now));
        let completed = assert_proceed(store.claim("completed", "hash", now));
        assert!(store.complete("completed", &completed.token, "body".to_string(), now));

        let later = now + Duration::from_secs(1);
        assert_proceed(store.claim("in-flight", "hash", later));
        assert_eq!(
            store.claim("completed", "hash", later),
            IdempotencyOutcome::Replay("body".to_string()),
            "completed responses outlive in-flight reservations"
        );

        let much_later = now + Duration::from_secs(2);
        assert_proceed(store.claim("completed", "hash", much_later));
    }

    #[test]
    fn audit_log_pages_filter_time_and_continue_after_cursor() {
        let db = new_shared_db();
        for (timestamp, action) in [
            (10, "create"),
            (20, "delete"),
            (20, "create"),
            (30, "create"),
        ] {
            write_audit_log(
                &db,
                AuditLog::new(
                    "admin".to_string(),
                    action.to_string(),
                    "item".to_string(),
                    timestamp,
                ),
            );
        }

        let first_page =
            query_audit_logs(&db, None, Some("create"), None, Some(10), Some(30), None, 1);
        assert_eq!(first_page.len(), 1);
        assert_eq!(first_page[0].timestamp, 30);
        let cursor = (first_page[0].timestamp, first_page[0].id.unwrap());

        let second_page = query_audit_logs(
            &db,
            None,
            Some("create"),
            None,
            Some(10),
            Some(30),
            Some(cursor),
            10,
        );
        assert_eq!(second_page.len(), 2);
        assert_eq!(second_page[0].timestamp, 20);
        assert_eq!(second_page[1].timestamp, 10);
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

        let page = get_leaderboard(&db, 2);
        assert_eq!(page.entries.len(), 2);
    }
}
