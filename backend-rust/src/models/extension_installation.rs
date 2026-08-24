use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The durable "which physical install is this" anchor for the (Phase 3)
/// browser extension: a UUID it generates once and persists in its own
/// storage. Lives on the main database (see migrations/0006) — it's small,
/// low-write-volume, relational state, not telemetry.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionInstallation {
    pub id: Uuid,
    pub roll_number: String,
    pub extension_instance_id: Uuid,
    pub first_seen_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub browser: Option<String>,
    pub os: Option<String>,
    #[serde(default = "default_true")]
    pub is_active: bool,
}

fn default_true() -> bool {
    true
}

impl ExtensionInstallation {
    pub fn table_name() -> &'static str {
        "extension_installations"
    }
}
