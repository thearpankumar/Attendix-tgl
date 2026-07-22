use chrono::{DateTime, Utc};
use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceFingerprint {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub fingerprint_id: String,
    #[serde(with = "bson::serde_helpers::datetime::FromChrono04DateTime")]
    pub first_seen: DateTime<Utc>,
    #[serde(with = "bson::serde_helpers::datetime::FromChrono04DateTime")]
    pub last_seen: DateTime<Utc>,
    #[serde(default)]
    pub verification_failures: i32,
    #[serde(default)]
    pub spoofing_attempts: i32,
    pub last_spoofing_reason: Option<String>,
    #[serde(default)]
    pub inconsistencies: Vec<String>,
    #[serde(default)]
    pub claimed_device_types: Vec<String>,
    #[serde(default)]
    pub user_agents_seen: Vec<UserAgentEntry>,
    #[serde(default)]
    pub sessions: Vec<DeviceSession>,
    #[serde(default)]
    pub is_trusted: bool,
    #[serde(default)]
    pub is_blocked: bool,
    pub block_reason: Option<String>,
    pub last_metrics: Option<DeviceMetrics>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserAgentEntry {
    pub ua: String,
    #[serde(with = "bson::serde_helpers::datetime::FromChrono04DateTime")]
    pub first_seen: DateTime<Utc>,
    #[serde(with = "bson::serde_helpers::datetime::FromChrono04DateTime")]
    pub last_seen: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceSession {
    pub session_id: ObjectId,
    pub roll_number: String,
    #[serde(with = "bson::serde_helpers::datetime::FromChrono04DateTime")]
    pub timestamp: DateTime<Utc>,
    pub was_successful: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceMetrics {
    pub max_touch_points: Option<i32>,
    pub hardware_concurrency: Option<i32>,
    pub device_memory: Option<i32>,
    pub webgl_renderer: Option<String>,
    pub screen_width: Option<i32>,
    pub screen_height: Option<i32>,
    pub platform: Option<String>,
}

impl DeviceFingerprint {
    pub fn collection_name() -> &'static str {
        "devicefingerprints"
    }

    pub fn new(fingerprint_id: String) -> Self {
        let now = Utc::now();
        Self {
            id: None,
            fingerprint_id,
            first_seen: now,
            last_seen: now,
            verification_failures: 0,
            spoofing_attempts: 0,
            last_spoofing_reason: None,
            inconsistencies: vec![],
            claimed_device_types: vec![],
            user_agents_seen: vec![],
            sessions: vec![],
            is_trusted: false,
            is_blocked: false,
            block_reason: None,
            last_metrics: None,
        }
    }

    pub fn record_verification_failure(&mut self, reason: Option<String>) {
        self.verification_failures += 1;
        self.last_seen = Utc::now();

        if let Some(r) = reason {
            self.last_spoofing_reason = Some(r);
            self.spoofing_attempts += 1;

            if !self.is_blocked && self.spoofing_attempts >= 5 {
                self.is_blocked = true;
                self.block_reason = Some(format!(
                    "Blocked after {} spoofing attempts",
                    self.spoofing_attempts
                ));
            }
        }
    }

    pub fn record_successful_verification(&mut self, session_id: ObjectId, roll_number: String) {
        self.last_seen = Utc::now();

        if self.verification_failures > 0 {
            self.verification_failures = (self.verification_failures - 1).max(0);
        }

        if self.sessions.len() >= 50 {
            self.sessions.remove(0);
        }

        self.sessions.push(DeviceSession {
            session_id,
            roll_number,
            timestamp: Utc::now(),
            was_successful: true,
        });

        let successful_count = self.sessions.iter().filter(|s| s.was_successful).count();
        if successful_count >= 3 && self.spoofing_attempts == 0 {
            self.is_trusted = true;
        }
    }

    pub fn increase_trust_score(&mut self) {
        self.spoofing_attempts = (self.spoofing_attempts - 1).max(0);
        self.verification_failures = (self.verification_failures - 1).max(0);

        if self.sessions.len() >= 3 && self.spoofing_attempts == 0 {
            self.is_trusted = true;
        }

        if self.is_blocked && self.spoofing_attempts < 5 {
            self.is_blocked = false;
            self.block_reason = None;
        }
    }
}
