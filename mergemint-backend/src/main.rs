// mergemint-backend/src/main.rs
//
// Application entry-point: builds the Axum router with middleware and starts
// the HTTP server.
//
// ## Request body size limits and timeout middleware (#476)
//
// Two middleware layers are added to the router to protect the service from
// slow clients and excessively large payloads:
//
//   * `RequestBodyLimitLayer` — rejects bodies larger than `MAX_BODY_BYTES`.
//     Without this, a malicious client could stream an arbitrarily large body
//     and exhaust server memory before any handler logic runs.
//
//   * `TimeoutLayer` — cancels any request (including body reads and handler
//     execution) that takes longer than `REQUEST_TIMEOUT`.  This prevents slow
//     clients or downstream Horizon calls from holding connections indefinitely
//     and starving the thread pool.
//
// ## Request correlation IDs (#486)
//
// Every inbound request is stamped with a UUID v4 correlation ID by
// `SetRequestIdLayer`.  The ID is read from the `x-request-id` header when
// present (so callers can propagate their own trace ID), or generated fresh
// when absent.  `TraceLayer` then opens a tracing span for each request that
// includes the correlation ID, making it trivial to grep logs for a single
// user's flow even when requests are interleaved.
//
// ## Prometheus metrics (#867)
//
// A `metrics` middleware records a per-route request counter and a latency
// histogram, both labelled by `route` and `status`.  The indexer exposes a
// `mergemint_indexer_lag_ledgers` gauge for how far behind the latest ledger
// it is.  Everything is rendered in the Prometheus text exposition format at
// `GET /metrics`.
//
// ## Structured JSON logging (#868)
//
// The subscriber is configured to emit either the human-friendly pretty
// format (the default, for local development) or structured JSON when the
// `LOG_FORMAT=json` environment variable is set.  JSON output is what log
// aggregators expect: each record is a single object whose span fields —
// including the `request_id` correlation ID and the matched `route` — appear
// as top-level structured keys rather than being interpolated into free text.
//
// ## Graceful shutdown
//
// `axum::serve` is wired to `shutdown_signal`, which waits for SIGINT
// (Ctrl+C) or, on Unix, SIGTERM. Once either fires, Axum stops accepting new
// connections but lets in-flight requests finish — including a `self_claim`
// / `resolve_dispute` call that has already reached the point of submitting
// a chain transaction — instead of dropping them mid-flight when a deploy
// sends SIGTERM.
//
// ## Webhook SSRF guard (#1116)
//
// Webhook target URLs are refused at registration when they resolve to a
// private, loopback, link-local or metadata address. Deliveries go through a
// client whose DNS resolver checks every address again before connecting, so
// DNS rebinding cannot get around the registration check. Set
// `WEBHOOK_ALLOW_PRIVATE_TARGETS=true` to allow local receivers in
// development. See `webhooks::ssrf`.
//
// ## SSE connection caps (#1118)
//
// `GET /bounties/stream` allows `SSE_MAX_CONNECTIONS_PER_IP` (default 32)
// connections per client IP and `SSE_MAX_CONNECTIONS` (default 1024) in
// total, and returns 429 over either cap. The server is started with
// connect-info so the handler can see the peer address. The open-connection
// count is exported as the `mergemint_sse_active_connections` gauge. See
// `sse_limit`.
//
// ## Health checks
//
// `GET /health` returns `200 ok` once the server is listening. The runtime
// container image is distroless (no shell or curl), so the binary doubles as
// its own probe: `mergemint-backend healthcheck` requests `/health` on the
// local listener and exits non-zero if it doesn't get a 200.
//
// ## Readiness probe (#870)
//
// `GET /ready` pings the database and returns `200` when the connection is
// usable, or `503 Service Unavailable` when it is not.  Orchestrators such as
// Kubernetes or Docker Compose can use this to avoid routing traffic to an
// instance whose database connection is broken.  `/health` stays cheap and
// database-independent so it remains a pure liveness check.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::Arc;
use std::time::Duration;

