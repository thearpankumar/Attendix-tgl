use crate::constants::{
    FLAG_DEVICE_FINGERPRINT_CHANGE, FLAG_MULTI_STUDENT_DEVICE, FLAG_RAPID_SUBMISSION,
    FLAG_STUDENT_DEVICE_SWITCHED,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::types::Json;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    pub id: Uuid,
    pub fingerprint_hash: String,
    pub bound_to_student: Option<String>,
    pub session_id: Option<Uuid>,
    pub first_seen_at: DateTime<Utc>,
    pub last_seen_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub attendance_count: i32,
    #[serde(default)]
    pub flags: Json<Vec<DeviceFlagEntry>>,
    pub metadata: Option<Json<DeviceMetadata>>,
    // Trust scoring fields
    #[serde(default)]
    pub successful_submissions: i32,
    #[serde(default)]
    pub failed_submissions: i32,
    #[serde(default)]
    pub spoofing_attempts: i32,
    #[serde(default)]
    pub is_blocked: bool,
    pub block_reason: Option<String>,
    pub blocked_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceFlagEntry {
    #[serde(rename = "type")]
    pub flag_type: String,
    pub timestamp: DateTime<Utc>,
    pub details: Option<String>,
    pub session_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceMetadata {
    pub user_agent: Option<String>,
    pub platform: Option<String>,
    pub browser: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeviceFlagType {
    MultiStudentDevice,
    StudentDeviceSwitched,
    RapidSubmission,
    DeviceFingerprintChange,
}

impl Device {
    pub fn table_name() -> &'static str {
        "devices"
    }

    pub fn hash_fingerprint(fingerprint: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(fingerprint.as_bytes());
        hex::encode(hasher.finalize())
    }

    pub fn new(
        fingerprint_hash: String,
        bound_to_student: String,
        session_id: Uuid,
        user_agent: Option<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            fingerprint_hash,
            bound_to_student: Some(bound_to_student),
            session_id: Some(session_id),
            first_seen_at: Utc::now(),
            last_seen_at: Some(Utc::now()),
            attendance_count: 1,
            flags: Json(vec![]),
            metadata: Some(Json(DeviceMetadata {
                user_agent,
                platform: None,
                browser: None,
            })),
            successful_submissions: 0,
            failed_submissions: 0,
            spoofing_attempts: 0,
            is_blocked: false,
            block_reason: None,
            blocked_at: None,
        }
    }

    pub fn add_flag(
        &mut self,
        flag_type: DeviceFlagType,
        details: Option<String>,
        session_id: Option<Uuid>,
    ) {
        self.flags.0.push(DeviceFlagEntry {
            flag_type: match flag_type {
                DeviceFlagType::MultiStudentDevice => FLAG_MULTI_STUDENT_DEVICE.to_string(),
                DeviceFlagType::StudentDeviceSwitched => FLAG_STUDENT_DEVICE_SWITCHED.to_string(),
                DeviceFlagType::RapidSubmission => FLAG_RAPID_SUBMISSION.to_string(),
                DeviceFlagType::DeviceFingerprintChange => {
                    FLAG_DEVICE_FINGERPRINT_CHANGE.to_string()
                }
            },
            timestamp: Utc::now(),
            details,
            session_id,
        });
    }

    pub fn has_multi_student_flag(&self) -> bool {
        self.flags
            .0
            .iter()
            .any(|f| f.flag_type == FLAG_MULTI_STUDENT_DEVICE)
    }
}
