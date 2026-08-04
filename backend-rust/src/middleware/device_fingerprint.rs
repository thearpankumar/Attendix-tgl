use chrono::Utc;
use std::sync::Arc;
use uuid::Uuid;

use crate::error::Result;
use crate::models::{DeviceFingerprint, UserAgentEntry};

pub async fn check_device_blocked(
    state: &Arc<crate::AppState>,
    fingerprint_id: &str,
) -> Result<Option<(bool, Option<String>)>> {
    if fingerprint_id.is_empty() || state.config.node_env == "test" {
        return Ok(None);
    }

    let row: Option<(bool, Option<String>)> = sqlx::query_as(
        "SELECT is_blocked, block_reason FROM device_fingerprints WHERE fingerprint_id = $1",
    )
    .bind(fingerprint_id)
    .fetch_optional(&state.db)
    .await?;

    Ok(row)
}

pub async fn record_device_success(
    state: &Arc<crate::AppState>,
    fingerprint_id: &str,
    session_id: Uuid,
    roll_number: &str,
    user_agent: &str,
) -> Result<()> {
    if fingerprint_id.is_empty() || state.config.node_env == "test" {
        return Ok(());
    }

    let existing = sqlx::query_as::<_, DeviceFingerprint>(
        "SELECT * FROM device_fingerprints WHERE fingerprint_id = $1",
    )
    .bind(fingerprint_id)
    .fetch_optional(&state.db)
    .await?;

    let mut device = existing.unwrap_or_else(|| DeviceFingerprint::new(fingerprint_id.to_string()));

    device.record_successful_verification(session_id, roll_number.to_string());

    add_user_agent(&mut device, user_agent);

    // Upsert: on first sight this inserts the full row; on subsequent visits it only
    // touches the same fields the original Mongo `$set` touched, leaving is_blocked /
    // spoofing_attempts / block_reason / last_spoofing_reason untouched.
    sqlx::query(
        "INSERT INTO device_fingerprints \
            (id, fingerprint_id, first_seen, last_seen, verification_failures, spoofing_attempts, \
             last_spoofing_reason, inconsistencies, claimed_device_types, user_agents_seen, sessions, \
             is_trusted, is_blocked, block_reason, last_metrics) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15) \
         ON CONFLICT (fingerprint_id) DO UPDATE SET \
            verification_failures = EXCLUDED.verification_failures, \
            sessions = EXCLUDED.sessions, \
            is_trusted = EXCLUDED.is_trusted, \
            user_agents_seen = EXCLUDED.user_agents_seen, \
            last_seen = EXCLUDED.last_seen",
    )
    .bind(device.id)
    .bind(&device.fingerprint_id)
    .bind(device.first_seen)
    .bind(device.last_seen)
    .bind(device.verification_failures)
    .bind(device.spoofing_attempts)
    .bind(&device.last_spoofing_reason)
    .bind(&device.inconsistencies)
    .bind(&device.claimed_device_types)
    .bind(&device.user_agents_seen)
    .bind(&device.sessions)
    .bind(device.is_trusted)
    .bind(device.is_blocked)
    .bind(&device.block_reason)
    .bind(&device.last_metrics)
    .execute(&state.db)
    .await?;

    Ok(())
}

fn add_user_agent(device: &mut DeviceFingerprint, user_agent: &str) {
    let now = Utc::now();

    if let Some(existing) = device
        .user_agents_seen
        .0
        .iter_mut()
        .find(|u| u.ua == user_agent)
    {
        existing.last_seen = now;
    } else {
        if device.user_agents_seen.0.len() >= 20 {
            device.user_agents_seen.0.remove(0);
        }
        device.user_agents_seen.0.push(UserAgentEntry {
            ua: user_agent.to_string(),
            first_seen: now,
            last_seen: now,
        });
    }
}
