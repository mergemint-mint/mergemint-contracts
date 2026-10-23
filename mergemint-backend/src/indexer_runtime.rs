use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::PgPool;
use stellar_xdr::{Limits, ReadXdr, ScAddress, ScVal};
use tracing::{info, warn};

const PAGE_SIZE: u32 = 100;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RpcEnvelope<T> {
    result: Option<T>,
    error: Option<Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetEventsResult {
    events: Vec<RpcEvent>,
    latest_ledger: u32,
    oldest_ledger: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RpcEvent {
    paging_token: String,
    ledger: u32,
    ledger_closed_at: String,
    topic: Vec<String>,
    value: String,
    tx_hash: String,
    contract_id: Option<String>,
}

pub async fn run_live(pool: PgPool, rpc_url: String, contract_id: String) -> Result<()> {
    let checkpoint_name = format!("indexer:{contract_id}");
    loop {
        if let Err(error) =
            poll_range(&pool, &rpc_url, &contract_id, &checkpoint_name, None, None).await
        {
            warn!(%error, "indexer poll failed; retrying after delay");
        }
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    }
}

pub async fn backfill(
    pool: &PgPool,
    rpc_url: &str,
    contract_id: &str,
    from_ledger: u32,
    to_ledger: Option<u32>,
) -> Result<()> {
    if from_ledger == 0 || to_ledger.is_some_and(|to| to < from_ledger) {
        return Err(anyhow!("ledger range must be positive and ordered"));
    }
    let checkpoint = format!(
        "backfill:{contract_id}:{from_ledger}:{}",
        to_ledger.unwrap_or_default()
    );
    poll_range(
        pool,
        rpc_url,
        contract_id,
        &checkpoint,
        Some(from_ledger),
        to_ledger,
    )
    .await
}

async fn poll_range(
    pool: &PgPool,
    rpc_url: &str,
    contract_id: &str,
    checkpoint_name: &str,
    from_ledger: Option<u32>,
    to_ledger: Option<u32>,
) -> Result<()> {
    let mut checkpoint = load_checkpoint(pool, checkpoint_name).await?;
    let (mut start_ledger, mut cursor) = if let Some(from) = from_ledger {
        if to_ledger.is_some_and(|to| checkpoint.0 >= to) {
            return Ok(());
        }
        if checkpoint.0 >= from {
            (checkpoint.0, checkpoint.1.take())
        } else {
            (from, None)
        }
    } else {
        (checkpoint.0.saturating_add(1).max(1), checkpoint.1.take())
    };
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    loop {
        let mut params = json!({
            "startLedger": start_ledger,
            "filters": [{ "type": "contract", "contractIds": [contract_id] }],
            "pagination": { "limit": PAGE_SIZE },
        });
        if let Some(ref next_cursor) = cursor {
            params["pagination"]["cursor"] = json!(next_cursor);
        }

        let response: RpcEnvelope<GetEventsResult> = client
            .post(rpc_url)
            .json(&json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "getEvents",
                "params": params,
            }))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        if let Some(error) = response.error {
            return Err(anyhow!("Soroban RPC getEvents failed: {error}"));
        }
        let result = response
            .result
            .context("Soroban RPC getEvents response omitted result")?;

        if start_ledger < result.oldest_ledger {
            let missing_to = result.oldest_ledger.saturating_sub(1);
            warn!(
                missing_from = start_ledger,
                missing_to, "RPC has pruned indexer ledgers"
            );
            let inserted = sqlx::query(
                "INSERT INTO indexer_gaps (checkpoint_name, missing_from, missing_to) \
                 VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
            )
            .bind(checkpoint_name)
            .bind(i64::from(start_ledger))
            .bind(i64::from(missing_to))
            .execute(pool)
            .await?
            .rows_affected()
                > 0;
            if inserted {
                crate::metrics::record_indexer_ledger_gap();
            }
            start_ledger = result.oldest_ledger;
            cursor = None;
        }

        let event_count = result.events.len();
        let mut reached_end = false;
        let mut last_processed = None;
        for event in &result.events {
            if to_ledger.is_some_and(|end| event.ledger > end) {
                reached_end = true;
                break;
            }
            store_event(pool, contract_id, event).await?;
            if index_reputation_event(pool, event).await? {
                crate::routes::leaderboard::invalidate_leaderboard_cache();
            }
            cursor = Some(event.paging_token.clone());
            start_ledger = event.ledger;
            last_processed = Some(event);
        }

        if let Some(last) = last_processed {
            save_checkpoint(pool, checkpoint_name, last.ledger, cursor.as_deref()).await?;
            checkpoint = (last.ledger, cursor.clone());
        } else if checkpoint_name.starts_with("indexer:") && result.latest_ledger >= start_ledger {
            save_checkpoint(pool, checkpoint_name, result.latest_ledger, None).await?;
            checkpoint = (result.latest_ledger, None);
        }
        info!(
            checkpoint = checkpoint_name,
            events = event_count,
            last_ledger = checkpoint.0,
            latest_ledger = result.latest_ledger,
            "indexer page persisted"
        );

        if reached_end
            || event_count < PAGE_SIZE as usize
            || to_ledger.is_some_and(|end| start_ledger >= end)
        {
            if from_ledger.is_some() {
                info!(
                    from_ledger = from_ledger.unwrap_or_default(),
                    to_ledger, "backfill complete"
                );
            }
            return Ok(());
        }
        if checkpoint.0 > 0 {
            start_ledger = checkpoint.0;
        }
    }
}

