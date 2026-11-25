// mergemint-backend/tests/idempotency_postgres.rs
//
// Integration tests for the shared PostgreSQL idempotency ledger (#1263).
// Each test uses a unique key so tests can run concurrently against one DB.
// When DATABASE_URL is unset the tests skip instead of failing, keeping the
// default `cargo test` run zero-configuration.

use std::time::Duration;

use mergemint_backend::db::{
    claim_idempotency_key, cleanup_expired_idempotency_keys, finalize_idempotency_key,
    new_shared_idempotency_store, request_fingerprint, IdempotencyClaim, SharedIdempotencyStore,
    IDEMPOTENCY_TTL,
};
use serde_json::{json, Value};
use sqlx::{postgres::PgPoolOptions, PgPool};

static MIGRATION_LOCK: std::sync::OnceLock<tokio::sync::Mutex<()>> = std::sync::OnceLock::new();

async fn shared_pool() -> Option<PgPool> {
    let Ok(url) = std::env::var("DATABASE_URL") else {
        eprintln!("DATABASE_URL is unset; skipping PostgreSQL idempotency test");
        return None;
    };
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&url)
        .await
        .expect("failed to connect to DATABASE_URL");

    // Tests run concurrently; `CREATE TABLE IF NOT EXISTS` is not race-safe
    // against another connection creating the same catalog entry (#1263).
    let _migration_guard = MIGRATION_LOCK
        .get_or_init(|| tokio::sync::Mutex::new(()))
        .lock()
        .await;
    for statement in include_str!("../migrations/0010_create_idempotency_keys.sql")
        .split(';')
        .filter(|statement| !statement.trim().is_empty())
    {
        sqlx::query(statement)
            .execute(&pool)
            .await
            .expect("failed to apply idempotency migration");
    }
    Some(pool)
}

fn unique_key() -> String {
    format!("it-{}", uuid::Uuid::new_v4())
}

fn fingerprint(payload: &Value) -> String {
    request_fingerprint(payload).expect("fingerprint must serialize")
}

async fn claim(
    pool: &PgPool,
    store: &SharedIdempotencyStore,
    key: &str,
    hash: &str,
    ttl: Duration,
) -> IdempotencyClaim {
    claim_idempotency_key(Some(pool), store, key, hash, ttl)
        .await
        .expect("claim must succeed")
}

#[tokio::test]
async fn postgres_replays_completed_response_for_same_payload() {
    let Some(pool) = shared_pool().await else {
        return;
    };
    let key = unique_key();
    let hash = fingerprint(&json!({"bounty_id": "replay", "claimant": "alice"}));
    let memory = new_shared_idempotency_store();
    let body = r#"{"ok":true}"#;

    assert_eq!(
        claim(&pool, &memory, &key, &hash, IDEMPOTENCY_TTL).await,
        IdempotencyClaim::Proceed
    );
    finalize_idempotency_key(
        Some(&pool),
        &memory,
        &key,
        &hash,
        Some(body.to_string()),
        IDEMPOTENCY_TTL,
    )
    .await
    .expect("finalize must succeed");
    assert_eq!(
        claim(&pool, &memory, &key, &hash, IDEMPOTENCY_TTL).await,
        IdempotencyClaim::Replay(body.to_string())
    );
}

#[tokio::test]
async fn postgres_reports_conflict_for_in_flight_key() {
    let Some(pool) = shared_pool().await else {
        return;
    };
    let key = unique_key();
    let hash = fingerprint(&json!({"bounty_id": "busy", "claimant": "alice"}));
    let memory = new_shared_idempotency_store();

    assert_eq!(
        claim(&pool, &memory, &key, &hash, IDEMPOTENCY_TTL).await,
        IdempotencyClaim::Proceed
    );
    // A second replica must see the in-flight reservation, not submit again.
    let other_replica = new_shared_idempotency_store();
    assert_eq!(
        claim(&pool, &other_replica, &key, &hash, IDEMPOTENCY_TTL).await,
        IdempotencyClaim::InFlight
    );
    finalize_idempotency_key(Some(&pool), &memory, &key, &hash, None, IDEMPOTENCY_TTL)
        .await
        .expect("release must succeed");
}

