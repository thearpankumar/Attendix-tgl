use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct PhotoHash {
    pub id: Uuid,
    pub roll_number: String,
    pub photo_hash: String,
    pub session_id: Uuid,
    pub captured_at: DateTime<Utc>,
    pub confidence: Option<f64>,
}

impl PhotoHash {
    pub fn table_name() -> &'static str {
        "photo_hashes"
    }
}
