use std::sync::Arc;

use crate::{error::Result, AppState};

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
    // GROUP BY (rather than a plain aggregate) so zero matching rows yields zero
    // result rows, matching the original Mongo `$match` + `$group` pipeline's
    // "no groups produced" behavior instead of SQL's usual "one row of NULLs".
    let row: Option<(i64, i64, i64)> = sqlx::query_as(
        "SELECT SUM(successful_submissions), SUM(failed_submissions), SUM(spoofing_attempts) \
         FROM devices WHERE fingerprint_hash = $1 GROUP BY fingerprint_hash",
    )
    .bind(fingerprint_hash)
    .fetch_optional(&state.db)
    .await?;

    Ok(row.map(|(successful, failed, spoofing)| DeviceTrustScore {
        score: BASE_SCORE,
        successful_submissions: successful as i32,
        failed_submissions: failed as i32,
        spoofing_attempts: spoofing as i32,
    }))
}

pub async fn update_device_trust(
    state: &Arc<AppState>,
    fingerprint_hash: &str,
    success: bool,
    spoofing_detected: bool,
) -> Result<()> {
    // Note: the original Mongo `update_one` touched a single, arbitrarily-chosen
    // matching document. Postgres has no equivalent "touch just one" semantics for
    // a non-unique WHERE clause without an explicit row lock/limit, so this updates
    // every `devices` row sharing this fingerprint_hash — a deliberate, more
    // predictable behavior for a device-wide trust counter.
    if success {
        sqlx::query(
            "UPDATE devices SET successful_submissions = successful_submissions + 1 WHERE fingerprint_hash = $1",
        )
        .bind(fingerprint_hash)
        .execute(&state.db)
        .await?;
    } else if spoofing_detected {
        sqlx::query(
            "UPDATE devices SET spoofing_attempts = spoofing_attempts + 1, failed_submissions = failed_submissions + 1 WHERE fingerprint_hash = $1",
        )
        .bind(fingerprint_hash)
        .execute(&state.db)
        .await?;
    } else {
        sqlx::query(
            "UPDATE devices SET failed_submissions = failed_submissions + 1 WHERE fingerprint_hash = $1",
        )
        .bind(fingerprint_hash)
        .execute(&state.db)
        .await?;
    }

    Ok(())
}

pub async fn flag_suspicious_device(
    state: &Arc<AppState>,
    fingerprint_hash: &str,
    reason: &str,
) -> Result<()> {
    sqlx::query(
        "UPDATE devices SET is_blocked = true, block_reason = $1, blocked_at = now() WHERE fingerprint_hash = $2",
    )
    .bind(reason)
    .bind(fingerprint_hash)
    .execute(&state.db)
    .await?;

    Ok(())
}
