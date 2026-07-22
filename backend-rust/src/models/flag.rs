use chrono::{DateTime, Utc};
use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Flag {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub flag_type: String,
    pub admin_id: Option<ObjectId>,
    pub student_id: Option<ObjectId>,
    pub details: Option<String>,
    pub session_id: Option<ObjectId>,
    pub timestamp: DateTime<Utc>,
    pub resolved: bool,
    pub resolved_by: Option<ObjectId>,
    pub resolved_at: Option<DateTime<Utc>>,
}

impl Flag {
    pub fn collection_name() -> &'static str {
        "flags"
    }
}
