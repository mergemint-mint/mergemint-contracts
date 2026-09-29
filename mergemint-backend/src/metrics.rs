//! Prometheus metrics for the mergemint backend.
//!
//! Exposes per-route request counters and latency histograms (labelled by
//! route and status) plus a gauge tracking how far the indexer lags behind
//! the latest ledger. Everything is rendered in the Prometheus text
//! exposition format and served at `/metrics`.

use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::Arc;

use axum::body::Body;
use axum::extract::State;
use axum::http::{Request, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use std::time::Instant;

/// Latency histogram buckets in seconds.
const LATENCY_BUCKETS: [f64; 11] = [
    0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
];

/// A single (route, status) observation series.
#[derive(Default)]
struct RouteMetrics {
    count: AtomicU64,
    /// Cumulative bucket counts, one per `LATENCY_BUCKETS` entry.
    buckets: [AtomicU64; LATENCY_BUCKETS.len()],
    sum_micros: AtomicU64,
}

impl RouteMetrics {
    fn observe(&self, elapsed_secs: f64) {
        self.count.fetch_add(1, Ordering::Relaxed);
        for (i, bound) in LATENCY_BUCKETS.iter().enumerate() {
            if elapsed_secs <= *bound {
                self.buckets[i].fetch_add(1, Ordering::Relaxed);
            }
        }
        self.sum_micros
            .fetch_add((elapsed_secs * 1_000_000.0) as u64, Ordering::Relaxed);
    }
}

/// Shared metrics registry.
#[derive(Default)]
pub struct Metrics {
    routes: std::sync::Mutex<std::collections::HashMap<(String, u16), Arc<RouteMetrics>>>,
    indexer_lag: AtomicI64,
}

impl Metrics {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record one request observation for the given route and status.
    pub fn observe_request(&self, route: &str, status: u16, elapsed_secs: f64) {
        let series = {
            let mut routes = self.routes.lock().expect("metrics mutex poisoned");
            routes
                .entry((route.to_string(), status))
                .or_insert_with(|| Arc::new(RouteMetrics::default()))
                .clone()
        };
        series.observe(elapsed_secs);
    }

    /// Update the indexer lag gauge (ledgers behind the chain tip).
    pub fn set_indexer_lag(&self, lag: i64) {
        self.indexer_lag.store(lag, Ordering::Relaxed);
    }

    /// Render all metrics in the Prometheus text exposition format.
    pub fn render(&self) -> String {
        let mut out = String::new();

        out.push_str("# HELP mergemint_http_requests_total Total HTTP requests by route and status.\n");
        out.push_str("# TYPE mergemint_http_requests_total counter\n");
        out.push_str("# HELP mergemint_http_request_duration_seconds HTTP request latency by route and status.\n");
        out.push_str("# TYPE mergemint_http_request_duration_seconds histogram\n");

        let routes = self.routes.lock().expect("metrics mutex poisoned");
        for ((route, status), series) in routes.iter() {
            let count = series.count.load(Ordering::Relaxed);
            out.push_str(&format!(
                "mergemint_http_requests_total{{route=\"{}\",status=\"{}\"}} {}\n",
                route, status, count
            ));

            let mut cumulative = 0u64;
            for (i, bound) in LATENCY_BUCKETS.iter().enumerate() {
                cumulative += series.buckets[i].load(Ordering::Relaxed);
                out.push_str(&format!(
                    "mergemint_http_request_duration_seconds_bucket{{route=\"{}\",status=\"{}\",le=\"{}\"}} {}\n",
                    route, status, bound, cumulative
                ));
            }
            out.push_str(&format!(
                "mergemint_http_request_duration_seconds_bucket{{route=\"{}\",status=\"{}\",le=\"+Inf\"}} {}\n",
                route, status, count
            ));
            let sum_secs = series.sum_micros.load(Ordering::Relaxed) as f64 / 1_000_000.0;
            out.push_str(&format!(
                "mergemint_http_request_duration_seconds_sum{{route=\"{}\",status=\"{}\"}} {}\n",
                route, status, sum_secs
            ));
            out.push_str(&format!(
                "mergemint_http_request_duration_seconds_count{{route=\"{}\",status=\"{}\"}} {}\n",
                route, status, count
            ));
        }
        drop(routes);

        out.push_str("# HELP mergemint_indexer_lag_ledgers Ledgers the indexer is behind the chain tip.\n");
        out.push_str("# TYPE mergemint_indexer_lag_ledgers gauge\n");
        out.push_str(&format!(
            "mergemint_indexer_lag_ledgers {}\n",
            self.indexer_lag.load(Ordering::Relaxed)
        ));

        out
    }
}

/// Axum middleware recording per-route counters and latency histograms.
///
/// The route label is taken from the matched path template (e.g.
/// `/v1/events/:id`) so cardinality stays bounded.
pub async fn track_metrics(
    State(metrics): State<Arc<Metrics>>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let route = request
        .extensions()
        .get::<axum::extract::MatchedPath>()
        .map(|m| m.as_str().to_string())
        .unwrap_or_else(|| "unmatched".to_string());

    let start = Instant::now();
    let response = next.run(request).await;
    let elapsed = start.elapsed().as_secs_f64();

    metrics.observe_request(&route, response.status().as_u16(), elapsed);
    response
}

/// Handler serving the Prometheus exposition at `/metrics`.
async fn metrics_handler(State(metrics): State<Arc<Metrics>>) -> impl IntoResponse {
    (
        StatusCode::OK,
        [("content-type", "text/plain; version=0.0.4")],
        metrics.render(),
    )
}

/// Build a router exposing the `/metrics` scrape endpoint.
pub fn metrics_router(metrics: Arc<Metrics>) -> Router {
    Router::new()
        .route("/metrics", get(metrics_handler))
        .with_state(metrics)
}