async fn load_checkpoint(pool: &PgPool, name: &str) -> Result<(u32, Option<String>)> {
    let row = sqlx::query_as::<_, (i64, Option<String>)>(
        "SELECT ledger, cursor FROM indexer_checkpoints WHERE name = $1",
    )
    .bind(name)
    .fetch_optional(pool)
    .await?;
    match row {
        Some((ledger, cursor)) => Ok((u32::try_from(ledger)?, cursor)),
        None => Ok((0, None)),
    }
}

async fn save_checkpoint(
    pool: &PgPool,
    name: &str,
    ledger: u32,
    cursor: Option<&str>,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO indexer_checkpoints (name, ledger, cursor) VALUES ($1, $2, $3) \
         ON CONFLICT (name) DO UPDATE SET ledger = GREATEST(indexer_checkpoints.ledger, EXCLUDED.ledger), \
         cursor = CASE WHEN EXCLUDED.ledger >= indexer_checkpoints.ledger THEN EXCLUDED.cursor ELSE indexer_checkpoints.cursor END, \
         updated_at = NOW()",
    )
    .bind(name)
    .bind(i64::from(ledger))
    .bind(cursor)
    .execute(pool)
    .await?;
    Ok(())
}

async fn store_event(pool: &PgPool, contract_id: &str, event: &RpcEvent) -> Result<()> {
    let timestamp = DateTime::parse_from_rfc3339(&event.ledger_closed_at)?.with_timezone(&Utc);
    let topic_json = serde_json::to_value(&event.topic)?;
    let value_json = json!(event.value);
    let event_type = event
        .topic
        .first()
        .and_then(|topic| decode_scval(topic).ok())
        .and_then(scval_symbol)
        .unwrap_or_else(|| "unknown".to_owned());
    sqlx::query(
        "INSERT INTO indexed_contract_events \
         (event_id, ledger, event_type, contract_id, transaction_hash, ledger_closed_at, topics, value) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8) ON CONFLICT (event_id) DO NOTHING",
    )
    .bind(&event.paging_token)
    .bind(i64::from(event.ledger))
    .bind(event_type)
    .bind(event.contract_id.as_deref().unwrap_or(contract_id))
    .bind(&event.tx_hash)
    .bind(timestamp)
    .bind(topic_json)
    .bind(value_json)
    .execute(pool)
    .await?;
    Ok(())
}

async fn index_reputation_event(pool: &PgPool, event: &RpcEvent) -> Result<bool> {
    let Some(event_name) = event
        .topic
        .first()
        .and_then(|topic| decode_scval(topic).ok())
        .and_then(scval_symbol)
    else {
        return Ok(false);
    };
    if event_name != "reputation_changed" {
        return Ok(false);
    }
    let Some(address) = event
        .topic
        .get(1)
        .and_then(|topic| decode_scval(topic).ok())
        .and_then(scval_address)
    else {
        return Err(anyhow!(
            "reputation_changed event has an invalid contributor topic"
        ));
    };
    let ScVal::Vec(Some(values)) = decode_scval(&event.value)? else {
        return Err(anyhow!("reputation_changed event data is not an XDR tuple"));
    };
    let values = &values.0;
    let (Some(ScVal::U32(old_value)), Some(ScVal::U32(new_value)), Some(reason)) =
        (values.first(), values.get(1), values.get(2))
    else {
        return Err(anyhow!(
            "reputation_changed event tuple has an invalid shape"
        ));
    };
    let reason = scval_symbol(reason.clone()).context("reputation event reason is not a symbol")?;
    let timestamp = DateTime::parse_from_rfc3339(&event.ledger_closed_at)?.with_timezone(&Utc);
    let inserted = sqlx::query(
        "INSERT INTO reputation_history \
         (event_id, contributor_address, ledger, timestamp, old_value, new_value, reason) \
         VALUES ($1, $2, $3, $4, $5, $6, $7) ON CONFLICT (event_id) DO NOTHING",
    )
    .bind(&event.paging_token)
    .bind(address)
    .bind(i64::from(event.ledger))
    .bind(timestamp)
    .bind(i64::from(*old_value))
    .bind(i64::from(*new_value))
    .bind(reason)
    .execute(pool)
    .await?
    .rows_affected()
        > 0;
    Ok(inserted)
}

fn decode_scval(encoded: &str) -> Result<ScVal> {
    ScVal::from_xdr_base64(encoded, Limits::none()).map_err(Into::into)
}

fn scval_symbol(value: ScVal) -> Option<String> {
    match value {
        ScVal::Symbol(symbol) => String::from_utf8(symbol.into()).ok(),
        _ => None,
    }
}

fn scval_address(value: ScVal) -> Option<String> {
    match value {
        ScVal::Address(ScAddress::Account(account)) => match account.0 {
            stellar_xdr::PublicKey::PublicKeyTypeEd25519(key) => Some(
                stellar_strkey::ed25519::PublicKey(key.0)
                    .to_string()
                    .to_string(),
            ),
        },
        ScVal::Address(ScAddress::Contract(contract)) => Some(
            stellar_strkey::Contract(contract.0 .0)
                .to_string()
                .to_string(),
        ),
        _ => None,
    }
}
