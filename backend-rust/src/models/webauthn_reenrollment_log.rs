use chrono::{DateTime, Utc};
use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebAuthnReenrollmentLog {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub student_id: String,
    pub admin_id: ObjectId,
    pub reason: Option<String>,
    pub previous_credential_id: Option<String>,
    pub new_credential_id: Option<String>,
    pub action_type: WebAuthnReenrollmentAction,
    #[serde(with = "bson::serde_helpers::datetime::FromChrono04DateTime")]
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

impl WebAuthnReenrollmentLog {
    pub fn collection_name() -> &'static str {
        "webauthnreenrollmentlogs"
    }
}