use axum::{
    body::Body,
    extract::State,
    http::{
        header::{CONTENT_TYPE, ETAG, IF_NONE_MATCH},
        HeaderName, HeaderValue, Method, Request, StatusCode,
    },
    middleware,
    middleware::Next,
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use tower_http::{
    cors::{AllowOrigin, CorsLayer},
    limit::RequestBodyLimitLayer,
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    trace::TraceLayer,
};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use utoipa::OpenApi;

mod contract_version;
mod db;
mod deadline_reminders;
mod events;
mod indexer_runtime;
mod metrics;
mod notifications;
mod rate_limit;
mod routes;
mod sse_limit;
#[cfg(test)]
mod test_helpers;
mod webhooks;

use db::{new_shared_db, new_shared_idempotency_store};
use deadline_reminders::{
    read_deadline_reminder_interval, read_deadline_reminder_lead_time, start_deadline_reminder_job,
};
use metrics::{metrics_router, track_metrics, Metrics};
use notifications::{
    create_email_subscription, start_email_notifier, unsubscribe_email, verify_email,
};
use routes::bounties::{
    bounty_stream, claim_bounty, get_bounty_route, list_bounties, list_bounties_by_assignee,
};
use routes::leaderboard::get_leaderboard_route;
use routes::reputation::get_reputation_history;
use routes::tx::{
    new_shared_rate_limiter, query_audit_logs, resolve_dispute, self_claim, AppState,
};
use webhooks::{
    create_subscription_route, delete_subscription_route, get_subscription_route,
    list_subscriptions_route, start_webhook_dispatcher, RetryConfig, SsrfPolicy, WebhookClient,
    WebhookTargetGuard,
};

/// Maximum allowed request body size (1 MiB).
const MAX_BODY_BYTES: usize = 1024 * 1024;

/// Environment variable controlling the maximum duration of a non-streaming request.
const REQUEST_TIMEOUT_SECS_ENV: &str = "REQUEST_TIMEOUT_SECS";
const REQUEST_TIMEOUT_SECS_DEFAULT: u64 = 30;

/// The canonical header name used to carry the correlation ID across service
/// boundaries.  Clients may supply their own value; if absent a UUID v4 is
/// generated automatically by `SetRequestIdLayer`.
const REQUEST_ID_HEADER: &str = "x-request-id";

/// Address the HTTP server listens on.
const LISTEN_ADDR: &str = "0.0.0.0:8080";

/// Address the `healthcheck` subcommand probes.
const HEALTHCHECK_ADDR: &str = "127.0.0.1:8080";

/// Reward-token allowlist env var consumed by create-bounty flows.
const ALLOWLISTED_REWARD_TOKENS_ENV: &str = "ALLOWLISTED_REWARD_TOKENS";

/// Env var holding a comma-separated allow-list of origins permitted to make
/// cross-origin requests, e.g.
/// "https://app.mergemint.xyz,https://staging.mergemint.xyz".
const CORS_ALLOWED_ORIGINS_ENV: &str = "CORS_ALLOWED_ORIGINS";

/// Env var selecting the log output format; `json` enables structured logs.
const LOG_FORMAT_ENV: &str = "LOG_FORMAT";
const CONTRACT_VERSION_WARN_ONLY_ENV: &str = "CONTRACT_VERSION_WARN_ONLY";
static CONTRACT_VERSION: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// OpenAPI description of the bounty read API (#865), served at
/// `/api-docs/openapi.json` and rendered by the Swagger UI page (#866).
#[derive(OpenApi)]
#[openapi(
    info(title = "MergeMint backend API"),
    paths(
        routes::bounties::list_bounties,
        routes::bounties::list_bounties_by_assignee,
        routes::bounties::get_bounty_route,
        routes::bounties::claim_bounty,
        routes::bounties::bounty_stream,
        routes::leaderboard::get_leaderboard_route,
        routes::reputation::get_reputation_history,
        routes::stats::get_stats_route,
        routes::tx::query_audit_logs,
        notifications::create_email_subscription,
        notifications::verify_email,
        notifications::unsubscribe_email,
        webhooks::list_subscriptions_route,
        webhooks::get_subscription_route,
    ),
    components(schemas(
        db::Bounty,
        db::BountyPage,
        db::BountySortField,
        db::SortOrder,
        routes::bounties::ClaimResponse,
        routes::tx::AppError,
        routes::tx::QueryAuditLogsResponse,
        routes::reputation::ReputationHistoryPage,
        db::AuditLog,
        db::LeaderboardEntry,
        db::LeaderboardPage,
        db::ReputationHistoryEntry,
        db::StatsResponse,
        db::TokenTotal,
        db::WebhookSubscription,
        notifications::CreateEmailSubscriptionRequest,
    )),
    tags(
        (name = "bounties", description = "Bounty listing and live updates"),
        (name = "notifications", description = "Email notification subscriptions")
    )
)]
struct ApiDoc;

