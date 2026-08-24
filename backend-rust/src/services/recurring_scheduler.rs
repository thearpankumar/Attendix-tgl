//! Multi-container-safe generation of sessions from recurring ("cron job")
//! rules.
//!
//! Every backend replica runs an identical copy of this loop -- there is no
//! leader election. Safety instead comes from claiming one due rule at a
//! time via `SELECT ... FOR UPDATE SKIP LOCKED` inside a single Postgres
//! transaction that also creates the new `sessions` row, re-points the
//! rule's locked short link, and advances `next_run_at`. `FOR UPDATE SKIP
//! LOCKED` makes a claimed row invisible to every other replica's
//! concurrently-running claim query until this transaction commits or rolls
//! back, and Postgres releases the lock automatically if a replica dies
//! mid-transaction -- no lock TTL/expiry bookkeeping needed, unlike a
//! Redis-based distributed lock. Because the claim and the schedule-advance
//! share one transaction, a crash between any of the steps rolls back
//! cleanly and the next tick (on any replica) retries from scratch.
//!
//! Missed occurrences (e.g. the whole fleet was down at the scheduled time)
//! are skipped, not backfilled: `RecurringSessionRule::next_occurrence_after`
//! always computes forward from "now", never from the rule's original
//! schedule.

use std::time::Duration;

use chrono::Utc;
use uuid::Uuid;

use crate::controllers::{create_session_row, SessionSpec, ShortlinkDirective};
use crate::error::Result;
use crate::models::{RecurringSessionRule, RECURRING_RULE_MAX_CONSECUTIVE_FAILURES};

const TICK_INTERVAL_SECS: u64 = 30;

pub fn spawn_recurring_session_scheduler(pool: sqlx::PgPool) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(TICK_INTERVAL_SECS));
        loop {
            interval.tick().await;
            // Drain every rule that's due this tick, not just one, so a
            // burst of same-minute rules doesn't wait multiple ticks to
            // all fire.
            loop {
                match claim_and_generate_one(&pool).await {
                    Ok(true) => continue,
                    Ok(false) => break,
                    Err(e) => {
                        tracing::warn!("Recurring session scheduler pass failed: {}", e);
                        break;
                    }
                }
            }
        }
    });
}

/// Claims and generates at most one due recurring rule. Returns `Ok(true)`
/// if a rule was claimed (whether or not generation itself succeeded --
/// callers should keep draining), `Ok(false)` if no rule was due.
///
/// `pub` (rather than module-private) specifically so integration tests can
/// call it directly from multiple concurrent pools to verify the
/// multi-replica claim guarantee -- see
/// `tests/security/recurring_scheduler_tests.rs`.
pub async fn claim_and_generate_one(pool: &sqlx::PgPool) -> Result<bool> {
    let mut tx = pool.begin().await?;

    let rule: Option<RecurringSessionRule> = sqlx::query_as(
        "SELECT * FROM recurring_session_rules \
         WHERE is_active = true AND next_run_at <= now() \
         ORDER BY next_run_at \
         LIMIT 1 \
         FOR UPDATE SKIP LOCKED",
    )
    .fetch_optional(&mut *tx)
    .await?;

    let Some(rule) = rule else {
        tx.rollback().await?;
        return Ok(false);
    };

    let now = Utc::now();
    let rule_id = rule.id;

    match generate_occurrence(&mut tx, &rule, now).await {
        Ok(()) => {
            tx.commit().await?;
        }
        Err(e) => {
            // Roll back the partial attempt entirely (the session insert,
            // if it got that far), then record the failure against the rule
            // in its own short transaction -- outside the failed one -- so
            // a persistently broken rule (e.g. its location was deleted)
            // doesn't spin every tick forever.
            tx.rollback().await?;
            tracing::warn!("Recurring rule {} failed to generate: {}", rule_id, e);
            record_generation_failure(pool, rule_id, &e.to_string()).await?;
        }
    }

    Ok(true)
}

async fn generate_occurrence(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    rule: &RecurringSessionRule,
    now: chrono::DateTime<Utc>,
) -> Result<()> {
    let expires_at = now + chrono::Duration::minutes(rule.duration_minutes as i64);
    // Mirrors how `expires_at` is derived above, from the rule's own
    // `class_duration_minutes` rather than its attendance-window
    // `duration_minutes` -- see migration 0005_session_monitoring.
    let monitoring_ends_at = rule
        .class_duration_minutes
        .filter(|_| rule.monitoring_enabled)
        .map(|minutes| now + chrono::Duration::minutes(minutes as i64));

    let shortlink = match &rule.locked_short_code {
        Some(code) => ShortlinkDirective::Existing(code.clone()),
        None => ShortlinkDirective::None,
    };

    let (session, _short_code, _token) = create_session_row(
        tx,
        SessionSpec {
            location_id: rule.location_id,
            batch_id: rule.batch_id,
            description: rule.description.clone(),
            created_by: rule.created_by,
            college_name: None,
            starts_at: None,
            expires_at,
            recurring_rule_id: Some(rule.id),
            shortlink,
            monitoring_enabled: rule.monitoring_enabled,
            class_duration_minutes: rule.class_duration_minutes,
            monitoring_ends_at,
            session_kind: rule.session_kind.clone(),
        },
    )
    .await?;

    let next_run_at = rule
        .next_occurrence_after(now)
        .map_err(crate::error::AppError::Internal)?;

    sqlx::query(
        "UPDATE recurring_session_rules \
         SET next_run_at = $1, last_generated_at = now(), last_generated_session_id = $2, \
             consecutive_failures = 0, last_error = NULL, claimed_by = NULL, claimed_at = NULL \
         WHERE id = $3",
    )
    .bind(next_run_at)
    .bind(session.id)
    .bind(rule.id)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

async fn record_generation_failure(
    pool: &sqlx::PgPool,
    rule_id: Uuid,
    error_message: &str,
) -> Result<()> {
    sqlx::query(
        "UPDATE recurring_session_rules \
         SET consecutive_failures = consecutive_failures + 1, \
             last_error = $1, \
             is_active = CASE WHEN consecutive_failures + 1 >= $2 THEN false ELSE is_active END \
         WHERE id = $3",
    )
    .bind(error_message)
    .bind(RECURRING_RULE_MAX_CONSECUTIVE_FAILURES)
    .bind(rule_id)
    .execute(pool)
    .await?;
    Ok(())
}
