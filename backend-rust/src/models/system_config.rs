use chrono::{DateTime, Utc};
use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};

use crate::constants::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemConfig {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    #[serde(default)]
    pub dev_bypass_enabled: bool,
    pub gps_validation: GpsValidationConfig,
    pub emulator_detection: EmulatorDetectionConfig,
    pub trust_score: TrustScoreConfig,
    pub updated_by: Option<ObjectId>,
    #[serde(with = "bson::serde_helpers::datetime::FromChrono04DateTime")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GpsValidationConfig {
    #[serde(default = "default_accuracy_very_suspicious")]
    pub accuracy_very_suspicious: f64,
    #[serde(default = "default_accuracy_suspicious")]
    pub accuracy_suspicious: f64,
    #[serde(default = "default_speed_threshold")]
    pub speed_threshold: f64,
    #[serde(default = "default_60000")]
    pub timestamp_drift_max: i64,
    #[serde(default = "default_position_jump_threshold")]
    pub position_jump_threshold: f64,
    #[serde(default = "default_true")]
    pub altitude_zero_penalty: bool,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_accuracy_very_suspicious() -> f64 {
    GPS_ACCURACY_GOOD_THRESHOLD
}
fn default_accuracy_suspicious() -> f64 {
    GPS_ACCURACY_MEDIUM_THRESHOLD
}
fn default_speed_threshold() -> f64 {
    DEFAULT_SPEED_THRESHOLD
}
fn default_position_jump_threshold() -> f64 {
    POSITION_JUMP_THRESHOLD_M
}
fn default_60000() -> i64 {
    60000
}
fn default_true() -> bool {
    true
}

impl Default for GpsValidationConfig {
    fn default() -> Self {
        Self {
            accuracy_very_suspicious: GPS_ACCURACY_GOOD_THRESHOLD,
            accuracy_suspicious: GPS_ACCURACY_MEDIUM_THRESHOLD,
            speed_threshold: DEFAULT_SPEED_THRESHOLD,
            timestamp_drift_max: 60000,
            position_jump_threshold: POSITION_JUMP_THRESHOLD_M,
            altitude_zero_penalty: true,
            enabled: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmulatorDetectionConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub block_on_high_severity: bool,
}

impl Default for EmulatorDetectionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            block_on_high_severity: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrustScoreConfig {
    #[serde(default = "default_15")]
    pub anomaly_penalty: f64,
    #[serde(default = "default_10")]
    pub safe_review_bonus: f64,
}

fn default_15() -> f64 {
    15.0
}

fn default_10() -> f64 {
    10.0
}

impl Default for TrustScoreConfig {
    fn default() -> Self {
        Self {
            anomaly_penalty: 15.0,
            safe_review_bonus: 10.0,
        }
    }
}

impl Default for SystemConfig {
    fn default() -> Self {
        Self {
            id: None,
            dev_bypass_enabled: false,
            gps_validation: GpsValidationConfig::default(),
            emulator_detection: EmulatorDetectionConfig::default(),
            trust_score: TrustScoreConfig::default(),
            updated_by: None,
            updated_at: Utc::now(),
        }
    }
}

impl SystemConfig {
    pub fn collection_name() -> &'static str {
        "systemconfigs"
    }
}