#[tokio::test]
async fn postgres_rejects_key_reuse_with_a_different_payload() {
    let Some(pool) = shared_pool().await else {
        return;
    };
    let key = unique_key();
    let first_hash = fingerprint(&json!({"bounty_id": "mismatch", "claimant": "alice"}));
    let other_hash = fingerprint(&json!({"bounty_id": "mismatch", "claimant": "bob"}));
    let memory = new_shared_idempotency_store();

    assert_eq!(
        claim(&pool, &memory, &key, &first_hash, IDEMPOTENCY_TTL).await,
        IdempotencyClaim::Proceed
    );
    finalize_idempotency_key(
        Some(&pool),
        &memory,
        &key,
        &first_hash,
        Some("original response".to_string()),
        IDEMPOTENCY_TTL,
    )
    .await
    .expect("finalize must succeed");

    assert_eq!(
        claim(&pool, &memory, &key, &other_hash, IDEMPOTENCY_TTL).await,
        IdempotencyClaim::PayloadMismatch
    );
}

#[tokio::test]
async fn postgres_concurrent_claims_have_exactly_one_winner() {
    let Some(pool) = shared_pool().await else {
        return;
    };
    let key = unique_key();
    let hash = fingerprint(&json!({"bounty_id": "race", "claimant": "alice"}));

    let mut attempts = Vec::new();
    for _ in 0..8 {
        let pool = pool.clone();
        let key = key.clone();
        let hash = hash.clone();
        attempts.push(tokio::spawn(async move {
            claim_idempotency_key(
                Some(&pool),
                &new_shared_idempotency_store(),
                &key,
                &hash,
                IDEMPOTENCY_TTL,
            )
            .await
        }));
    }

    let mut winners = 0;
    for attempt in attempts {
        match attempt.await.expect("claim task must not panic") {
            Ok(IdempotencyClaim::Proceed) => winners += 1,
            Ok(IdempotencyClaim::InFlight) => {}
            other => panic!("unexpected claim outcome: {other:?}"),
        }
    }
    assert_eq!(winners, 1, "only one replica may own a key");

    finalize_idempotency_key(
        Some(&pool),
        &new_shared_idempotency_store(),
        &key,
        &hash,
        None,
        IDEMPOTENCY_TTL,
    )
    .await
    .expect("release must succeed");
}

#[tokio::test]
async fn postgres_reclaims_an_expired_reservation() {
    let Some(pool) = shared_pool().await else {
        return;
    };
    let key = unique_key();
    let first_hash = fingerprint(&json!({"bounty_id": "expiry", "claimant": "alice"}));
    let retry_hash = fingerprint(&json!({"bounty_id": "expiry", "claimant": "bob"}));
    let memory = new_shared_idempotency_store();

    assert_eq!(
        claim(&pool, &memory, &key, &first_hash, Duration::from_millis(10)).await,
        IdempotencyClaim::Proceed
    );
    tokio::time::sleep(Duration::from_millis(100)).await;

    // The expired row is removed atomically and the retry claims the key
    // with its own payload hash instead of getting a false mismatch.
    assert_eq!(
        claim(&pool, &memory, &key, &retry_hash, IDEMPOTENCY_TTL).await,
        IdempotencyClaim::Proceed
    );
    finalize_idempotency_key(
        Some(&pool),
        &memory,
        &key,
        &retry_hash,
        None,
        IDEMPOTENCY_TTL,
    )
    .await
    .expect("release must succeed");
}

#[tokio::test]
async fn postgres_cleanup_removes_expired_rows() {
    let Some(pool) = shared_pool().await else {
        return;
    };
    let key = unique_key();
    sqlx::query(
        "INSERT INTO idempotency_keys (key, request_hash, status, response_body, expires_at) \
         VALUES ($1, $2, 'completed', $3, $4)",
    )
    .bind(&key)
    .bind("stale-hash")
    .bind(Some("stale response"))
    .bind(chrono::Utc::now() - chrono::Duration::hours(1))
    .execute(&pool)
    .await
    .expect("insert must succeed");

    let removed = cleanup_expired_idempotency_keys(&pool)
        .await
        .expect("cleanup must succeed");
    assert!(removed >= 1, "the stale row must be swept");

    let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM idempotency_keys WHERE key = $1")
        .bind(&key)
        .fetch_one(&pool)
        .await
        .expect("count must succeed");
    assert_eq!(remaining, 0);
}
