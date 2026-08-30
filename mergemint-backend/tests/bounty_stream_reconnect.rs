// mergemint-backend/tests/bounty_stream_reconnect.rs
//
// Integration test for the `GET /api/v1/bounties/stream` SSE endpoint's
// reconnect behavior.
//
// `bounty_stream` (src/routes/bounties.rs) fans out bounty-update
// notifications to every connected client via a `tokio::sync::broadcast`
// channel: each SSE connection calls `.subscribe()` for its own receiver.
// That means a client which drops its connection and reconnects gets a
// brand-new receiver rather than resuming an old one — so this test drives
// a real client against a real server, drops the connection mid-stream, and
// reconnects, asserting:
//
//   * an event broadcast while a connection is live is delivered to it
//     exactly once (no duplicates), and
//   * the reconnected client gets a working fresh subscription that
//     delivers events broadcast after it resubscribes.

use std::sync::Arc;
use std::time::Duration;

use axum::{routing::get, Router};
use mergemint_backend::db::{new_shared_db, new_shared_idempotency_store};
use mergemint_backend::routes::bounties::bounty_stream;
use mergemint_backend::AppState;
use tokio::net::TcpListener;

/// Start a real HTTP server exposing only `bounty_stream`, and return its
/// base URL. The server runs for the lifetime of the test process.
async fn spawn_test_server(state: Arc<AppState>) -> String {
    let app = Router::new()
        .route("/stream", get(bounty_stream))
        .with_state(state);

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("failed to bind test listener");
    let addr = listener.local_addr().expect("failed to read local addr");

    tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .expect("test server crashed");
    });

    format!("http://{addr}/stream")
}

/// Read raw SSE `data:` payloads off `response` until `count` have been
/// collected. Non-data lines (event names, keep-alive comments, blank
/// separators) are ignored.
async fn collect_events(mut response: reqwest::Response, count: usize) -> Vec<String> {
    let mut events = Vec::new();
    let mut buf = String::new();

    while events.len() < count {
        let chunk = response
            .chunk()
            .await
            .expect("error reading SSE chunk")
            .expect("stream ended before expected events arrived");
        buf.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(sep) = buf.find("\n\n") {
            let raw_event = buf[..sep].to_string();
            buf.drain(..sep + 2);
            for line in raw_event.lines() {
                if let Some(data) = line.strip_prefix("data:") {
                    events.push(data.trim().to_string());
                }
            }
        }
    }

    events
}

#[tokio::test]
async fn reconnect_after_drop_delivers_no_duplicate_events() {
    let state = Arc::new(AppState {
        db: new_shared_db(),
        idempotency: new_shared_idempotency_store(),
        bounty_broadcast: tokio::sync::broadcast::channel(16).0,
    });
    let url = spawn_test_server(state.clone()).await;
    let client = reqwest::Client::new();

    // ── First connection ────────────────────────────────────────────────
    let first_response = client.get(&url).send().await.expect("first connect failed");
    tokio::spawn({
        let broadcast = state.bounty_broadcast.clone();
        async move {
            // Give the server time to subscribe before publishing.
            tokio::time::sleep(Duration::from_millis(50)).await;
            let _ = broadcast.send("bounty-before-drop".to_string());
        }
    });
    let first_events = collect_events(first_response, 1).await;
    assert_eq!(
        first_events,
        vec![r#"{"bountyId":"bounty-before-drop"}"#.to_string()],
        "the live connection must receive the event broadcast while it was open"
    );
    // `first_response` (and the connection it owns) is dropped here,
    // simulating a client disconnecting mid-stream.
    drop(first_events);

    // ── Reconnect ────────────────────────────────────────────────────────
    let second_response = client.get(&url).send().await.expect("reconnect failed");
    tokio::spawn({
        let broadcast = state.bounty_broadcast.clone();
        async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            let _ = broadcast.send("bounty-after-reconnect".to_string());
        }
    });
    let second_events = collect_events(second_response, 1).await;

    assert_eq!(
        second_events,
        vec![r#"{"bountyId":"bounty-after-reconnect"}"#.to_string()],
        "the reconnected client must get a working fresh subscription"
    );
    assert!(
        !second_events.contains(&r#"{"bountyId":"bounty-before-drop"}"#.to_string()),
        "reconnecting must never replay an event already delivered on the dropped connection"
    );
}
