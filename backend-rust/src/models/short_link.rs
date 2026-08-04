use chrono::{DateTime, Utc};
use rand::RngExt;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ShortLink {
    pub id: Uuid,
    pub short_code: String,
    pub session_id: Option<Uuid>,
    pub created_by: Uuid,
    #[serde(default = "default_true")]
    pub is_active: bool,
    pub expires_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub click_count: i32,
    pub last_clicked_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

fn default_true() -> bool {
    true
}

impl ShortLink {
    pub fn table_name() -> &'static str {
        "short_links"
    }

    pub fn generate_short_code(length: usize) -> String {
        let chars = "abcdefghijkmnpqrstuvwxyz23456789";
        let mut rng = rand::rng();
        (0..length)
            .map(|_| chars.chars().nth(rng.random_range(0..chars.len())).unwrap())
            .collect()
    }

    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            return expires_at <= Utc::now();
        }
        false
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortLinkCreate {
    pub session_id: Option<Uuid>,
    pub expires_at: Option<DateTime<Utc>>,
}
