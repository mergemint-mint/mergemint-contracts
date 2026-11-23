//! Prometheus metrics for the mergemint backend.
//!
//! Exposes per-route request counters and latency histograms (labelled by
//! route and status) plus a gauge tracking how far the indexer lags behind
//! the latest ledger, plus a gauge of open SSE stream connections and a
//! counter of SSE connections refused over a cap (#1118). Everything is
//! rendered in the Prometheus text exposition format and served at `/metrics`.

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

use crate::sse_limit::{LimitExceeded, SseConnectionLimiter};

/// Latency histogram buckets in seconds.
const LATENCY_BUCKETS: [f64; 11] = [
    0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
];

static INDEXER_LEDGER_GAPS: AtomicU64 = AtomicU64::new(0);

pub fn record_indexer_ledger_gap() {
    INDEXER_LEDGER_GAPS.fetch_add(1, Ordering::Relaxed);
}

pub fn initialize_indexer_ledger_gaps(total: u64) {
    INDEXER_LEDGER_GAPS.store(total, Ordering::Relaxed);
}

/// Indexer decode failures (malformed XDR / payloads).
static INDEXER_DECODE_ERRORS: AtomicU64 = AtomicU64::new(0);
/// Indexer events whose topic/type is not recognised.
static INDEXER_UNRECOGNIZED_EVENTS: AtomicU64 = AtomicU64::new(0);
/// Webhook deliveries by outcome (`success`, `failure`).
static WEBHOOK_DELIVERIES_SUCCESS: AtomicU64 = AtomicU64::new(0);
static WEBHOOK_DELIVERIES_FAILURE: AtomicU64 = AtomicU64::new(0);
/// Webhook deliveries blocked by the SSRF guard.
static WEBHOOK_SSRF_BLOCKED: AtomicU64 = AtomicU64::new(0);
/// SSE connections rejected by reason (`per_ip`, `global`).
static SSE_REJECTED_PER_IP: AtomicU64 = AtomicU64::new(0);
static SSE_REJECTED_GLOBAL: AtomicU64 = AtomicU64::new(0);
/// DB pool connections by state (`in_use`, `idle`).
static DB_POOL_IN_USE: AtomicU64 = AtomicU64::new(0);
static DB_POOL_IDLE: AtomicU64 = AtomicU64::new(0);

pub fn record_indexer_decode_error() {
    INDEXER_DECODE_ERRORS.fetch_add(1, Ordering::Relaxed);
}

pub fn record_indexer_unrecognized_event() {
    INDEXER_UNRECOGNIZED_EVENTS.fetch_add(1, Ordering::Relaxed);
}

pub fn record_webhook_delivery(success: bool) {
    if success {
        WEBHOOK_DELIVERIES_SUCCESS.fetch_add(1, Ordering::Relaxed);
    } else {
        WEBHOOK_DELIVERIES_FAILURE.fetch_add(1, Ordering::Relaxed);
    }
}

pub fn record_webhook_ssrf_blocked() {
    WEBHOOK_SSRF_BLOCKED.fetch_add(1, Ordering::Relaxed);
}

pub fn record_sse_rejected(reason: LimitExceeded) {
    match reason {
        LimitExceeded::PerIp => SSE_REJECTED_PER_IP.fetch_add(1, Ordering::Relaxed),
        LimitExceeded::Global => SSE_REJECTED_GLOBAL.fetch_add(1, Ordering::Relaxed),
    };
}

/// Update the DB pool saturation gauges from the current pool state.
pub fn set_db_pool_connections(in_use: u64, idle: u64) {
    DB_POOL_IN_USE.store(in_use, Ordering::Relaxed);
    DB_POOL_IDLE.store(idle, Ordering::Relaxed);
}

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
    /// Source of the SSE connection gauge. `None` reports zero.
    sse_limiter: Option<SseConnectionLimiter>,
}

impl Metrics {
    pub fn new() -> Self {
        Self::default()
    }

