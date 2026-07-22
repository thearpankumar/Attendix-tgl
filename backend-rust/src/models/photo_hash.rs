use chrono::{DateTime, Utc};
use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhotoHash {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub roll_number: String,
    pub photo_hash: String,
    pub session_id: ObjectId,
    #[serde(with = "bson::serde_helpers::datetime::FromChrono04DateTime")]
    pub captured_at: DateTime<Utc>,
    pub confidence: Option<f64>,
}

impl PhotoHash {
    pub fn collection_name() -> &'static str {
        "photo_hashes"
    }
}
