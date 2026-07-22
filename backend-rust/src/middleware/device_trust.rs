use mongodb::{bson::doc, Collection};
use std::sync::Arc;

use crate::{error::Result, models::Device, AppState};

const BASE_SCORE: i32 = 50;
const SUCCESS_BONUS: i32 = 5;
const MAX_SUCCESS_BONUS: i32 = 40;
const FAIL_PENALTY: i32 = 10;
const MAX_FAIL_PENALTY: i32 = 45;
const SPOOFING_PENALTY: i32 = 15;

#[derive(Debug, Clone)]
pub struct DeviceTrustScore {
    pub score: i32,
    pub successful_submissions: i32,
    pub failed_submissions: i32,
    pub spoofing_attempts: i32,
}

impl DeviceTrustScore {
    pub fn calculate(&self) -> i32 {
        let mut score = BASE_SCORE;
        score += (self.successful_submissions * SUCCESS_BONUS).min(MAX_SUCCESS_BONUS);
        score -= (self.failed_submissions * FAIL_PENALTY).min(MAX_FAIL_PENALTY);
        score -= (self.spoofing_attempts * SPOOFING_PENALTY).min(MAX_FAIL_PENALTY);
        score.clamp(0, 100)
    }
}

pub async fn get_device_trust_score(
    state: &Arc<AppState>,
    fingerprint_hash: &str,
) -> Result<Option<DeviceTrustScore>> {
    let devices: Collection<Device> = state.database().collection(Device::collection_name());

    let filter = doc! { "fingerprintHash": fingerprint_hash };

    let pipeline = vec![
        doc! { "$match": filter },
        doc! {
            "$group": {
                "_id": "$fingerprintHash",
                "successfulSubmissions": { "$sum": "$successfulSubmissions" },
                "failedSubmissions": { "$sum": "$failedSubmissions" },
                "spoofingAttempts": { "$sum": "$spoofingAttempts" },
            }
        },
    ];

    let mut cursor = devices.aggregate(pipeline).await?;

    if cursor.advance().await? {
        let doc = cursor.deserialize_current()?;
        Ok(Some(DeviceTrustScore {
            score: BASE_SCORE,
            successful_submissions: doc.get_i32("successfulSubmissions").unwrap_or(0),
            failed_submissions: doc.get_i32("failedSubmissions").unwrap_or(0),
            spoofing_attempts: doc.get_i32("spoofingAttempts").unwrap_or(0),
        }))
    } else {
        Ok(None)
    }
}

pub async fn update_device_trust(
    state: &Arc<AppState>,
    fingerprint_hash: &str,
    success: bool,
    spoofing_detected: bool,
) -> Result<()> {
    let devices: Collection<Device> = state.database().collection(Device::collection_name());

    let update = if success {
        doc! { "$inc": { "successfulSubmissions": 1 } }
    } else if spoofing_detected {
        doc! { "$inc": { "spoofingAttempts": 1, "failedSubmissions": 1 } }
    } else {
        doc! { "$inc": { "failedSubmissions": 1 } }
    };

    devices
        .update_one(doc! { "fingerprintHash": fingerprint_hash }, update)
        .await?;

    Ok(())
}

pub async fn flag_suspicious_device(
    state: &Arc<AppState>,
    fingerprint_hash: &str,
    reason: &str,
) -> Result<()> {
    let devices: Collection<Device> = state.database().collection(Device::collection_name());

    devices
        .update_one(
            doc! { "fingerprintHash": fingerprint_hash },
            doc! {
                "$set": {
                    "isBlocked": true,
                    "blockReason": reason,
                    "blockedAt": mongodb::bson::DateTime::now(),
                }
            },
        )
        .await?;

    Ok(())
}
