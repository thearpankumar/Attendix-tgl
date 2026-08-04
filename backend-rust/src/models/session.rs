use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: Uuid,
    pub location_id: Uuid,
    pub batch_id: Option<Uuid>,
    pub token_hash: String,
    pub token_prefix: String,
    pub description: Option<String>,
    pub created_by: Uuid,
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
    pub batch_id: Option<Uuid>,
    pub description: Option<String>,
    pub expires_at: DateTime<Utc>,
    pub created_by: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStats {
    pub total: i32,
    pub verified: i32,
    pub unverified: i32,
    pub flagged: i32,
}