    /// Report open and refused SSE connections from `limiter`.
    pub fn with_sse_limiter(mut self, limiter: SseConnectionLimiter) -> Self {
        self.sse_limiter = Some(limiter);
        self
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
    #[allow(dead_code)] // Set by the indexer, which the binary does not run yet.
    pub fn set_indexer_lag(&self, lag: i64) {
        self.indexer_lag.store(lag, Ordering::Relaxed);
    }

    /// Render all metrics in the Prometheus text exposition format.
    pub fn render(&self) -> String {
        let mut out = String::new();

        out.push_str(
            "# HELP mergemint_http_requests_total Total HTTP requests by route and status.\n",
        );
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

        out.push_str(
            "# HELP mergemint_indexer_lag_ledgers Ledgers the indexer is behind the chain tip.\n",
        );
        out.push_str("# TYPE mergemint_indexer_lag_ledgers gauge\n");
        out.push_str(&format!(
            "mergemint_indexer_lag_ledgers {}\n",
            self.indexer_lag.load(Ordering::Relaxed)
        ));

        out.push_str(
            "# HELP indexer_ledger_gaps_total Number of ledger gaps detected by the indexer.\n",
        );
        out.push_str("# TYPE indexer_ledger_gaps_total counter\n");
        out.push_str(&format!(
            "indexer_ledger_gaps_total {}\n",
            INDEXER_LEDGER_GAPS.load(Ordering::Relaxed)
        ));

        out.push_str("# HELP mergemint_indexer_decode_errors_total Indexer events that failed to decode.\n");
        out.push_str("# TYPE mergemint_indexer_decode_errors_total counter\n");
        out.push_str(&format!(
            "mergemint_indexer_decode_errors_total {}\n",
            INDEXER_DECODE_ERRORS.load(Ordering::Relaxed)
        ));

        out.push_str("# HELP mergemint_indexer_unrecognized_events_total Indexer events with an unrecognised topic/type.\n");
        out.push_str("# TYPE mergemint_indexer_unrecognized_events_total counter\n");
        out.push_str(&format!(
            "mergemint_indexer_unrecognized_events_total {}\n",
            INDEXER_UNRECOGNIZED_EVENTS.load(Ordering::Relaxed)
        ));

        out.push_str("# HELP mergemint_webhook_deliveries_total Webhook deliveries by outcome.\n");
        out.push_str("# TYPE mergemint_webhook_deliveries_total counter\n");
        out.push_str(&format!(
            "mergemint_webhook_deliveries_total{{outcome=\"success\"}} {}\n",
            WEBHOOK_DELIVERIES_SUCCESS.load(Ordering::Relaxed)
        ));
        out.push_str(&format!(
            "mergemint_webhook_deliveries_total{{outcome=\"failure\"}} {}\n",
            WEBHOOK_DELIVERIES_FAILURE.load(Ordering::Relaxed)
        ));

        out.push_str("# HELP mergemint_webhook_ssrf_blocked_total Webhook deliveries blocked by the SSRF guard.\n");
        out.push_str("# TYPE mergemint_webhook_ssrf_blocked_total counter\n");
        out.push_str(&format!(
            "mergemint_webhook_ssrf_blocked_total {}\n",
            WEBHOOK_SSRF_BLOCKED.load(Ordering::Relaxed)
        ));

        out.push_str("# HELP mergemint_db_pool_connections Database pool connections by state.\n");
        out.push_str("# TYPE mergemint_db_pool_connections gauge\n");
        out.push_str(&format!(
            "mergemint_db_pool_connections{{state=\"in_use\"}} {}\n",
            DB_POOL_IN_USE.load(Ordering::Relaxed)
        ));
        out.push_str(&format!(
            "mergemint_db_pool_connections{{state=\"idle\"}} {}\n",
            DB_POOL_IDLE.load(Ordering::Relaxed)
        ));

        let (cache_hits, cache_misses) = crate::routes::leaderboard::leaderboard_cache_counts();
        out.push_str(
            "# HELP mergemint_leaderboard_cache_total Leaderboard cache requests by result.\n",
        );
        out.push_str("# TYPE mergemint_leaderboard_cache_total counter\n");
        out.push_str(&format!(
            "mergemint_leaderboard_cache_total{{result=\"hit\"}} {cache_hits}\n"
        ));
        out.push_str(&format!(
            "mergemint_leaderboard_cache_total{{result=\"miss\"}} {cache_misses}\n"
        ));

        let limiter = self.sse_limiter.as_ref();
        out.push_str(
            "# HELP mergemint_sse_active_connections Open GET /bounties/stream connections.\n",
        );
        out.push_str("# TYPE mergemint_sse_active_connections gauge\n");
        out.push_str(&format!(
            "mergemint_sse_active_connections {}\n",
            limiter.map_or(0, |l| l.active())
        ));
        out.push_str("# HELP mergemint_sse_rejected_connections_total SSE connections refused with 429, by cap.\n");
        out.push_str("# TYPE mergemint_sse_rejected_connections_total counter\n");
        for (label, reason) in [
            ("per_ip", LimitExceeded::PerIp),
            ("global", LimitExceeded::Global),
        ] {
            out.push_str(&format!(
                "mergemint_sse_rejected_connections_total{{reason=\"{}\"}} {}\n",
                label,
                limiter.map_or(0, |l| l.rejected(reason))
            ));
        }

        out.push_str("# HELP mergemint_sse_rejected_total SSE connections rejected, by reason.\n");
        out.push_str("# TYPE mergemint_sse_rejected_total counter\n");
        out.push_str(&format!(
            "mergemint_sse_rejected_total{{reason=\"per_ip\"}} {}\n",
            SSE_REJECTED_PER_IP.load(Ordering::Relaxed)
        ));
        out.push_str(&format!(
            "mergemint_sse_rejected_total{{reason=\"global\"}} {}\n",
            SSE_REJECTED_GLOBAL.load(Ordering::Relaxed)
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

/// `GET /metrics` handler rendering the registry in Prometheus text format.
async fn metrics_handler(State(metrics): State<Arc<Metrics>>) -> impl IntoResponse {
    (
        StatusCode::OK,
        [("content-type", "text/plain; version=0.0.4")],
        metrics.render(),
    )
}

/// Build a router exposing `GET /metrics` backed by `metrics`.
pub fn router(metrics: Arc<Metrics>) -> Router {
    Router::new()
        .route("/metrics", get(metrics_handler))
        .with_state(metrics)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value_for(rendered: &str, needle: &str) -> Option<f64> {
        rendered
            .lines()
            .find(|line| line.starts_with(needle))
            .and_then(|line| line.rsplit(' ').next())
            .and_then(|v| v.parse().ok())
    }

    #[test]
    fn renders_help_and_type_for_new_metrics() {
        let rendered = Metrics::new().render();
        for name in [
            "mergemint_indexer_decode_errors_total",
            "mergemint_indexer_unrecognized_events_total",
            "mergemint_webhook_deliveries_total",
            "mergemint_webhook_ssrf_blocked_total",
            "mergemint_sse_rejected_total",
            "mergemint_db_pool_connections",
        ] {
            assert!(
                rendered.contains(&format!("# HELP {name} ")),
                "missing HELP for {name}"
            );
            assert!(
                rendered.contains(&format!("# TYPE {name} ")),
                "missing TYPE for {name}"
            );
        }
    }

    #[test]
    fn counters_increment_on_error_paths() {
        let before = Metrics::new().render();
        let decode_before = value_for(&before, "mergemint_indexer_decode_errors_total").unwrap();
        let unrec_before =
            value_for(&before, "mergemint_indexer_unrecognized_events_total").unwrap();
        let fail_before = value_for(
            &before,
            "mergemint_webhook_deliveries_total{outcome=\"failure\"}",
        )
        .unwrap();
        let ssrf_before = value_for(&before, "mergemint_webhook_ssrf_blocked_total").unwrap();
        let sse_before = value_for(
            &before,
            "mergemint_sse_rejected_total{reason=\"per_ip\"}",
        )
        .unwrap();

        record_indexer_decode_error();
        record_indexer_unrecognized_event();
        record_webhook_delivery(false);
        record_webhook_ssrf_blocked();
        record_sse_rejected(LimitExceeded::PerIp);

        let after = Metrics::new().render();
        assert_eq!(
            value_for(&after, "mergemint_indexer_decode_errors_total").unwrap(),
            decode_before + 1.0
        );
        assert_eq!(
            value_for(&after, "mergemint_indexer_unrecognized_events_total").unwrap(),
            unrec_before + 1.0
        );
        assert_eq!(
            value_for(
                &after,
                "mergemint_webhook_deliveries_total{outcome=\"failure\"}"
            )
            .unwrap(),
            fail_before + 1.0
        );
        assert_eq!(
            value_for(&after, "mergemint_webhook_ssrf_blocked_total").unwrap(),
            ssrf_before + 1.0
        );
        assert_eq!(
            value_for(&after, "mergemint_sse_rejected_total{reason=\"per_ip\"}").unwrap(),
            sse_before + 1.0
        );
    }

    #[test]
    fn db_pool_gauges_reflect_state() {
        set_db_pool_connections(7, 3);
        let rendered = Metrics::new().render();
        assert_eq!(
            value_for(&rendered, "mergemint_db_pool_connections{state=\"in_use\"}").unwrap(),
            7.0
        );
        assert_eq!(
            value_for(&rendered, "mergemint_db_pool_connections{state=\"idle\"}").unwrap(),
            3.0
        );
    }
}