/// Swagger UI version loaded from the CDN by `swagger_ui`.
const SWAGGER_UI_VERSION: &str = "5.17.14";

/// SSE keep-alive interval (seconds) to prevent idle connections from being
/// closed by proxies and load balancers. Defaults to 15 seconds.
const SSE_KEEP_ALIVE_SECS_ENV: &str = "SSE_KEEP_ALIVE_SECS";
const SSE_KEEP_ALIVE_SECS_DEFAULT: u64 = 15;

#[tokio::main]
async fn main() {
    let _ = dotenvy::dotenv();
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("backfill") {
        if let Err(error) = run_backfill_command(&args[2..]).await {
            eprintln!("backfill failed: {error:#}");
            std::process::exit(1);
        }
        return;
    }
    if args.get(1).map(String::as_str) == Some("healthcheck") {
        std::process::exit(match healthcheck(HEALTHCHECK_ADDR) {
            Ok(()) => 0,
            Err(err) => {
                eprintln!("healthcheck failed: {err}");
                1
            }
        });
    }

    // ---------------------------------------------------------------------------
    // Initialise structured logging (#486, #868)
    //
    // We use a layered subscriber so that:
    //  - RUST_LOG (or the compiled-in default "info") controls the verbosity.
    //  - The output format is selected by LOG_FORMAT: `json` emits one JSON
    //    object per record (for log aggregators), anything else keeps the
    //    pretty format for local development.
    //  - Span fields injected by TraceLayer — the `request_id` correlation ID
    //    and the matched `route` — are recorded as structured keys, so they
    //    surface as top-level fields in JSON output.
    // ---------------------------------------------------------------------------
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        // Default: info-level for our crate, warn for noisy deps.
        "mergemint_backend=info,tower_http=debug,axum::rejection=trace"
            .parse()
            .unwrap()
    });

    let registry = tracing_subscriber::registry().with(env_filter);

    if log_format_is_json() {
        registry.with(fmt::layer().json()).init();
    } else {
        registry.with(fmt::layer()).init();
    }

    warn_if_reward_token_allowlist_empty();

    let postgres = match connect_postgres().await {
        Ok(pool) => pool,
        Err(error) => {
            tracing::error!(%error, "failed to initialize PostgreSQL");
            std::process::exit(1);
        }
    };
    if let Some(pool) = postgres.as_ref() {
        let rpc_url = std::env::var("RPC_URL")
            .ok()
            .filter(|value| !value.trim().is_empty());
        let contract_id = std::env::var("CONTRACT_ID")
            .ok()
            .filter(|value| !value.trim().is_empty());
        if let (Some(rpc_url), Some(contract_id)) = (rpc_url, contract_id) {
            let version_result = std::env::var("CONTRACT_VERSION_SOURCE_ACCOUNT")
                .map_err(anyhow::Error::from)
                .and_then(|source_account| {
                    Ok((rpc_url.clone(), contract_id.clone(), source_account))
                });
            match version_result {
                Ok((rpc_url, contract_id, source_account)) => {
                    match contract_version::check_contract_version(
                        &rpc_url,
                        &contract_id,
                        &source_account,
                    )
                    .await
                    {
                        Ok(version) if contract_version::is_supported_version(&version) => {
                            let _ = CONTRACT_VERSION.set(version.clone());
                            tracing::info!(%version, "contract version validated");
                        }
                        Ok(version) => {
                            if contract_version_warn_only() {
                                tracing::warn!(%version, "contract version is outside supported range 1.x.y");
                                let _ = CONTRACT_VERSION.set(version);
                            } else {
                                tracing::error!(%version, "contract version is outside supported range 1.x.y");
                                std::process::exit(1);
                            }
                        }
                        Err(error) => {
                            if contract_version_warn_only() {
                                tracing::warn!(%error, "contract version check failed; continuing in warn-only mode");
                            } else {
                                tracing::error!(%error, "contract version check failed");
                                std::process::exit(1);
                            }
                        }
                    }
                }
                Err(error) if contract_version_warn_only() => {
                    tracing::warn!(%error, "contract version source account is missing");
                }
                Err(error) => {
                    tracing::error!(%error, "contract version source account is missing");
                    std::process::exit(1);
                }
            }
            let indexer_pool = pool.clone();
            tokio::spawn(indexer_runtime::run_live(
                indexer_pool,
                rpc_url,
                contract_id,
            ));
        } else if contract_version_warn_only() {
            tracing::warn!("RPC_URL or CONTRACT_ID is missing; contract version check and live indexer are disabled");
        } else {
            tracing::error!("RPC_URL and CONTRACT_ID are required for contract version validation");
            std::process::exit(1);
        }
    } else if !contract_version_warn_only() {
        tracing::error!(
            "DATABASE_URL is required to run the persistent indexer and enforce contract version"
        );
        std::process::exit(1);
    }

    let shared_db = new_shared_db();
    if let Some(pool) = postgres {
        db::set_postgres_pool(&shared_db, pool);
    }
    let idempotency = new_shared_idempotency_store();
    let (bounty_broadcast, _) = tokio::sync::broadcast::channel(100);
    let sse_keep_alive_duration = read_sse_keep_alive_duration();

    let ssrf_policy = SsrfPolicy::from_env();
    if ssrf_policy.allow_private_targets {
        tracing::warn!(
            env = webhooks::ssrf::ALLOW_PRIVATE_TARGETS_ENV,
            "webhook SSRF protection is OFF: private and loopback targets are allowed. \
             Use this for local development only"
        );
    }
    let webhook_guard = WebhookTargetGuard::new(ssrf_policy);
    let sse_limiter = sse_limit::SseConnectionLimiter::from_env();

    let state = Arc::new(AppState {
        db: shared_db,
        idempotency,
        rate_limiter: new_shared_rate_limiter(),
        bounty_broadcast,
        leaderboard_cache: routes::leaderboard::new_leaderboard_cache(),
        sse_keep_alive_duration,
        webhook_guard: webhook_guard.clone(),
        sse_limiter: sse_limiter.clone(),
    });

    // No fallback to a default client here: that client would skip the SSRF
    // guard.
    let webhook_client = WebhookClient::new(webhook_guard, Duration::from_secs(10))
        .expect("failed to build webhook HTTP client");
    let _webhook_dispatcher =
        start_webhook_dispatcher(state.clone(), webhook_client, RetryConfig::default());
    let _email_notifier = start_email_notifier(state.clone());
    let _deadline_reminder_job = start_deadline_reminder_job(
        state.clone(),
        read_deadline_reminder_lead_time(),
        read_deadline_reminder_interval(),
    );

    let metrics = Arc::new(Metrics::new().with_sse_limiter(sse_limiter));

    let app = Router::new()
        .route("/health", get(health))
        .route("/api-docs/openapi.json", get(openapi_json))
        .route("/swagger-ui", get(swagger_ui))
        .route("/ready", get(ready))
        .route("/tx/resolve-dispute", post(resolve_dispute))
        .route("/tx/self-claim", post(self_claim))
        .route("/admin/audit-logs", get(query_audit_logs))
        .route("/bounties", get(list_bounties))
        .route("/bounties/:id", get(get_bounty_route))
        .route(
            "/bounties/assignee/:address",
            get(list_bounties_by_assignee),
        )
        .route("/leaderboard", get(get_leaderboard_route))
        .route(
            "/contributors/:address/reputation-history",
            get(get_reputation_history),
        )
        .route("/stats", get(routes::stats::get_stats_route))
        // ── Bounty push channel (#482) ─────────────────────────────────────
        .route("/bounties/:id/claim", post(claim_bounty))
        .route("/bounties/stream", get(bounty_stream))
        .route(
            "/webhooks/subscriptions",
            post(create_subscription_route).get(list_subscriptions_route),
        )
        .route(
            "/webhooks/subscriptions/:id",
            get(get_subscription_route).delete(delete_subscription_route),
        )
        .route(
            "/notifications/subscriptions",
            post(create_email_subscription),
        )
        .route("/notifications/verify", get(verify_email))
        .route("/notifications/unsubscribe", get(unsubscribe_email))
        .with_state(state)
        .merge(metrics_router(metrics.clone()))
        // Timeout wraps the handler stack, while metrics remains outermost so
        // generated 504 responses are observed like handler responses.
        .layer(middleware::from_fn_with_state(
            RequestTimeout(read_request_timeout_duration()),
            apply_request_timeout,
        ))
        // ── Prometheus metrics middleware (#867) ───────────────────────────
        //
        // Records a per-route request counter and latency histogram, labelled
        // by the matched route and response status.
        .layer(middleware::from_fn_with_state(metrics, track_metrics))
        // ── Request correlation IDs (#486) ─────────────────────────────────
        .layer(PropagateRequestIdLayer::new(HeaderName::from_static(
            REQUEST_ID_HEADER,
        )))
        .layer(SetRequestIdLayer::new(
            HeaderName::from_static(REQUEST_ID_HEADER),
            MakeRequestUuid,
        ))
        // ── Tracing ────────────────────────────────────────────────────────
        .layer(TraceLayer::new_for_http())
        // ── CORS ───────────────────────────────────────────────────────────
        .layer(cors_layer())
        // ── Body size limit (#476) ─────────────────────────────────────────
        .layer(RequestBodyLimitLayer::new(MAX_BODY_BYTES));

    let listener = tokio::net::TcpListener::bind(LISTEN_ADDR)
        .await
        .expect("failed to bind listener");

    tracing::info!(addr = LISTEN_ADDR, "mergemint-backend listening");

    // Connect-info gives handlers the peer address, which the SSE per-IP cap
    // needs.
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .expect("server error");
}

