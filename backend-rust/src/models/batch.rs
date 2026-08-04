use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Batch {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
}

/// A batch row joined with its students, matching the JSON shape the API
/// previously returned for a Mongo `Batch` document with an embedded array.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchWithStudents {
    #[serde(flatten)]
    pub batch: Batch,
    pub students: Vec<Student>,
}

/// A persisted student row (own table, FK'd to `batches`).
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Student {
    pub id: Uuid,
    pub batch_id: Uuid,
    pub position: i32,
    pub name: String,
    pub roll_number: String,
    pub college_name: Option<String>,
    pub email: Option<String>,
}

impl Batch {
    pub fn table_name() -> &'static str {
        "batches"
    }
}

/// Student payload as submitted by the API (no id/batch_id/position — those are
/// assigned when the row is inserted).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StudentInput {
    pub name: String,
    pub roll_number: String,
    pub college_name: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchCreate {
    pub name: String,
    pub description: Option<String>,
    pub students: Vec<StudentInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchUpdate {
    pub name: Option<String>,
    pub description: Option<String>,
    pub students: Option<Vec<StudentInput>>,
}
