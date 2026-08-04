use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Flag {
    pub id: Uuid,
    pub flag_type: String,
    pub admin_id: Option<Uuid>,
    pub student_id: Option<Uuid>,
    pub details: Option<String>,
    pub session_id: Option<Uuid>,
    pub timestamp: DateTime<Utc>,
    pub resolved: bool,
    pub resolved_by: Option<Uuid>,
    pub resolved_at: Option<DateTime<Utc>>,
}

impl Flag {
    pub fn table_name() -> &'static str {
        "flags"
    }
}