#[derive(Clone, Copy)]
struct RequestTimeout(Duration);

async fn apply_request_timeout(
    State(timeout): State<RequestTimeout>,
    request: Request<Body>,
    next: Next,
) -> Response {
    if request.uri().path() == "/bounties/stream" {
        return next.run(request).await;
    }

    match tokio::time::timeout(timeout.0, next.run(request)).await {
        Ok(response) => response,
        Err(_) => {
            let (status, body) = routes::tx::AppError::gateway_timeout();
            (status, body).into_response()
        }
    }
}

fn read_request_timeout_duration() -> Duration {
    match std::env::var(REQUEST_TIMEOUT_SECS_ENV) {
        Ok(value) => match value.parse::<u64>().ok().filter(|secs| *secs > 0) {
            Some(secs) => Duration::from_secs(secs),
            None => {
                tracing::warn!(
                    value,
                    default_secs = REQUEST_TIMEOUT_SECS_DEFAULT,
                    "REQUEST_TIMEOUT_SECS must be a positive integer; using default"
                );
                Duration::from_secs(REQUEST_TIMEOUT_SECS_DEFAULT)
            }
        },
        Err(_) => Duration::from_secs(REQUEST_TIMEOUT_SECS_DEFAULT),
    }
}

/// Read SSE keep-alive interval from environment, defaulting to 15 seconds.
fn read_sse_keep_alive_duration() -> Duration {
    match std::env::var(SSE_KEEP_ALIVE_SECS_ENV) {
        Ok(val) => match val.parse::<u64>() {
            Ok(secs) => {
                tracing::info!(
                    secs = secs,
                    "SSE keep-alive interval configured from environment"
                );
                Duration::from_secs(secs)
            }
            Err(_) => {
                tracing::warn!(
                    value = val,
                    default_secs = SSE_KEEP_ALIVE_SECS_DEFAULT,
                    "SSE_KEEP_ALIVE_SECS is not a valid integer, using default"
                );
                Duration::from_secs(SSE_KEEP_ALIVE_SECS_DEFAULT)
            }
        },
        Err(_) => {
            tracing::debug!(
                default_secs = SSE_KEEP_ALIVE_SECS_DEFAULT,
                "SSE_KEEP_ALIVE_SECS not set, using default"
            );
            Duration::from_secs(SSE_KEEP_ALIVE_SECS_DEFAULT)
        }
    }
}

