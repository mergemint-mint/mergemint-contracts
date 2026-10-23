use std::sync::Arc;
use std::time::Duration;

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{Html, IntoResponse},
    Json,
};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use rustls::pki_types::ServerName;
use serde::Deserialize;
use tokio::{
    io::{AsyncBufRead, AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpStream,
};
use tokio_rustls::{client::TlsStream, TlsConnector};
use utoipa::IntoParams;
use uuid::Uuid;

use crate::{
    db::{
        add_email_subscription, get_bounty, unsubscribe_email_subscription,
        verified_email_subscriptions_for, verify_email_subscription, EmailSubscription,
    },
    events::decode_bounty_event,
    routes::tx::AppState,
};

const PUBLIC_BASE_URL_ENV: &str = "PUBLIC_BASE_URL";
const SMTP_HOST_ENV: &str = "SMTP_HOST";
const SMTP_PORT_ENV: &str = "SMTP_PORT";
const SMTP_USERNAME_ENV: &str = "SMTP_USERNAME";
const SMTP_PASSWORD_ENV: &str = "SMTP_PASSWORD";
const SMTP_FROM_ENV: &str = "SMTP_FROM";

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct CreateEmailSubscriptionRequest {
    pub contributor_address: String,
    pub email: String,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct TokenQuery {
    pub token: String,
}

struct SmtpConfig {
    host: String,
    port: u16,
    username: Option<String>,
    password: Option<String>,
    from: String,
}

impl SmtpConfig {
    fn from_env() -> anyhow::Result<Self> {
        let host = std::env::var(SMTP_HOST_ENV)
            .map_err(|_| anyhow::anyhow!("{SMTP_HOST_ENV} is required for email delivery"))?;
        let from = std::env::var(SMTP_FROM_ENV)
            .map_err(|_| anyhow::anyhow!("{SMTP_FROM_ENV} is required for email delivery"))?;
        let port = std::env::var(SMTP_PORT_ENV)
            .unwrap_or_else(|_| "465".to_string())
            .parse::<u16>()?;
        let username = std::env::var(SMTP_USERNAME_ENV)
            .ok()
            .filter(|value| !value.trim().is_empty());
        let password = std::env::var(SMTP_PASSWORD_ENV)
            .ok()
            .filter(|value| !value.trim().is_empty());
        anyhow::ensure!(
            username.is_some() == password.is_some(),
            "{SMTP_USERNAME_ENV} and {SMTP_PASSWORD_ENV} must be configured together"
        );
        anyhow::ensure!(port > 0, "{SMTP_PORT_ENV} must be a positive port");
        anyhow::ensure!(!host.trim().is_empty(), "{SMTP_HOST_ENV} cannot be empty");
        validate_mailbox(&from)?;

        Ok(Self {
            host,
            port,
            username,
            password,
            from,
        })
    }
}

fn validate_mailbox(mailbox: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        !mailbox
            .chars()
            .any(|character| matches!(character, '\r' | '\n' | '<' | '>'))
            && mailbox.matches('@').count() == 1
            && mailbox.split_once('@').is_some_and(|(local, domain)| {
                !local.is_empty()
                    && !domain.is_empty()
                    && !mailbox.chars().any(char::is_whitespace)
            }),
        "invalid email address"
    );
    Ok(())
}

fn public_base_url() -> anyhow::Result<String> {
    let value = std::env::var(PUBLIC_BASE_URL_ENV)
        .map_err(|_| anyhow::anyhow!("{PUBLIC_BASE_URL_ENV} is required"))?;
    let base = value.trim_end_matches('/');
    let parsed = reqwest::Url::parse(base)?;
    let is_localhost = parsed.scheme() == "http"
        && matches!(
            parsed.host_str(),
            Some("localhost" | "127.0.0.1" | "::1" | "[::1]")
        );
    anyhow::ensure!(
        parsed.scheme() == "https" || is_localhost,
        "{PUBLIC_BASE_URL_ENV} must use HTTPS outside localhost"
    );
    anyhow::ensure!(
        parsed.username().is_empty()
            && parsed.password().is_none()
            && parsed.query().is_none()
            && parsed.fragment().is_none(),
        "{PUBLIC_BASE_URL_ENV} cannot contain credentials, a query, or a fragment"
    );
    Ok(base.to_string())
}

async fn send_email(to: &str, subject: &str, body: &str) -> anyhow::Result<()> {
    tokio::time::timeout(Duration::from_secs(15), async {
        let config = SmtpConfig::from_env()?;
        validate_mailbox(to)?;
        anyhow::ensure!(
            !subject.chars().any(|character| matches!(character, '\r' | '\n')),
            "email subject contains an invalid line break"
        );

        let tcp = TcpStream::connect((config.host.as_str(), config.port)).await?;
        let mut roots = rustls::RootCertStore::empty();
        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        let tls_config = rustls::ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth();
        let connector = TlsConnector::from(Arc::new(tls_config));
        let server_name = ServerName::try_from(config.host.clone())?;
        let tls = connector.connect(server_name, tcp).await?;
        smtp_send(tls, &config, to, subject, body).await
    })
    .await?
}

async fn smtp_send(
    tls: TlsStream<TcpStream>,
    config: &SmtpConfig,
    to: &str,
    subject: &str,
    body: &str,
) -> anyhow::Result<()> {
    let mut stream = BufReader::new(tls);
    expect_smtp_reply(&mut stream, 220).await?;
    smtp_command(&mut stream, "EHLO mergemint\r\n", 250).await?;

    if let (Some(username), Some(password)) = (&config.username, &config.password) {
        let credentials = STANDARD.encode(format!("\0{username}\0{password}"));
        smtp_command(
            &mut stream,
            &format!("AUTH PLAIN {credentials}\r\n"),
            235,
        )
        .await?;
    }

    smtp_command(
        &mut stream,
        &format!("MAIL FROM:<{}>\r\n", config.from),
        250,
    )
    .await?;
    smtp_command(&mut stream, &format!("RCPT TO:<{to}>\r\n"), 250).await?;
    smtp_command(&mut stream, "DATA\r\n", 354).await?;

    let mut message = format!(
        "From: {}\r\nTo: {}\r\nSubject: {}\r\nMIME-Version: 1.0\r\nContent-Type: text/plain; charset=UTF-8\r\nContent-Transfer-Encoding: 8bit\r\n\r\n",
        config.from, to, subject
    );
    for line in body.lines() {
        if line.starts_with('.') {
            message.push('.');
        }
        message.push_str(line);
        message.push_str("\r\n");
    }
    message.push_str(".\r\n");
    stream.get_mut().write_all(message.as_bytes()).await?;
    expect_smtp_reply(&mut stream, 250).await?;
    stream.get_mut().write_all(b"QUIT\r\n").await?;
    Ok(())
}

async fn smtp_command(
    stream: &mut BufReader<TlsStream<TcpStream>>,
    command: &str,
    expected: u16,
) -> anyhow::Result<()> {
    stream.get_mut().write_all(command.as_bytes()).await?;
    expect_smtp_reply(stream, expected).await
}

async fn expect_smtp_reply<R: AsyncBufRead + Unpin>(
    stream: &mut R,
    expected: u16,
) -> anyhow::Result<()> {
    let (code, reply) = read_smtp_reply(stream).await?;
    anyhow::ensure!(
        code == expected,
        "SMTP server returned {code}: {}",
        reply.trim()
    );
    Ok(())
}

async fn read_smtp_reply<R: AsyncBufRead + Unpin>(stream: &mut R) -> anyhow::Result<(u16, String)> {
    let mut full_reply = String::new();
    let mut code = None;
    loop {
        let mut line = String::new();
        if stream.read_line(&mut line).await? == 0 {
            anyhow::bail!("SMTP server closed the connection");
        }
        if code.is_none() {
            code = line.get(..3).and_then(|prefix| prefix.parse::<u16>().ok());
        }
        let continues = line.as_bytes().get(3) == Some(&b'-');
        full_reply.push_str(&line);
        if !continues {
            return Ok((
                code.ok_or_else(|| anyhow::anyhow!("invalid SMTP response"))?,
                full_reply,
            ));
        }
    }
}

#[utoipa::path(
    post,
    path = "/notifications/subscriptions",
    request_body = CreateEmailSubscriptionRequest,
    responses(
        (status = 202, description = "Subscription request accepted"),
        (status = 400, description = "Invalid contributor address or email"),
        (status = 503, description = "Email delivery is not configured"),
    ),
    tag = "notifications"
)]
pub async fn create_email_subscription(
    State(state): State<Arc<AppState>>,
    Json(request): Json<CreateEmailSubscriptionRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let address = request.contributor_address.trim();
    let email = request.email.trim();
    if !crate::routes::bounties::is_syntactically_valid_address(address)
        || validate_mailbox(email).is_err()
    {
        return Err(email_error(
            StatusCode::BAD_REQUEST,
            "A contributor address and valid email address are required",
        ));
    }

    let base_url = public_base_url().map_err(|error| {
        tracing::error!(%error, "email verification URL is not configured");
        email_error(StatusCode::SERVICE_UNAVAILABLE, "Email notifications are not configured")
    })?;
    if let Err(error) = SmtpConfig::from_env() {
        tracing::error!(%error, "SMTP email delivery is not configured");
        return Err(email_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "Email notifications are not configured",
        ));
    }
    let verification_token = Uuid::new_v4().simple().to_string();
    let unsubscribe_token = Uuid::new_v4().simple().to_string();
    let subscription = add_email_subscription(
        &state.db,
        address.to_string(),
        email.to_string(),
        verification_token.clone(),
        unsubscribe_token.clone(),
    );
    if subscription.verified {
        return Ok((
            StatusCode::ACCEPTED,
            Json(serde_json::json!({"status": "accepted"})),
        ));
    }
    let verify_url = format!("{base_url}/notifications/verify?token={verification_token}");
    let unsubscribe_url =
        format!("{base_url}/notifications/unsubscribe?token={unsubscribe_token}");
    let body = format!(
        "Confirm email notifications for contributor {address} by visiting:\n\n{verify_url}\n\nIf you did not request this, ignore this email. To cancel this pending subscription, visit:\n{unsubscribe_url}"
    );
    if let Err(error) = send_email(email, "Verify MergeMint email notifications", &body).await {
        tracing::error!(%error, "failed to send email verification message");
        return Err(email_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "Unable to send verification email",
        ));
    }

    Ok((
        StatusCode::ACCEPTED,
        Json(serde_json::json!({"status": "accepted"})),
    ))
}

