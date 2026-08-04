use crate::models::text_enum_sqlx;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct WebAuthnReenrollmentLog {
    pub id: Uuid,
    pub student_id: String,
    pub admin_id: Uuid,
    pub reason: Option<String>,
    pub previous_credential_id: Option<String>,
    pub new_credential_id: Option<String>,
    pub action_type: WebAuthnReenrollmentAction,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WebAuthnReenrollmentAction {
    #[serde(rename = "reset")]
    Reset,
    #[serde(rename = "suspend")]
    Suspend,
    #[serde(rename = "unsuspend")]
    Unsuspend,
}
text_enum_sqlx!(WebAuthnReenrollmentAction);

impl WebAuthnReenrollmentLog {
    pub fn table_name() -> &'static str {
        "webauthn_reenrollment_logs"
    }
}
