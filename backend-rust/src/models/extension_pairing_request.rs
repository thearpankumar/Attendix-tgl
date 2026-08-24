use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A short-lived request to pair one browser-extension install to a session
/// via the existing WebAuthn ceremony (see
/// `controllers::extension_pairing`). Lives on the main database (see
/// migrations/0007_extension_pairing).
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionPairingRequest {
    pub id: Uuid,
    pub session_id: Uuid,
    pub extension_instance_id: Uuid,
    pub pairing_code: String,
    pub status: String,
    pub roll_number: Option<String>,
    pub webauthn_credential_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

impl ExtensionPairingRequest {
    pub fn table_name() -> &'static str {
        "extension_pairing_requests"
    }

    pub fn is_expired(&self) -> bool {
        self.expires_at <= Utc::now()
    }
}