#[utoipa::path(
    get,
    path = "/notifications/verify",
    params(TokenQuery),
    responses((status = 200, description = "Email verification result")),
    tag = "notifications"
)]
pub async fn verify_email(
    State(state): State<Arc<AppState>>,
    Query(query): Query<TokenQuery>,
) -> impl IntoResponse {
    if verify_email_subscription(&state.db, &query.token) {
        Html("<p>Email notifications are verified and enabled.</p>")
    } else {
        Html("<p>This verification link is invalid or has already been used.</p>")
    }
}

#[utoipa::path(
    get,
    path = "/notifications/unsubscribe",
    params(TokenQuery),
    responses((status = 200, description = "Email unsubscribe result")),
    tag = "notifications"
)]
pub async fn unsubscribe_email(
    State(state): State<Arc<AppState>>,
    Query(query): Query<TokenQuery>,
) -> impl IntoResponse {
    if unsubscribe_email_subscription(&state.db, &query.token) {
        Html("<p>Email notifications have been unsubscribed.</p>")
    } else {
        Html("<p>This unsubscribe link is invalid or has already been used.</p>")
    }
}

fn email_error(
    status: StatusCode,
    message: &str,
) -> (StatusCode, Json<serde_json::Value>) {
    (status, Json(serde_json::json!({"error": message})))
}

