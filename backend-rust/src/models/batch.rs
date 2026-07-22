use chrono::{DateTime, Utc};
use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Batch {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub name: String,
    pub description: Option<String>,
    #[serde(default)]
    pub students: Vec<Student>,
    pub created_by: ObjectId,
    #[serde(with = "bson::serde_helpers::datetime::FromChrono04DateTime")]
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Student {
    pub name: String,
    pub roll_number: String,
    pub college_name: Option<String>,
    pub email: Option<String>,
}

impl Batch {
    pub fn collection_name() -> &'static str {
        "batches"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchCreate {
    pub name: String,
    pub description: Option<String>,
    pub students: Vec<Student>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchUpdate {
    pub name: Option<String>,
    pub description: Option<String>,
    pub students: Option<Vec<Student>>,
}
