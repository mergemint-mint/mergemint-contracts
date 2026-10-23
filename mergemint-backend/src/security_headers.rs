// mergemint-backend/src/security_headers.rs
//
// Security headers middleware (#1115).
//
// Adds standard security headers to every response, including JSON routes,
// the SSE stream, CORS preflights and errors produced by other layers
// (413, 504, ...). Headers a handler already set are left alone.
//
//   * `X-Content-Type-Options: nosniff` — stops browsers guessing a content
//     type, so a JSON body can never be treated as HTML or script.
//   * `Referrer-Policy: no-referrer` — API URLs can contain addresses and
//     tokens (e.g. email verification links); never leak them.
//   * `X-Frame-Options: DENY` — no backend page (e.g. `/swagger-ui`) can be
//     framed for clickjacking.
//   * `Strict-Transport-Security` — only when `HSTS_ENABLED=true`. HSTS must
//     only be sent when the service is reached over HTTPS (normally behind a
//     TLS-terminating proxy), otherwise local HTTP development breaks.
//
// None of these headers affect CORS (they are not CORS headers and do not
// change the `Access-Control-*` set) or SSE (they do not touch
// `Content-Type`, caching or the body stream).

use axum::{
    extract::{Request, State},
    http::{header, HeaderMap, HeaderValue},
    middleware::Next,
    response::Response,
};

/// Enables the `Strict-Transport-Security` header when `true`/`1`/`yes`.
pub const HSTS_ENABLED_ENV: &str = "HSTS_ENABLED";
/// HSTS `max-age` in seconds. Defaults to one year.
pub const HSTS_MAX_AGE_SECS_ENV: &str = "HSTS_MAX_AGE_SECS";
/// Adds `includeSubDomains` to the HSTS header when `true`/`1`/`yes`.
pub const HSTS_INCLUDE_SUBDOMAINS_ENV: &str = "HSTS_INCLUDE_SUBDOMAINS";

pub const HSTS_MAX_AGE_SECS_DEFAULT: u64 = 31_536_000;

/// Configuration for [`security_headers`].
#[derive(Clone, Debug, Default)]
pub struct SecurityHeadersConfig {
    /// Pre-rendered `Strict-Transport-Security` value, or `None` to omit it.
    hsts: Option<HeaderValue>,
}

impl SecurityHeadersConfig {
    /// Config that never sends HSTS (plain HTTP / local development).
    pub fn without_hsts() -> Self {
        Self { hsts: None }
    }

    /// Config that sends HSTS with the given max-age.
    pub fn with_hsts(max_age_secs: u64, include_subdomains: bool) -> Self {
        let mut value = format!("max-age={max_age_secs}");
        if include_subdomains {
            value.push_str("; includeSubDomains");
        }
        Self {
            hsts: Some(HeaderValue::from_str(&value).expect("valid HSTS header value")),
        }
    }

    /// Reads the config from `HSTS_ENABLED`, `HSTS_MAX_AGE_SECS` and
    /// `HSTS_INCLUDE_SUBDOMAINS`.
    pub fn from_env() -> Self {
        Self::from_lookup(|key| std::env::var(key).ok())
    }

    fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Self {
        if !lookup(HSTS_ENABLED_ENV).is_some_and(|v| is_truthy(&v)) {
            return Self::without_hsts();
        }
        let max_age = match lookup(HSTS_MAX_AGE_SECS_ENV) {
            None => HSTS_MAX_AGE_SECS_DEFAULT,
            Some(raw) => raw.trim().parse().unwrap_or_else(|_| {
                tracing::warn!(
                    value = %raw,
                    default = HSTS_MAX_AGE_SECS_DEFAULT,
                    "invalid HSTS_MAX_AGE_SECS, using default"
                );
                HSTS_MAX_AGE_SECS_DEFAULT
            }),
        };
        let include_subdomains = lookup(HSTS_INCLUDE_SUBDOMAINS_ENV).is_some_and(|v| is_truthy(&v));
        Self::with_hsts(max_age, include_subdomains)
    }
}

