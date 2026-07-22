use crate::constants::{WEBAUTHN_AUTHENTICATOR_TYPE_MULTI, WEBAUTHN_DEVICE_UNKNOWN};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use chrono::{DateTime, Utc};
use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebAuthnCredential {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub student_id: String,
    pub credential_id: String,
    #[serde(
        serialize_with = "serialize_bytes",
        deserialize_with = "deserialize_bytes"
    )]
    pub public_key: Vec<u8>,
    #[serde(default)]
    pub counter: u32,
    #[serde(default = "default_device_label")]
    pub device_label: String,
    #[serde(default = "default_device_type")]
    pub device_type: String,
    #[serde(default)]
    pub transports: Vec<String>,
    #[serde(with = "bson::serde_helpers::datetime::FromChrono04DateTime")]
    pub enrolled_at: DateTime<Utc>,
    pub enrolled_ip_address: Option<String>,
    pub enrolled_user_agent: Option<String>,
    pub created_by_admin_id: Option<ObjectId>,
    #[serde(default)]
    pub sign_count: u32,
    pub last_used_at: Option<DateTime<Utc>>,
    pub last_session_id: Option<ObjectId>,
    #[serde(default)]
    pub is_suspended: bool,
    pub suspended_reason: Option<String>,
    pub suspended_at: Option<DateTime<Utc>>,
    pub suspended_by: Option<ObjectId>,
    pub aaguid: Option<String>,
    pub reset_at: Option<DateTime<Utc>>,
    pub reset_by: Option<ObjectId>,
}

fn default_device_label() -> String {
    WEBAUTHN_DEVICE_UNKNOWN.to_string()
}

fn default_device_type() -> String {
    WEBAUTHN_AUTHENTICATOR_TYPE_MULTI.to_string()
}

impl WebAuthnCredential {
    pub fn collection_name() -> &'static str {
        "webauthncredentials"
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
