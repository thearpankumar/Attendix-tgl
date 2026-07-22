use chrono::Utc;
use mongodb::bson::{doc, oid::ObjectId, Bson, DateTime as BsonDateTime};
use mongodb::Collection;
use std::sync::Arc;

use crate::error::Result;
use crate::models::{DeviceFingerprint, UserAgentEntry};

fn to_bson_datetime(dt: chrono::DateTime<Utc>) -> BsonDateTime {
    BsonDateTime::from_millis(dt.timestamp_millis())
}

pub async fn check_device_blocked(
    state: &Arc<crate::AppState>,
    fingerprint_id: &str,
) -> Result<Option<(bool, Option<String>)>> {
    if fingerprint_id.is_empty() || state.config.node_env == "test" {
        return Ok(None);
    }

    let db_name = state
        .config
        .mongodb_uri
        .split('/')
        .next_back()
        .unwrap_or("default");

    let collection: Collection<DeviceFingerprint> = state
        .db
        .database(db_name)
        .collection(DeviceFingerprint::collection_name());

    let device = collection
        .find_one(doc! { "fingerprintId": fingerprint_id })
        .await?;

    Ok(device.map(|d| (d.is_blocked, d.block_reason)))
}

pub async fn record_device_success(
    state: &Arc<crate::AppState>,
    fingerprint_id: &str,
    session_id: ObjectId,
    roll_number: &str,
    user_agent: &str,
) -> Result<()> {
    if fingerprint_id.is_empty() || state.config.node_env == "test" {
        return Ok(());
    }

    let db_name = state
        .config
        .mongodb_uri
        .split('/')
        .next_back()
        .unwrap_or("default");

    let collection: Collection<DeviceFingerprint> = state
        .db
        .database(db_name)
        .collection(DeviceFingerprint::collection_name());

    let device = collection
        .find_one(doc! { "fingerprintId": fingerprint_id })
        .await?;

    let mut device = if let Some(d) = device {
        d
    } else {
        DeviceFingerprint::new(fingerprint_id.to_string())
    };

    device.record_successful_verification(session_id, roll_number.to_string());

    add_user_agent(&mut device, user_agent);

    let sessions_bson = Bson::Array(
        device
            .sessions
            .iter()
            .map(|s| {
                doc! {
                    "sessionId": s.session_id,
                    "rollNumber": &s.roll_number,
                    "timestamp": to_bson_datetime(s.timestamp),
                    "wasSuccessful": s.was_successful,
                }
            })
            .map(Bson::Document)
            .collect(),
    );

    let ua_bson = Bson::Array(
        device
            .user_agents_seen
            .iter()
            .map(|u| {
                doc! {
                    "ua": &u.ua,
                    "firstSeen": to_bson_datetime(u.first_seen),
                    "lastSeen": to_bson_datetime(u.last_seen),
                }
            })
            .map(Bson::Document)
            .collect(),
    );

    collection
        .update_one(
            doc! { "fingerprintId": fingerprint_id },
            doc! { "$set": {
                "verificationFailures": device.verification_failures,
                "sessions": sessions_bson,
                "isTrusted": device.is_trusted,
                "userAgentsSeen": ua_bson,
                "lastSeen": to_bson_datetime(device.last_seen),
            }},
        )
        .upsert(true)
        .await?;

    Ok(())
}

fn add_user_agent(device: &mut DeviceFingerprint, user_agent: &str) {
    let now = Utc::now();

    if let Some(existing) = device
        .user_agents_seen
        .iter_mut()
        .find(|u| u.ua == user_agent)
    {
        existing.last_seen = now;
    } else {
        if device.user_agents_seen.len() >= 20 {
            device.user_agents_seen.remove(0);
        }
        device.user_agents_seen.push(UserAgentEntry {
            ua: user_agent.to_string(),
            first_seen: now,
            last_seen: now,
        });
    }
}
