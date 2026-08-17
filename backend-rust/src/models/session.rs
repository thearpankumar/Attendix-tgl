use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: Uuid,
    pub location_id: Option<Uuid>,
    pub batch_id: Option<Uuid>,
    /// Set only for sessions created via the Excel-upload bulk creation flow
    /// (migration 0003) — mutually exclusive with `batch_id` (enforced by a
    /// DB check constraint). Tells roster/export code which table to
    /// resolve students from; see `controllers::roster_source`.
    pub excel_batch_id: Option<Uuid>,
    pub token_hash: String,
    pub token_prefix: String,
    pub description: Option<String>,
    pub created_by: Uuid,
    pub college_name: Option<String>,
    pub starts_at: Option<DateTime<Utc>>,
    #[serde(default = "default_true")]
    pub is_active: bool,
    pub expires_at: DateTime<Utc>,
    #[serde(default)]
    pub rotation_count: i32,
    pub totp_secret: Option<String>,
    pub created_at: DateTime<Utc>,
}

fn default_true() -> bool {
    true
}

impl Session {
    pub fn table_name() -> &'static str {
        "sessions"
    }

    pub fn generate_token() -> String {
        use rand::Rng;
        let mut rng = rand::rng();
        let mut bytes = [0u8; 16];
        rng.fill_bytes(&mut bytes);
        hex::encode(bytes)
    }

    pub fn hash_token(token: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(token.as_bytes());
        hex::encode(hasher.finalize())
    }

    pub fn is_expired(&self) -> bool {
        self.expires_at <= Utc::now()
    }

    pub fn get_token_prefix(token: &str) -> String {
        token.chars().take(8).collect()
    }

    pub fn generate_totp_secret() -> String {
        use rand::Rng;
        let mut rng = rand::rng();
        let mut bytes = [0u8; 32];
        rng.fill_bytes(&mut bytes);
        hex::encode(bytes)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionCreate {
    pub location_id: Uuid,
    pub batch_id: Uuid,
    pub description: Option<String>,
    pub expires_at: DateTime<Utc>,
    pub created_by: Uuid,
    pub assigned_admin_id: Uuid,
    pub college_name: String,
    pub starts_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStats {
    pub total: i32,
    pub verified: i32,
    pub unverified: i32,
    pub flagged: i32,
}
