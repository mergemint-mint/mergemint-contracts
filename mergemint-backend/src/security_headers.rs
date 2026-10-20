// mergemint-backend/src/security_headers.rs
//
// Axum middleware that appends standard security headers to every response
// (issue #1115).
//
// ## Headers set
//
//   X-Content-Type-Options: nosniff
//     Prevents browsers from MIME-sniffing a response away from the declared
//     Content-Type.
//
//   X-Frame-Options: DENY
//     Legacy clickjacking guard for browsers that don't support CSP
//     frame-ancestors.
//
//   Referrer-Policy: strict-origin-when-cross-origin
//     Sends the full URL as the Referer on same-origin requests, only the
//     origin on cross-origin HTTPS→HTTPS requests, and nothing on downgrade.
//
//   X-XSS-Protection: 0
//     Explicitly disables the legacy browser XSS filter, which can itself be
//     abused in some browsers.  The correct XSS mitigation is CSP, not this
//     header.
//
//   Permissions-Policy: (restrictive subset)
//     Opts out of features the app does not use (geolocation, camera, etc.)
//     to reduce the attack surface if a dependency is ever compromised.
//
//   Strict-Transport-Security: max-age=63072000; includeSubDomains
//     Tells compliant browsers to only contact this host over HTTPS for the
//     next two years.  **Only emitted when `hsts_enabled` is `true`** (set via
//     the `HSTS_ENABLED` environment variable) so HTTP-only local/dev setups
//     are not broken.
//
// ## SSE and CORS compatibility
//
// The middleware runs *after* the CORS layer and *after* the SSE handler has
// already written its status line.  The headers are appended in a
// `map_response` pass; they are added even to streaming SSE responses.
// None of the security headers interfere with SSE chunked transfer or with
// the CORS headers already set by tower-http's `CorsLayer`.
//
// ## Tests
//
// See the `tests` module at the bottom of this file.

use axum::{
    body::Body,
    extract::State,
    http::{
        header::{HeaderName, HeaderValue, STRICT_TRANSPORT_SECURITY},
        Request, Response,
    },
    middleware::Next,
    response::IntoResponse,
};

/// Environment variable that controls Strict-Transport-Security emission.
pub const HSTS_ENABLED_ENV: &str = "HSTS_ENABLED";

/// HSTS `max-age` value: two years in seconds.
const HSTS_MAX_AGE: u64 = 63_072_000;

/// Read whether HSTS should be emitted from the environment.
pub fn hsts_enabled() -> bool {
    std::env::var(HSTS_ENABLED_ENV)
        .ok()
        .map(|v| v.trim().eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// Axum middleware function that adds security headers to every response.
///
/// Wire it up with:
/// ```rust
/// use axum::middleware;
/// use mergemint_backend::security_headers::{security_headers_middleware, hsts_enabled};
///
/// let app = Router::new()
///     // ... routes ...
///     .layer(middleware::from_fn_with_state(
///         hsts_enabled(),
///         security_headers_middleware,
///     ));
/// ```
pub async fn security_headers_middleware(
    State(hsts): State<bool>,
    request: Request<Body>,
    next: Next,
) -> impl IntoResponse {
    let mut response: Response<Body> = next.run(request).await;
    let headers = response.headers_mut();

    // X-Content-Type-Options
    headers.insert(
        HeaderName::from_static("x-content-type-options"),
        HeaderValue::from_static("nosniff"),
    );

    // X-Frame-Options
    headers.insert(
        HeaderName::from_static("x-frame-options"),
        HeaderValue::from_static("DENY"),
    );

    // Referrer-Policy
    headers.insert(
        HeaderName::from_static("referrer-policy"),
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );

    // X-XSS-Protection — disable the legacy filter
    headers.insert(
        HeaderName::from_static("x-xss-protection"),
        HeaderValue::from_static("0"),
    );

    // Permissions-Policy
    headers.insert(
        HeaderName::from_static("permissions-policy"),
        HeaderValue::from_static(
            "geolocation=(), camera=(), microphone=(), payment=(), usb=()",
        ),
    );

    // Strict-Transport-Security (HTTPS only)
    if hsts {
        headers.insert(
            STRICT_TRANSPORT_SECURITY,
            HeaderValue::from_str(&format!(
                "max-age={HSTS_MAX_AGE}; includeSubDomains"
            ))
            .expect("valid HSTS header value"),
        );
    }

    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, routing::get, Router};
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    fn test_app(hsts: bool) -> Router {
        Router::new()
            .route("/", get(|| async { "ok" }))
            .layer(axum::middleware::from_fn_with_state(
                hsts,
                security_headers_middleware,
            ))
    }

    async fn get_response(app: Router, uri: &str) -> axum::http::Response<Body> {
        app.oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn sets_x_content_type_options() {
        let app = test_app(false);
        let resp = get_response(app, "/").await;
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            resp.headers()
                .get("x-content-type-options")
                .and_then(|v| v.to_str().ok()),
            Some("nosniff")
        );
    }

    #[tokio::test]
    async fn sets_x_frame_options() {
        let app = test_app(false);
        let resp = get_response(app, "/").await;
        assert_eq!(
            resp.headers()
                .get("x-frame-options")
                .and_then(|v| v.to_str().ok()),
            Some("DENY")
        );
    }

    #[tokio::test]
    async fn sets_referrer_policy() {
        let app = test_app(false);
        let resp = get_response(app, "/").await;
        assert_eq!(
            resp.headers()
                .get("referrer-policy")
                .and_then(|v| v.to_str().ok()),
            Some("strict-origin-when-cross-origin")
        );
    }

    #[tokio::test]
    async fn disables_xss_protection() {
        let app = test_app(false);
        let resp = get_response(app, "/").await;
        assert_eq!(
            resp.headers()
                .get("x-xss-protection")
                .and_then(|v| v.to_str().ok()),
            Some("0")
        );
    }

    #[tokio::test]
    async fn sets_permissions_policy() {
        let app = test_app(false);
        let resp = get_response(app, "/").await;
        let header = resp
            .headers()
            .get("permissions-policy")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        assert!(header.contains("geolocation=()"));
        assert!(header.contains("camera=()"));
    }

    #[tokio::test]
    async fn omits_hsts_when_disabled() {
        let app = test_app(false);
        let resp = get_response(app, "/").await;
        assert!(
            resp.headers()
                .get("strict-transport-security")
                .is_none(),
            "HSTS header must not be present when hsts=false"
        );
    }

    #[tokio::test]
    async fn emits_hsts_when_enabled() {
        let app = test_app(true);
        let resp = get_response(app, "/").await;
        let hsts_val = resp
            .headers()
            .get("strict-transport-security")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        assert!(
            hsts_val.contains("max-age=63072000"),
            "HSTS header must include max-age"
        );
        assert!(
            hsts_val.contains("includeSubDomains"),
            "HSTS header must include includeSubDomains"
        );
    }
}
