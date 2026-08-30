use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// One *active* row per (session, roll_number) — the device currently
/// allowed to report telemetry for that student in that session. Lives on
/// the main database (see migrations/0006), with a real FK to `sessions`.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct SessionDeviceLock {
    pub id: Uuid,
    pub session_id: Uuid,
    pub roll_number: String,
    pub extension_instance_id: Uuid,
    pub webauthn_credential_id: Option<Uuid>,
    pub device_fingerprint_hash: Option<String>,
    pub locked_at: DateTime<Utc>,
    pub released_at: Option<DateTime<Utc>>,
    pub status: String,
    pub superseded_by: Option<Uuid>,
    /// The `jti` of the currently-valid signed telemetry token for this
    /// lock, if one has been issued — used to blacklist it the moment this
    /// lock is superseded by a re-pair (see
    /// controllers::extension_pairing::finish_pairing).
    pub telemetry_token_jti: Option<String>,
    pub telemetry_token_expires_at: Option<DateTime<Utc>>,
}

impl SessionDeviceLock {
    pub fn table_name() -> &'static str {
        "session_device_locks"
    }

    pub fn is_active(&self) -> bool {
        self.status == "active"
    }
}