fn is_truthy(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

/// Axum middleware that applies the security headers to every response.
pub async fn security_headers(
    State(config): State<SecurityHeadersConfig>,
    request: Request,
    next: Next,
) -> Response {
    let mut response = next.run(request).await;
    apply(response.headers_mut(), &config);
    response
}

fn apply(headers: &mut HeaderMap, config: &SecurityHeadersConfig) {
    set_default(
        headers,
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    set_default(
        headers,
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    set_default(
        headers,
        header::X_FRAME_OPTIONS,
        HeaderValue::from_static("DENY"),
    );
    if let Some(hsts) = &config.hsts {
        set_default(headers, header::STRICT_TRANSPORT_SECURITY, hsts.clone());
    }
}

fn set_default(headers: &mut HeaderMap, name: header::HeaderName, value: HeaderValue) {
    headers.entry(name).or_insert(value);
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Method, Request, StatusCode},
        middleware,
        response::{
            sse::{Event, Sse},
            IntoResponse,
        },
        routing::get,
        Json, Router,
    };
    use std::collections::HashMap;
    use tower::ServiceExt;
    use tower_http::cors::{AllowOrigin, CorsLayer};

    const ORIGIN: &str = "https://app.mergemint.xyz";

    fn app(config: SecurityHeadersConfig) -> Router {
        Router::new()
            .route(
                "/json",
                get(|| async { Json(serde_json::json!({ "ok": true })) }),
            )
            .route(
                "/stream",
                get(|| async {
                    let events = futures_util::stream::once(async {
                        Ok::<_, std::convert::Infallible>(Event::default().data("hello"))
                    });
                    Sse::new(events)
                }),
            )
            .route(
                "/custom",
                get(|| async {
                    ([(header::REFERRER_POLICY, "same-origin")], "custom").into_response()
                }),
            )
            .layer(
                CorsLayer::new()
                    .allow_methods([Method::GET, Method::POST])
                    .allow_origin(AllowOrigin::list([HeaderValue::from_static(ORIGIN)])),
            )
            .layer(middleware::from_fn_with_state(config, security_headers))
    }

    fn get_req(uri: &str) -> Request<Body> {
        Request::builder()
            .uri(uri)
            .header(header::ORIGIN, ORIGIN)
            .body(Body::empty())
            .unwrap()
    }

    fn assert_base_headers(headers: &HeaderMap) {
        assert_eq!(headers[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
        assert_eq!(headers[header::REFERRER_POLICY], "no-referrer");
        assert_eq!(headers[header::X_FRAME_OPTIONS], "DENY");
    }

    #[tokio::test]
    async fn json_route_gets_security_headers_and_keeps_cors() {
        let res = app(SecurityHeadersConfig::without_hsts())
            .oneshot(get_req("/json"))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert_base_headers(res.headers());
        assert!(res
            .headers()
            .get(header::STRICT_TRANSPORT_SECURITY)
            .is_none());
        assert_eq!(res.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN], ORIGIN);
        assert_eq!(res.headers()[header::CONTENT_TYPE], "application/json");
    }

    #[tokio::test]
    async fn sse_route_gets_security_headers_and_stays_a_stream() {
        let res = app(SecurityHeadersConfig::without_hsts())
            .oneshot(get_req("/stream"))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert_base_headers(res.headers());
        assert_eq!(res.headers()[header::CONTENT_TYPE], "text/event-stream");
        let body = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(&body[..], b"data: hello\n\n");
    }

    #[tokio::test]
    async fn cors_preflight_gets_security_headers() {
        let req = Request::builder()
            .method(Method::OPTIONS)
            .uri("/json")
            .header(header::ORIGIN, ORIGIN)
            .header(header::ACCESS_CONTROL_REQUEST_METHOD, "GET")
            .body(Body::empty())
            .unwrap();
        let res = app(SecurityHeadersConfig::without_hsts())
            .oneshot(req)
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert_base_headers(res.headers());
        assert_eq!(res.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN], ORIGIN);
    }

    #[tokio::test]
    async fn hsts_is_sent_only_when_enabled() {
        let res = app(SecurityHeadersConfig::with_hsts(600, true))
            .oneshot(get_req("/json"))
            .await
            .unwrap();
        assert_eq!(
            res.headers()[header::STRICT_TRANSPORT_SECURITY],
            "max-age=600; includeSubDomains"
        );
    }

    #[tokio::test]
    async fn handler_set_headers_are_not_overridden() {
        let res = app(SecurityHeadersConfig::without_hsts())
            .oneshot(get_req("/custom"))
            .await
            .unwrap();
        assert_eq!(res.headers()[header::REFERRER_POLICY], "same-origin");
        assert_eq!(res.headers()[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
    }

    fn config_from(pairs: &[(&str, &str)]) -> SecurityHeadersConfig {
        let env: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        SecurityHeadersConfig::from_lookup(|k| env.get(k).cloned())
    }

    #[test]
    fn env_config_defaults_to_no_hsts() {
        assert!(config_from(&[]).hsts.is_none());
        assert!(config_from(&[(HSTS_ENABLED_ENV, "false")]).hsts.is_none());
    }

    #[test]
    fn env_config_enables_hsts_with_defaults() {
        let config = config_from(&[(HSTS_ENABLED_ENV, "true")]);
        assert_eq!(config.hsts.unwrap(), "max-age=31536000");
    }

    #[test]
    fn env_config_reads_max_age_and_subdomains() {
        let config = config_from(&[
            (HSTS_ENABLED_ENV, "1"),
            (HSTS_MAX_AGE_SECS_ENV, "86400"),
            (HSTS_INCLUDE_SUBDOMAINS_ENV, "yes"),
        ]);
        assert_eq!(config.hsts.unwrap(), "max-age=86400; includeSubDomains");
    }

    #[test]
    fn env_config_falls_back_on_invalid_max_age() {
        let config = config_from(&[
            (HSTS_ENABLED_ENV, "true"),
            (HSTS_MAX_AGE_SECS_ENV, "forever"),
        ]);
        assert_eq!(config.hsts.unwrap(), "max-age=31536000");
    }
}
