use chrono::{DateTime, Utc};
use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub location_id: ObjectId,
    pub batch_id: Option<ObjectId>,
    pub token_hash: String,
    pub token_prefix: String,
    pub description: Option<String>,
    pub created_by: ObjectId,
    #[serde(default = "default_true")]
    pub is_active: bool,
    #[serde(with = "bson::serde_helpers::datetime::FromChrono04DateTime")]
    pub expires_at: DateTime<Utc>,
    #[serde(default)]
    pub rotation_count: i32,
    pub totp_secret: Option<String>,
    #[serde(with = "bson::serde_helpers::datetime::FromChrono04DateTime")]
    pub created_at: DateTime<Utc>,
}

fn default_true() -> bool {
    true
}

impl Session {
    pub fn collection_name() -> &'static str {
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
    pub location_id: ObjectId,
    pub batch_id: Option<ObjectId>,
    pub description: Option<String>,
    pub expires_at: DateTime<Utc>,
    pub created_by: ObjectId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStats {
    pub total: i32,
    pub verified: i32,
    pub unverified: i32,
    pub flagged: i32,
}