pub fn start_email_notifier(state: Arc<AppState>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut receiver = state.bounty_broadcast.subscribe();
        loop {
            match receiver.recv().await {
                Ok(message) => dispatch_email_event(&state, &message).await,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(missed)) => {
                    tracing::error!(missed, "email notifier missed bounty events");
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    tracing::info!("Bounty event channel closed; exiting email notifier");
                    break;
                }
            }
        }
    })
}

async fn dispatch_email_event(state: &AppState, message: &str) {
    let event = decode_bounty_event(message);
    let Some((subject, description)) = email_template(&event.name) else {
        return;
    };
    let Some(bounty) = get_bounty(&state.db, &event.bounty_id) else {
        tracing::warn!(
            event = %event.name,
            bounty_id = %event.bounty_id,
            "cannot send email notification for unknown bounty"
        );
        return;
    };
    let Some(assignee) = bounty.assignee.as_deref() else {
        return;
    };
    let subscriptions = verified_email_subscriptions_for(&state.db, assignee);
    let base_url = match public_base_url() {
        Ok(url) => url,
        Err(error) => {
            tracing::error!(%error, "email unsubscribe URL is not configured");
            return;
        }
    };

    for subscription in subscriptions {
        let body = render_event_email(
            description,
            &bounty.id,
            &bounty.status,
            &base_url,
            &subscription,
        );
        if let Err(error) = send_email(&subscription.email, subject, &body).await {
            tracing::error!(
                event = %event.name,
                bounty_id = %bounty.id,
                %error,
                "failed to deliver email notification"
            );
        }
    }
}

fn email_template(event: &str) -> Option<(&'static str, &'static str)> {
    match event {
        "bounty_claimed" => Some(("Bounty claim recorded", "Your bounty claim was recorded.")),
        "approval_recorded" | "bounty_completed" => Some((
            "Bounty work approved",
            "Your submitted work received an approval.",
        )),
        "bounty_disputed" | "dispute_resolved" => Some((
            "Bounty dispute update",
            "There is an update to a dispute on your bounty.",
        )),
        "deadline_approaching" => Some((
            "Bounty deadline approaching",
            "A claimed bounty is approaching its deadline.",
        )),
        "reward_paid" => Some(("Bounty payout sent", "A payout was recorded for your bounty.")),
        _ => None,
    }
}

fn render_event_email(
    description: &str,
    bounty_id: &str,
    status: &str,
    base_url: &str,
    subscription: &EmailSubscription,
) -> String {
    let unsubscribe_url = format!(
        "{base_url}/notifications/unsubscribe?token={}",
        subscription.unsubscribe_token
    );
    format!(
        "{description}\n\nBounty: {bounty_id}\nStatus: {status}\n\nUnsubscribe from all email notifications:\n{unsubscribe_url}"
    )
}