/// `GET /api-docs/openapi.json` — the generated OpenAPI document.
async fn openapi_json() -> Json<utoipa::openapi::OpenApi> {
    Json(ApiDoc::openapi())
}

/// `GET /swagger-ui` — interactive API docs rendering `/api-docs/openapi.json`.
async fn swagger_ui() -> Html<String> {
    Html(format!(
        r##"<!doctype html>
<html>
<head>
  <meta charset="utf-8">
  <title>MergeMint API</title>
  <link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/swagger-ui-dist@{v}/swagger-ui.css">
</head>
<body>
  <div id="swagger-ui"></div>
  <script src="https://cdn.jsdelivr.net/npm/swagger-ui-dist@{v}/swagger-ui-bundle.js"></script>
  <script>SwaggerUIBundle({{ url: "/api-docs/openapi.json", dom_id: "#swagger-ui" }});</script>
</body>
</html>"##,
        v = SWAGGER_UI_VERSION
    ))
}

/// Liveness probe used by container healthchecks.
///
/// Keeps checks lightweight and cheap; does not touch the database (#870).
async fn health() -> &'static str {
    "ok"
}

/// Readiness probe used by orchestrators (Kubernetes / Docker Compose) (#870).
///
/// Pings the database connection and returns:
///  * `200 ready` if the database is open and reachable
///  * `503 database unavailable` if the database is closed or unreachable
async fn ready(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let mut ready = db::ping_db(&state.db).is_ok();
    if let Some(pool) = db::read_db(&state.db).postgres.clone() {
        ready &= sqlx::query("SELECT 1").execute(&pool).await.is_ok();
    }
    let version = CONTRACT_VERSION
        .get()
        .map(String::as_str)
        .unwrap_or("unknown");
    let status = if ready {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    let text = if ready {
        "ready"
    } else {
        "database unavailable"
    };
    (
        status,
        Json(json!({ "status": text, "contractVersion": version })),
    )
}

async fn connect_postgres() -> anyhow::Result<Option<sqlx::PgPool>> {
    let database_url = match std::env::var("DATABASE_URL") {
        Ok(value) if !value.trim().is_empty() => value,
        _ => return Ok(None),
    };
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await?;
    for migration in [
        include_str!("../migrations/0005_create_reputation_history.sql"),
        include_str!("../migrations/0006_create_indexer_state.sql"),
    ] {
        for statement in migration
            .split(';')
            .map(str::trim)
            .filter(|sql| !sql.is_empty())
        {
            sqlx::query(statement).execute(&pool).await?;
        }
    }
    let gap_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM indexer_gaps")
        .fetch_one(&pool)
        .await?;
    metrics::initialize_indexer_ledger_gaps(u64::try_from(gap_count).unwrap_or_default());
    Ok(Some(pool))
}

async fn run_backfill_command(args: &[String]) -> anyhow::Result<()> {
    let mut from_ledger = None;
    let mut to_ledger = None;
    let mut index = 0;
    while index < args.len() {
        let flag = &args[index];
        let value = args
            .get(index + 1)
            .ok_or_else(|| anyhow::anyhow!("missing value for {flag}"))?;
        match flag.as_str() {
            "--from-ledger" => from_ledger = Some(value.parse::<u32>()?),
            "--to-ledger" => to_ledger = Some(value.parse::<u32>()?),
            _ => return Err(anyhow::anyhow!("unknown backfill option: {flag}")),
        }
        index += 2;
    }
    let from_ledger = from_ledger.ok_or_else(|| anyhow::anyhow!("--from-ledger is required"))?;
    let rpc_url = std::env::var("RPC_URL")?;
    let contract_id = std::env::var("CONTRACT_ID")?;
    let pool = connect_postgres()
        .await?
        .ok_or_else(|| anyhow::anyhow!("DATABASE_URL is required for backfill"))?;
    indexer_runtime::backfill(&pool, &rpc_url, &contract_id, from_ledger, to_ledger).await
}

fn contract_version_warn_only() -> bool {
    std::env::var(CONTRACT_VERSION_WARN_ONLY_ENV)
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "true" | "1" | "yes"
            )
        })
        .unwrap_or(false)
}

