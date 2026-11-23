//! Integration tests for the shared PostgreSQL idempotency store (#1263).
//!
//! These tests run against a real database when `DATABASE_URL` is set; in a
//! zero-configuration checkout they are skipped so `cargo test` still passes
//! with only the in-memory fallback available.

use std::time::Duration;

use mergemint_backend::db::{IdempotencyOutcome, IdempotencyReservation, PostgresIdempotencyStore};
use sqlx::{postgres::PgPoolOptions, PgPool};

async fn test_store(prefix: &str) -> Option<PostgresIdempotencyStore> {
    let Ok(url) = std::env::var("DATABASE_URL") else {
        eprintln!("DATABASE_URL is not set; skipping PostgreSQL idempotency test");
        return None;
    };
    if url.trim().is_empty() {
        eprintln!("DATABASE_URL is empty; skipping PostgreSQL idempotency test");
        return None;
    }

    let pool: PgPool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await
        .expect("connect to test database");
    for statement in include_str!("../migrations/0010_create_idempotency_keys.sql")
        .split(';')
        .map(str::trim)
        .filter(|sql| !sql.is_empty())
    {
        sqlx::query(statement)
            .execute(&pool)
            .await
            .expect("apply idempotency migration");
    }
    sqlx::query("DELETE FROM idempotency_keys WHERE key LIKE $1")
        .bind(format!("{prefix}%"))
        .execute(&pool)
        .await
        .expect("clear test keys");

    Some(PostgresIdempotencyStore {
        pool,
        in_flight_ttl: Duration::from_secs(60),
        completed_ttl: Duration::from_secs(60 * 60),
    })
}

fn assert_proceed(outcome: IdempotencyOutcome) -> IdempotencyReservation {
    match outcome {
        IdempotencyOutcome::Proceed(reservation) => reservation,
        other => panic!("expected a fresh reservation, got {other:?}"),
    }
}

#[tokio::test]
async fn postgres_replays_completed_response_and_rejects_payload_reuse() {
    let Some(store) = test_store("pg-replay-").await else {
        return;
    };
    let key = "pg-replay-key";

    let reservation = assert_proceed(store.claim(key, "hash-a").await.expect("claim"));
    assert_eq!(
        store.claim(key, "hash-b").await.expect("claim"),
        IdempotencyOutcome::PayloadMismatch,
        "a different payload must not replay an unrelated response"
    );
    assert_eq!(
        store.claim(key, "hash-a").await.expect("claim"),
        IdempotencyOutcome::Conflict,
        "a second in-flight request must not submit the transaction again"
    );

    assert!(store
        .complete(key, &reservation.token, r#"{"ok":true}"#)
        .await
        .expect("complete"));
    assert_eq!(
        store.claim(key, "hash-a").await.expect("claim"),
        IdempotencyOutcome::Replay(r#"{"ok":true}"#.to_string())
    );
    assert_eq!(
        store.claim(key, "hash-b").await.expect("claim"),
        IdempotencyOutcome::PayloadMismatch
    );
}

#[tokio::test]
async fn postgres_concurrent_claims_have_exactly_one_winner() {
    let Some(store) = test_store("pg-concurrent-").await else {
        return;
    };
    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(4));
    let mut tasks = Vec::new();

    for _ in 0..4 {
        let store = store.clone();
        let barrier = std::sync::Arc::clone(&barrier);
        tasks.push(tokio::spawn(async move {
            barrier.wait().await;
            store
                .claim("pg-concurrent-key", "same-hash")
                .await
                .expect("claim")
        }));
    }

    let mut outcomes = Vec::new();
    for task in tasks {
        outcomes.push(task.await.expect("claim task"));
    }
    assert_eq!(
        outcomes
            .iter()
            .filter(|outcome| matches!(outcome, IdempotencyOutcome::Proceed(_)))
            .count(),
        1,
        "exactly one replica may own the key"
    );
    assert_eq!(
        outcomes
            .iter()
            .filter(|outcome| **outcome == IdempotencyOutcome::Conflict)
            .count(),
        3,
        "every other concurrent request must be rejected"
    );
}

#[tokio::test]
async fn postgres_expired_reservations_are_reclaimable_and_cleaned_up() {
    let Some(base) = test_store("pg-ttl-").await else {
        return;
    };
    let completed_ttl = base.completed_ttl;
    let store = PostgresIdempotencyStore {
        pool: base.pool,
        in_flight_ttl: Duration::from_millis(1),
        completed_ttl,
    };

    assert_proceed(store.claim("pg-ttl-key", "hash").await.expect("claim"));
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert_proceed(store.claim("pg-ttl-key", "hash").await.expect("claim"));

    sqlx::query(
        "UPDATE idempotency_keys SET expires_at = NOW() - INTERVAL '1 second' WHERE key = $1",
    )
    .bind("pg-ttl-key")
    .execute(&store.pool)
    .await
    .expect("expire row");
    let removed = store.cleanup_expired().await.expect("cleanup");
    assert!(
        removed >= 1,
        "the periodic cleanup must delete expired rows"
    );
}

#[tokio::test]
async fn postgres_release_allows_a_failed_request_to_retry_the_same_key() {
    let Some(store) = test_store("pg-release-").await else {
        return;
    };

    let reservation = assert_proceed(store.claim("pg-release-key", "hash").await.expect("claim"));
    store
        .release("pg-release-key", &reservation.token)
        .await
        .expect("release");
    assert_proceed(store.claim("pg-release-key", "hash").await.expect("claim"));
}

#[tokio::test]
async fn postgres_stale_token_cannot_finalize_a_newer_reservation() {
    let Some(base) = test_store("pg-stale-").await else {
        return;
    };
    let completed_ttl = base.completed_ttl;
    let store = PostgresIdempotencyStore {
        pool: base.pool,
        in_flight_ttl: Duration::from_millis(1),
        completed_ttl,
    };
    let key = "pg-stale-key";

    let stale = assert_proceed(store.claim(key, "hash").await.expect("claim"));
    tokio::time::sleep(Duration::from_millis(20)).await;
    let fresh = assert_proceed(store.claim(key, "hash").await.expect("claim"));

    assert!(!store
        .complete(key, &stale.token, "stale body")
        .await
        .expect("stale complete"));
    store
        .release(key, &stale.token)
        .await
        .expect("stale release");
    assert!(store
        .complete(key, &fresh.token, "fresh body")
        .await
        .expect("fresh complete"));
    assert_eq!(
        store.claim(key, "hash").await.expect("replay"),
        IdempotencyOutcome::Replay("fresh body".to_string())
    );
}
