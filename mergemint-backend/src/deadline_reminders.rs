use std::sync::Arc;
use std::time::Duration;

use chrono::{Duration as ChronoDuration, Utc};

use crate::db::{bounties_due_for_deadline_reminder, mark_deadline_reminder_sent};
use crate::events::encode_bounty_event;
use crate::notifications::DEADLINE_APPROACHING_EVENT;
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
                tracing::warn!(
                    env = name,
                    value,
                    default,
                    "invalid {description}; using default"
                );
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

/// Fan a reminder out to live SSE clients on a best-effort basis.
///
/// `broadcast::Sender::send` fails only when there are no receivers at all.
/// That is a normal idle state (no SSE clients connected), *not* a delivery
/// failure: durable notification is handled by the persistent email and
/// webhook consumers started alongside this job, and the durable record is
/// written by [`dispatch_due_reminders`]. An idle channel therefore deserves a
/// debug event, never an error that repeats on every tick.
fn publish_live_reminder(state: &AppState, bounty_id: &str) {
    let event = encode_bounty_event(DEADLINE_APPROACHING_EVENT, bounty_id);
    if let Err(error) = state.bounty_broadcast.send(event) {
        tracing::debug!(
            bounty_id = %bounty_id,
            %error,
            "no live bounty event receivers; deadline reminder is not streamed"
        );
    }
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
        // Live SSE is optional. Never let the number of connected clients
        // decide whether the reminder is recorded as delivered.
        publish_live_reminder(state, &bounty.id);

        // Idempotent durable record: also removes the bounty from the next
        // `bounties_due_for_deadline_reminder` query, so each due bounty is
        // processed exactly once even if this job runs again.
        if mark_deadline_reminder_sent(&state.db, bounty.id.clone()) {
            tracing::debug!(bounty_id = %bounty.id, "recorded deadline reminder");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{acquire_db, new_shared_db, read_db, Bounty};
    use crate::routes::leaderboard::new_leaderboard_cache;
    use crate::routes::tx::new_shared_rate_limiter;
    use std::sync::Mutex;
    use tokio::sync::broadcast::{error::TryRecvError, Receiver};
    use tracing::instrument::WithSubscriber;
    use tracing_subscriber::prelude::*;

    const TEST_LEAD_TIME: Duration = Duration::from_secs(24 * 60 * 60);

    /// State with a sender but no subscribers unless the test keeps the
    /// returned receiver alive. This is the idle-SSE case from #1264.
    fn test_state() -> (Arc<AppState>, Receiver<String>) {
        let (sender, receiver) = tokio::sync::broadcast::channel(16);
        let state = Arc::new(AppState {
            db: new_shared_db(),
            postgres: None,
            idempotency: crate::db::new_shared_idempotency_store(),
            rate_limiter: new_shared_rate_limiter(),
            bounty_broadcast: sender,
            leaderboard_cache: new_leaderboard_cache(),
            sse_keep_alive_duration: Duration::from_secs(15),
            webhook_guard: Default::default(),
            sse_limiter: Default::default(),
        });
        (state, receiver)
    }

    fn add_bounty(state: &AppState, id: &str, status: &str, deadline: chrono::DateTime<Utc>) {
        acquire_db(&state.db).bounties.push(Bounty {
            id: id.to_string(),
            creator: "creator".to_string(),
            title: String::new(),
            description: String::new(),
            assignee: Some("assignee".to_string()),
            created_at: Utc::now(),
            reward: 100,
            reward_token: None,
            reward_decimals: 7,
            deadline: Some(deadline),
            status: status.to_string(),
            tags: Vec::new(),
        });
    }

    fn drain_events(receiver: &mut Receiver<String>) -> Vec<String> {
        let mut events = Vec::new();
        loop {
            match receiver.try_recv() {
                Ok(event) => events.push(event),
                Err(TryRecvError::Empty | TryRecvError::Closed) => return events,
                Err(TryRecvError::Lagged(_)) => {}
            }
        }
    }

    #[tokio::test]
    async fn zero_sse_subscribers_still_records_each_due_bounty_once() {
        let (state, receiver) = test_state();
        drop(receiver);

        let now = Utc::now();
        let soon = now + ChronoDuration::hours(1);
        add_bounty(&state, "due-1", "claimed", soon);
        add_bounty(&state, "due-2", "claimed", soon);
        add_bounty(&state, "later", "claimed", now + ChronoDuration::hours(48));
        add_bounty(&state, "not-claimed", "open", soon);

        // Repeat the tick: idempotency must prevent a second pass over the
        // same bounties even when nobody is listening on the SSE channel.
        dispatch_due_reminders(&state, TEST_LEAD_TIME).await;
        dispatch_due_reminders(&state, TEST_LEAD_TIME).await;

        let sent = read_db(&state.db).deadline_reminders_sent.clone();
        assert_eq!(sent.len(), 2, "each due bounty must be processed once");
        assert!(sent.contains("due-1"));
        assert!(sent.contains("due-2"));
        assert!(!sent.contains("later"));
        assert!(!sent.contains("not-claimed"));
    }

    #[tokio::test]
    async fn each_due_bounty_is_published_once_to_a_live_subscriber() {
        let (state, mut receiver) = test_state();
        let now = Utc::now();
        let soon = now + ChronoDuration::hours(1);
        add_bounty(&state, "due-1", "claimed", soon);
        add_bounty(&state, "due-2", "claimed", soon);
        add_bounty(&state, "later", "claimed", now + ChronoDuration::hours(48));

        dispatch_due_reminders(&state, TEST_LEAD_TIME).await;
        dispatch_due_reminders(&state, TEST_LEAD_TIME).await;

        let events = drain_events(&mut receiver);
        assert_eq!(events.len(), 2, "each due bounty must be published once");
        assert!(events.contains(&encode_bounty_event(DEADLINE_APPROACHING_EVENT, "due-1")));
        assert!(events.contains(&encode_bounty_event(DEADLINE_APPROACHING_EVENT, "due-2")));
    }

    /// Collects ERROR-level log metadata emitted while a future runs.
    #[derive(Default)]
    struct ErrorLogs(Arc<Mutex<Vec<String>>>);

    impl<S> tracing_subscriber::Layer<S> for ErrorLogs
    where
        S: tracing::Subscriber,
    {
        fn on_event(
            &self,
            event: &tracing::Event<'_>,
            _context: tracing_subscriber::layer::Context<'_, S>,
        ) {
            if *event.metadata().level() == tracing::Level::ERROR {
                self.0
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .push(event.metadata().name().to_string());
            }
        }
    }

    #[tokio::test]
    async fn idle_channel_emits_no_error_logs() {
        let errors = Arc::new(Mutex::new(Vec::new()));
        let subscriber = tracing_subscriber::registry().with(ErrorLogs(errors.clone()));

        async {
            let (state, receiver) = test_state();
            drop(receiver);
            add_bounty(
                &state,
                "due-1",
                "claimed",
                Utc::now() + ChronoDuration::hours(1),
            );
            dispatch_due_reminders(&state, TEST_LEAD_TIME).await;
        }
        .with_subscriber(subscriber)
        .await;

        let logged = errors.lock().unwrap_or_else(|error| error.into_inner());
        assert!(
            logged.is_empty(),
            "unexpected error logs on idle channel: {logged:?}"
        );
    }
}
