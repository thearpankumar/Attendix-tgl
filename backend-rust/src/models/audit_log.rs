use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::error::Result;

/// Hash used as `previous_hash` for the very first row in the chain.
const GENESIS_HASH: &str = "GENESIS";

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AuditLogRow {
    pub seq: i64,
    pub id: Uuid,
    pub admin_id: Option<Uuid>,
    pub event: String,
    pub detail: serde_json::Value,
    pub ip_address: Option<String>,
    pub created_at: DateTime<Utc>,
    pub previous_hash: String,
    pub entry_hash: String,
}

fn compute_entry_hash(
    previous_hash: &str,
    id: &Uuid,
    admin_id: Option<Uuid>,
    event: &str,
    detail: &serde_json::Value,
    ip_address: Option<&str>,
    created_at: &DateTime<Utc>,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(previous_hash.as_bytes());
    hasher.update(id.as_bytes());
    hasher.update(
        admin_id
            .map(|a| a.to_string())
            .unwrap_or_default()
            .as_bytes(),
    );
    hasher.update(event.as_bytes());
    hasher.update(detail.to_string().as_bytes());
    hasher.update(ip_address.unwrap_or("").as_bytes());
    hasher.update(created_at.to_rfc3339().as_bytes());
    hex::encode(hasher.finalize())
}

/// Appends a hash-chained audit log entry. Each row's `entry_hash` covers its
/// own fields plus the previous row's `entry_hash`, so an attacker with
/// direct database access (bypassing the application entirely) who edits or
/// deletes a row breaks the chain from that point forward — detectable by
/// `verify_chain` without needing a separate, independently-secured log
/// store.
///
/// A Postgres advisory transaction lock serializes concurrent appends so two
/// simultaneous writers can't both read the same `previous_hash` and produce
/// two rows claiming the same predecessor.
pub async fn record_audit_event(
    pool: &sqlx::PgPool,
    admin_id: Option<Uuid>,
    event: &str,
    detail: serde_json::Value,
    ip_address: Option<&str>,
) -> Result<()> {
    let mut tx = pool.begin().await?;

    sqlx::query("SELECT pg_advisory_xact_lock(hashtext('admin_audit_log_chain'))")
        .execute(&mut *tx)
        .await?;

    let previous_hash: String =
        sqlx::query_scalar("SELECT entry_hash FROM admin_audit_log ORDER BY seq DESC LIMIT 1")
            .fetch_optional(&mut *tx)
            .await?
            .unwrap_or_else(|| GENESIS_HASH.to_string());

    let id = Uuid::new_v4();
    // `Utc::now()` carries nanosecond precision, but Postgres `TIMESTAMPTZ`
    // only stores microseconds — hashing the untruncated value here would
    // make `verify_chain`'s recomputation (using the value read back from
    // the DB, already rounded to microseconds) never match this row's
    // `entry_hash`, so every row would appear tampered. Truncate up front so
    // the hashed value is bit-identical to what a later read-back produces.
    let created_at = DateTime::<Utc>::from_timestamp_micros(Utc::now().timestamp_micros())
        .unwrap_or_else(Utc::now);
    let entry_hash = compute_entry_hash(
        &previous_hash,
        &id,
        admin_id,
        event,
        &detail,
        ip_address,
        &created_at,
    );

    sqlx::query(
        "INSERT INTO admin_audit_log \
         (id, admin_id, event, detail, ip_address, created_at, previous_hash, entry_hash) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(id)
    .bind(admin_id)
    .bind(event)
    .bind(&detail)
    .bind(ip_address)
    .bind(created_at)
    .bind(&previous_hash)
    .bind(&entry_hash)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}

/// Walks the chain in `seq` order and recomputes every hash, confirming each
/// row's `previous_hash` matches its predecessor's `entry_hash` and each
/// row's own `entry_hash` matches its recomputed content hash. Returns the
/// `seq` of the first row where the chain breaks, if any — a break means
/// something between the first row and that point was altered or removed
/// outside the application.
pub async fn verify_chain(pool: &sqlx::PgPool) -> Result<Option<i64>> {
    let rows: Vec<AuditLogRow> = sqlx::query_as("SELECT * FROM admin_audit_log ORDER BY seq ASC")
        .fetch_all(pool)
        .await?;

    let mut expected_previous = GENESIS_HASH.to_string();
    for row in &rows {
        if row.previous_hash != expected_previous {
            return Ok(Some(row.seq));
        }
        let recomputed = compute_entry_hash(
            &row.previous_hash,
            &row.id,
            row.admin_id,
            &row.event,
            &row.detail,
            row.ip_address.as_deref(),
            &row.created_at,
        );
        if recomputed != row.entry_hash {
            return Ok(Some(row.seq));
        }
        expected_previous = row.entry_hash.clone();
    }
    Ok(None)
}
