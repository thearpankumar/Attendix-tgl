use crate::constants::{WEBAUTHN_AUTHENTICATOR_TYPE_MULTI, WEBAUTHN_DEVICE_UNKNOWN};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

fn serialize_bytes<S>(bytes: &[u8], serializer: S) -> std::result::Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    BASE64.encode(bytes).serialize(serializer)
}

fn deserialize_bytes<'de, D>(deserializer: D) -> std::result::Result<Vec<u8>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    BASE64.decode(s).map_err(serde::de::Error::custom)
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct WebAuthnCredential {
    pub id: Uuid,
    pub student_id: String,
    pub credential_id: String,
    #[serde(
        serialize_with = "serialize_bytes",
        deserialize_with = "deserialize_bytes"
    )]
    pub public_key: Vec<u8>,
    /// WebAuthn signature counters are u32 on the wire; stored as i64 since
    /// Postgres has no unsigned integer type.
    #[serde(default)]
    pub counter: i64,
    #[serde(default = "default_device_label")]
    pub device_label: String,
    #[serde(default = "default_device_type")]
    pub device_type: String,
    #[serde(default)]
    pub transports: Vec<String>,
    pub enrolled_at: DateTime<Utc>,
    pub enrolled_ip_address: Option<String>,
    pub enrolled_user_agent: Option<String>,
    pub created_by_admin_id: Option<Uuid>,
    #[serde(default)]
    pub sign_count: i64,
    pub last_used_at: Option<DateTime<Utc>>,
    pub last_session_id: Option<Uuid>,
    #[serde(default)]
    pub is_suspended: bool,
    pub suspended_reason: Option<String>,
    pub suspended_at: Option<DateTime<Utc>>,
    pub suspended_by: Option<Uuid>,
    pub aaguid: Option<String>,
    pub reset_at: Option<DateTime<Utc>>,
    pub reset_by: Option<Uuid>,
}

fn default_device_label() -> String {
    WEBAUTHN_DEVICE_UNKNOWN.to_string()
}

fn default_device_type() -> String {
    WEBAUTHN_AUTHENTICATOR_TYPE_MULTI.to_string()
}

impl WebAuthnCredential {
    pub fn table_name() -> &'static str {
        "webauthn_credentials"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WebAuthnDeviceTypeEnum {
    #[serde(rename = "single_device")]
    SingleDevice,
    #[serde(rename = "singleDevice")]
    SingleDeviceAlt,
    #[serde(rename = "multi_device")]
    MultiDevice,
    #[serde(rename = "multiDevice")]
    MultiDeviceAlt,
}
