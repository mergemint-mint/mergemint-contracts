//! OpenAPI description of the bounty read API (#865, #866, #1277).
//!
//! Kept in the library so both the HTTP binary and contract tests generate the
//! exact same document. The binary previously declared its own private copy,
//! which made the committed snapshot impossible to validate from tests.

use utoipa::OpenApi;

/// OpenAPI description served at `/api-docs/openapi.json`.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "MergeMint backend API",
        description = "Rate-limited responses include X-RateLimit-Limit and X-RateLimit-Remaining; 429 responses include Retry-After in seconds."
    ),
    paths(
        crate::routes::bounties::list_bounties,
        crate::routes::bounties::list_bounties_by_assignee,
        crate::routes::bounties::get_bounty_route,
        crate::routes::bounties::claim_bounty,
        crate::routes::bounties::bounty_stream,
        crate::routes::leaderboard::get_leaderboard_route,
        crate::routes::reputation::get_reputation_history,
        crate::routes::stats::get_stats_route,
        crate::routes::tx::query_audit_logs,
        crate::routes::tx::cancel_bounty_transaction,
        crate::routes::tx::top_up_bounty_transaction,
        crate::routes::tx::extend_deadline_transaction,
        crate::routes::tx::submit_signed_transaction,
        crate::webhooks::list_failed_webhooks_route,
        crate::webhooks::replay_webhook_route,
        crate::notifications::create_email_subscription,
        crate::notifications::verify_email,
        crate::notifications::unsubscribe_email,
        crate::webhooks::list_subscriptions_route,
        crate::webhooks::get_subscription_route,
    ),
    components(schemas(
        crate::db::Bounty,
        crate::db::BountyPage,
        crate::db::BountySortField,
        crate::db::SortOrder,
        crate::routes::bounties::ClaimResponse,
        crate::routes::tx::AppError,
        crate::routes::tx::QueryAuditLogsResponse,
        crate::routes::reputation::ReputationHistoryPage,
        crate::routes::tx::CancelBountyRequest,
        crate::routes::tx::TopUpBountyRequest,
        crate::routes::tx::ExtendDeadlineRequest,
        crate::routes::tx::UnsignedTransactionResponse,
        crate::routes::tx::SubmitSignedTransactionRequest,
        crate::routes::tx::SubmitTransactionResponse,
        crate::db::AuditLog,
        crate::db::LeaderboardEntry,
        crate::db::LeaderboardPage,
        crate::db::ReputationHistoryEntry,
        crate::db::StatsResponse,
        crate::db::TokenTotal,
        crate::db::WebhookSubscription,
        crate::webhooks::FailedWebhookDelivery,
        crate::webhooks::FailedWebhookPage,
        crate::notifications::CreateEmailSubscriptionRequest,
    )),
    tags(
        (name = "bounties", description = "Bounty listing and live updates"),
        (name = "notifications", description = "Email notification subscriptions")
    )
)]
pub struct ApiDoc;