/// Build the CORS layer from `CORS_ALLOWED_ORIGINS`.
fn cors_layer() -> CorsLayer {
    let origins = std::env::var(CORS_ALLOWED_ORIGINS_ENV).unwrap_or_default();
    let allowed: Vec<HeaderValue> = origins
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse::<HeaderValue>().ok())
        .collect();

    CorsLayer::new()
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([CONTENT_TYPE, IF_NONE_MATCH])
        .expose_headers([ETAG])
        .allow_origin(AllowOrigin::list(allowed))
}

/// Returns `true` when `LOG_FORMAT=json` (case insensitive).
fn log_format_is_json() -> bool {
    std::env::var(LOG_FORMAT_ENV)
        .map(|v| v.eq_ignore_ascii_case("json"))
        .unwrap_or(false)
}

/// Warn once at startup when the reward-token allowlist is empty.
fn warn_if_reward_token_allowlist_empty() {
    let empty = std::env::var(ALLOWLISTED_REWARD_TOKENS_ENV)
        .map(|v| v.trim().is_empty())
        .unwrap_or(true);
    if empty {
        tracing::warn!(
            env = ALLOWLISTED_REWARD_TOKENS_ENV,
            "reward-token allowlist is empty; create-bounty flows will reject all tokens"
        );
    }
}

