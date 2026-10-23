use std::sync::Arc;
use std::time::Duration;

use chrono::{Duration as ChronoDuration, Utc};

use crate::db::{bounties_due_for_deadline_reminder, mark_deadline_reminder_sent};
use crate::events::encode_bounty_event;
use crate::routes::tx::AppState;

pub const DEADLINE_REMINDER_LEAD_HOURS_ENV: &str = "DEADLINE_REMINDER_LEAD_HOURS";
pub const DEADLINE_REMINDER_INTERVAL_SECS_ENV: &str = "DEADLINE_REMINDER_INTERVAL_SECS";
const DEFAULT_LEAD_HOURS: u64 = 24;
const DEFAULT_INTERVAL_SECS: u64 = 60;
const MAX_LEAD_HOURS: u64 = 24 * 365 * 100;
const MAX_INTERVAL_SECS: u64 = 365 * 24 * 60 * 60;

pub fn read_deadline_reminder_lead_time() -> Duration {
    Duration::from_secs(
        read_positive_env(
            DEADLINE_REMINDER_LEAD_HOURS_ENV,
            DEFAULT_LEAD_HOURS,
            MAX_LEAD_HOURS,
            "deadline reminder lead time",
        )
        .saturating_mul(3600),
    )
}

pub fn read_deadline_reminder_interval() -> Duration {
    Duration::from_secs(read_positive_env(
        DEADLINE_REMINDER_INTERVAL_SECS_ENV,
        DEFAULT_INTERVAL_SECS,
        MAX_INTERVAL_SECS,
        "deadline reminder interval",
    ))
}

fn read_positive_env(name: &str, default: u64, max: u64, description: &str) -> u64 {
    match std::env::var(name) {
        Ok(value) => match value
            .parse::<u64>()
            .ok()
            .filter(|value| *value > 0 && *value <= max)
        {
            Some(value) => value,
            None => {
                tracing::warn!(env = name, value, default, "invalid {description}; using default");
                default
            }
        },
        Err(_) => default,
    }
}

pub fn start_deadline_reminder_job(
    state: Arc<AppState>,
    lead_time: Duration,
    interval: Duration,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        loop {
            ticker.tick().await;
            dispatch_due_reminders(&state, lead_time).await;
        }
    })
}

pub async fn dispatch_due_reminders(state: &AppState, lead_time: Duration) {
    let now = Utc::now();
    let lead_time = ChronoDuration::from_std(lead_time).unwrap_or_else(|error| {
        tracing::warn!(%error, "deadline reminder lead time is out of range; using 24 hours");
        ChronoDuration::hours(24)
    });
    let reminder_at = now
        .checked_add_signed(lead_time)
        .unwrap_or_else(|| now + ChronoDuration::days(36500));
    let due = bounties_due_for_deadline_reminder(&state.db, now, reminder_at);

    for bounty in due {
        let event = encode_bounty_event("deadline_approaching", &bounty.id);
        match state.bounty_broadcast.send(event) {
            Ok(_) => {
                mark_deadline_reminder_sent(&state.db, bounty.id.clone());
            }
            Err(error) => {
                tracing::error!(
                    bounty_id = %bounty.id,
                    %error,
                    "failed to publish deadline reminder; it will be retried"
                );
            }
        }
    }
}
