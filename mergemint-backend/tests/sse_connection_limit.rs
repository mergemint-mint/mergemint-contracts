// mergemint-backend/tests/sse_connection_limit.rs
//
// End-to-end tests for the `GET /bounties/stream` connection caps (#1118):
// a real server and real HTTP clients check that the per-IP and global caps
// return 429, and that a slot is freed once its client disconnects.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::{routing::get, Router};
use mergemint_backend::db::{new_shared_db, new_shared_idempotency_store};
use mergemint_backend::routes::bounties::bounty_stream;
use mergemint_backend::sse_limit::SseConnectionLimiter;
use mergemint_backend::AppState;
use reqwest::StatusCode;
use tokio::net::TcpListener;

fn state_with(limiter: SseConnectionLimiter) -> Arc<AppState> {
    Arc::new(AppState {
        db: new_shared_db(),
        postgres: None,
        idempotency: new_shared_idempotency_store(),
        rate_limiter: mergemint_backend::routes::tx::new_shared_rate_limiter(),
        bounty_broadcast: tokio::sync::broadcast::channel(16).0,
        leaderboard_cache: mergemint_backend::routes::leaderboard::new_leaderboard_cache(),
        // Short keep-alive so the server notices a closed client quickly.
        sse_keep_alive_duration: Duration::from_millis(50),
        webhook_guard: Default::default(),
        sse_limiter: limiter,
    })
}

/// Serve `bounty_stream` with connect-info, as `main` does.
async fn spawn_server(state: Arc<AppState>) -> String {
    let app = Router::new()
        .route("/stream", get(bounty_stream))
        .with_state(state);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .unwrap();
    });
    format!("http://{addr}/stream")
}

/// A fresh client per connection, so each SSE stream has its own socket.
async fn open(url: &str) -> reqwest::Response {
    reqwest::Client::new().get(url).send().await.unwrap()
}

/// Poll until `cond` holds or two seconds pass.
async fn wait_for(mut cond: impl FnMut() -> bool) -> bool {
    for _ in 0..80 {
        if cond() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    cond()
}

#[tokio::test]
async fn per_ip_cap_returns_429_and_frees_slot_on_disconnect() {
    let limiter = SseConnectionLimiter::new(2, 100);
    let url = spawn_server(state_with(limiter.clone())).await;

    let first = open(&url).await;
    let second = open(&url).await;
    assert_eq!(first.status(), StatusCode::OK);
    assert_eq!(second.status(), StatusCode::OK);
    assert_eq!(limiter.active(), 2);

    let rejected = open(&url).await;
    assert_eq!(rejected.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(
        rejected
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok()),
        Some("5")
    );
    let body: serde_json::Value = serde_json::from_slice(&rejected.bytes().await.unwrap()).unwrap();
    assert_eq!(body["code"], 429);
    assert_eq!(
        limiter.active(),
        2,
        "a rejected request must not hold a slot"
    );

    // Disconnect one client; its slot must be freed.
    drop(first);
    assert!(
        wait_for(|| limiter.active() == 1).await,
        "slot not released after disconnect (active = {})",
        limiter.active()
    );

    let reconnect = open(&url).await;
    assert_eq!(reconnect.status(), StatusCode::OK);
    assert_eq!(limiter.active(), 2);

    drop((second, reconnect));
    assert!(wait_for(|| limiter.active() == 0).await);
}

#[tokio::test]
async fn global_cap_returns_429() {
    let limiter = SseConnectionLimiter::new(10, 1);
    let url = spawn_server(state_with(limiter.clone())).await;

    let held = open(&url).await;
    assert_eq!(held.status(), StatusCode::OK);
    assert_eq!(open(&url).await.status(), StatusCode::TOO_MANY_REQUESTS);

    drop(held);
    assert!(wait_for(|| limiter.active() == 0).await);
    assert_eq!(open(&url).await.status(), StatusCode::OK);
}

#[tokio::test]
async fn accepted_stream_still_delivers_events() {
    let limiter = SseConnectionLimiter::new(1, 1);
    let state = state_with(limiter);
    let url = spawn_server(state.clone()).await;

    let mut response = open(&url).await;
    assert_eq!(response.status(), StatusCode::OK);
    let broadcast = state.bounty_broadcast.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let _ = broadcast.send("bounty-42".to_string());
    });

    let mut buf = String::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    while !buf.contains("bounty-42") {
        let chunk = tokio::time::timeout_at(deadline, response.chunk())
            .await
            .expect("event not received in time")
            .unwrap()
            .expect("stream closed early");
        buf.push_str(&String::from_utf8_lossy(&chunk));
    }
    assert!(buf.contains(r#"data: {"bountyId":"bounty-42"}"#));
}