/// Wait for SIGINT (Ctrl+C) or, on Unix, SIGTERM.
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {}
        _ = terminate => {}
    }

    tracing::info!("shutdown signal received; draining in-flight requests");
}

/// Probe `/health` on the local listener; used by the `healthcheck` subcommand.
fn healthcheck(addr: &str) -> std::io::Result<()> {
    let mut stream = TcpStream::connect(addr)?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")?;

    let mut response = String::new();
    stream.read_to_string(&mut response)?;

    if response.starts_with("HTTP/1.1 200") || response.starts_with("HTTP/1.0 200") {
        Ok(())
    } else {
        Err(std::io::Error::other(format!(
            "unexpected health response: {}",
            response.lines().next().unwrap_or("")
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use tower::ServiceExt;

    #[tokio::test]
    async fn ready_returns_200_when_db_is_healthy() {
        let shared_db = crate::db::new_shared_db();
        let idempotency = crate::db::new_shared_idempotency_store();
        let (bounty_broadcast, _) = tokio::sync::broadcast::channel(10);
        let state = std::sync::Arc::new(super::AppState {
            db: shared_db,
            idempotency,
            rate_limiter: crate::routes::tx::new_shared_rate_limiter(),
            bounty_broadcast,
            leaderboard_cache: crate::routes::leaderboard::new_leaderboard_cache(),
            sse_keep_alive_duration: std::time::Duration::from_secs(15),
            webhook_guard: Default::default(),
            sse_limiter: Default::default(),
        });

        let app = axum::Router::new()
            .route("/ready", axum::routing::get(super::ready))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/ready")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(payload["status"], "ready");
        assert!(payload["contractVersion"].is_string());
    }

    #[tokio::test]
    async fn ready_returns_503_when_db_is_closed() {
        let shared_db = crate::db::new_shared_db();
        crate::db::close_db(&shared_db);

        let idempotency = crate::db::new_shared_idempotency_store();
        let (bounty_broadcast, _) = tokio::sync::broadcast::channel(10);
        let state = std::sync::Arc::new(super::AppState {
            db: shared_db,
            idempotency,
            rate_limiter: crate::routes::tx::new_shared_rate_limiter(),
            bounty_broadcast,
            leaderboard_cache: crate::routes::leaderboard::new_leaderboard_cache(),
            sse_keep_alive_duration: std::time::Duration::from_secs(15),
            webhook_guard: Default::default(),
            sse_limiter: Default::default(),
        });

        let app = axum::Router::new()
            .route("/ready", axum::routing::get(super::ready))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/ready")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(
            response.status(),
            axum::http::StatusCode::SERVICE_UNAVAILABLE
        );
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(payload["status"], "database unavailable");
        assert!(payload["contractVersion"].is_string());
    }

    #[tokio::test]
    async fn openapi_document_lists_bounty_routes() {
        let Json(doc) = openapi_json().await;
        for path in [
            "/bounties",
            "/bounties/{id}",
            "/bounties/assignee/{address}",
            "/admin/audit-logs",
            "/leaderboard",
            "/stats",
            "/webhooks/subscriptions",
            "/webhooks/subscriptions/{id}",
        ] {
            assert!(doc.paths.paths.contains_key(path), "missing {path}");
        }
        let schemas = doc.components.expect("components").schemas;
        assert!(schemas.contains_key("Bounty"));
        assert!(schemas.contains_key("AppError"));
    }

    #[test]
    fn openapi_document_matches_snapshot() {
        let document = serde_json::to_value(ApiDoc::openapi()).expect("OpenAPI serializes");
        let mut paths = serde_json::Map::new();
        for (path, item) in document["paths"].as_object().expect("paths object") {
            let mut methods: Vec<_> = item
                .as_object()
                .expect("path item object")
                .keys()
                .cloned()
                .collect();
            methods.sort();
            paths.insert(path.clone(), serde_json::json!(methods));
        }

        let mut schemas = serde_json::Map::new();
        for (name, schema) in document["components"]["schemas"]
            .as_object()
            .expect("schemas object")
        {
            let mut fields: Vec<_> = schema["properties"]
                .as_object()
                .map(|properties| properties.keys().cloned().collect())
                .unwrap_or_default();
            fields.sort();
            schemas.insert(
                name.clone(),
                serde_json::json!({
                    "fields": fields,
                    "enum": schema.get("enum").cloned().unwrap_or(serde_json::Value::Null)
                }),
            );
        }
        let actual = serde_json::json!({ "paths": paths, "schemas": schemas });
        let expected: serde_json::Value =
            serde_json::from_str(include_str!("../openapi.snapshot.json"))
                .expect("OpenAPI snapshot must be valid JSON");
        assert_eq!(
            actual, expected,
            "OpenAPI contract changed; review and update openapi.snapshot.json"
        );
    }

    #[tokio::test]
    async fn json_get_responses_match_documented_schemas() {
        use crate::db::{acquire_db, add_webhook_subscription, Bounty};
        use crate::test_helpers::validate_openapi_response;
        use chrono::Utc;

        let db = crate::db::new_shared_db();
        let address = format!("G{}", "A".repeat(55));
        acquire_db(&db).bounties.push(Bounty {
            id: "bounty-1".to_string(),
            creator: "creator".to_string(),
            assignee: Some(address.clone()),
            created_at: Utc::now(),
            ..Default::default()
        });
        add_webhook_subscription(
            &db,
            "sub-1".to_string(),
            "https://example.com/hook".to_string(),
            "secret".to_string(),
            vec!["*".to_string()],
        );
        let state = Arc::new(AppState {
            db,
            idempotency: new_shared_idempotency_store(),
            rate_limiter: new_shared_rate_limiter(),
            bounty_broadcast: tokio::sync::broadcast::channel(16).0,
            leaderboard_cache: routes::leaderboard::new_leaderboard_cache(),
            sse_keep_alive_duration: Duration::from_secs(15),
            webhook_guard: Default::default(),
            sse_limiter: Default::default(),
        });
        let app = Router::new()
            .route("/admin/audit-logs", get(query_audit_logs))
            .route("/bounties", get(list_bounties))
            .route("/bounties/:id", get(get_bounty_route))
            .route(
                "/bounties/assignee/:address",
                get(list_bounties_by_assignee),
            )
            .route("/leaderboard", get(get_leaderboard_route))
            .route("/stats", get(routes::stats::get_stats_route))
            .route("/webhooks/subscriptions", get(list_subscriptions_route))
            .route("/webhooks/subscriptions/:id", get(get_subscription_route))
            .with_state(state);
        let openapi = ApiDoc::openapi();
        let assignee_uri = format!("/bounties/assignee/{address}");

        for (uri, schema_path) in [
            ("/admin/audit-logs", "/admin/audit-logs"),
            ("/bounties", "/bounties"),
            ("/bounties/bounty-1", "/bounties/{id}"),
            (assignee_uri.as_str(), "/bounties/assignee/{address}"),
            ("/leaderboard", "/leaderboard"),
            ("/stats", "/stats"),
            ("/webhooks/subscriptions", "/webhooks/subscriptions"),
            (
                "/webhooks/subscriptions/sub-1",
                "/webhooks/subscriptions/{id}",
            ),
        ] {
            let response = app
                .clone()
                .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK, "GET {uri}");
            let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap();
            let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            validate_openapi_response(&openapi, schema_path, 200, &body)
                .unwrap_or_else(|error| panic!("GET {uri} violates OpenAPI: {error}"));
        }
    }

    #[tokio::test]
    async fn timed_out_requests_return_typed_504_and_are_counted() {
        let metrics = Arc::new(Metrics::new());
        let app = Router::new()
            .route(
                "/slow",
                get(|| async {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                    "finished"
                }),
            )
            .layer(middleware::from_fn_with_state(
                RequestTimeout(Duration::from_millis(1)),
                apply_request_timeout,
            ))
            .layer(middleware::from_fn_with_state(
                metrics.clone(),
                track_metrics,
            ));

        let response = app
            .oneshot(Request::builder().uri("/slow").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&body).unwrap(),
            serde_json::json!({ "code": 504, "message": "request timed out" })
        );
        assert!(metrics.render().contains("status=\"504\""));
    }

    #[tokio::test]
    async fn bounty_stream_path_is_excluded_from_request_timeout() {
        let app = Router::new()
            .route(
                "/bounties/stream",
                get(|| async {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                    "stream established"
                }),
            )
            .layer(middleware::from_fn_with_state(
                RequestTimeout(Duration::from_millis(1)),
                apply_request_timeout,
            ));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/bounties/stream")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn health_stays_cheap_and_independent_of_db() {
        assert_eq!(health().await, "ok");
    }
}
