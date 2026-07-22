use crate::constants::Severity;
use chrono::{DateTime, Utc};
use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Attendance {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub session_id: ObjectId,
    pub student_name: String,
    pub roll_number: String,
    pub photo_url: String,
    pub photo_public_id: String,
    pub photo_hash: Option<String>,
    #[serde(default)]
    pub photo_reuse_detected: bool,
    pub student_latitude: f64,
    pub student_longitude: f64,
    pub distance_from_location: f64,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub network_provider: Option<String>,
    pub network_org: Option<String>,
    #[serde(default)]
    pub verified: bool,
    #[serde(default = "default_true")]
    pub face_detected: bool,
    pub device_fingerprint: Option<String>,
    pub device_fingerprint_hash: Option<String>,
    #[serde(default)]
    pub device_first_seen: bool,
    pub totp_code: Option<String>,
    pub totp_valid: Option<bool>,
    pub device_flag: Option<AttendanceDeviceFlag>,
    pub webauthn_credential_id: Option<String>,
    #[serde(default)]
    pub webauthn_verified: bool,
    pub webauthn_device_type: Option<WebAuthnDeviceType>,
    pub webauthn_authenticator_attachment: Option<WebAuthnAttachment>,
    pub webauthn_counter: Option<i32>,
    #[serde(default)]
    pub webauthn_replay_attack: bool,
    #[serde(default)]
    pub flag_reviewed: bool,
    pub flag_reviewed_by: Option<ObjectId>,
    pub flag_reviewed_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub flagged: bool,
    pub flag_reason: Option<String>,
    pub flag_details: Option<String>,
    #[serde(with = "bson::serde_helpers::datetime::FromChrono04DateTime")]
    pub captured_at: DateTime<Utc>,
    pub gps_accuracy: Option<f64>,
    pub gps_altitude: Option<f64>,
    pub gps_altitude_accuracy: Option<f64>,
    pub gps_speed: Option<f64>,
    pub gps_heading: Option<f64>,
    pub gps_timestamp: Option<i64>,
    #[serde(default)]
    pub gps_mock_location: bool,
    pub gps_provider: Option<String>,
    #[serde(default)]
    pub gps_anomalies: Vec<GpsAnomaly>,
    pub gps_confidence: Option<GpsConfidence>,
    #[serde(default)]
    pub emulator_detected: bool,
    #[serde(default)]
    pub emulator_flags: Vec<EmulatorFlag>,
    #[serde(default)]
    pub integrity_checks: Vec<IntegrityCheck>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AttendanceDeviceFlag {
    #[serde(rename = "MULTI_STUDENT_DEVICE")]
    MultiStudentDevice,
    #[serde(rename = "STUDENT_DEVICE_SWITCHED")]
    StudentDeviceSwitched,
    #[serde(rename = "RAPID_SUBMISSION")]
    RapidSubmission,
    #[serde(rename = "DEVICE_FINGERPRINT_CHANGE")]
    DeviceFingerprintChange,
    #[serde(rename = "WEBAUTHN_REPLAY_ATTACK")]
    WebauthnReplayAttack,
    #[serde(rename = "WEBAUTHN_NOT_SUPPORTED")]
    WebauthnNotSupported,
    #[serde(rename = "WEBAUTHN_CREDENTIAL_SUSPENDED")]
    WebauthnCredentialSuspended,
    #[serde(rename = "GPS_ANOMALY_DETECTED")]
    GpsAnomalyDetected,
    #[serde(rename = "EMULATOR_DETECTED")]
    EmulatorDetected,
    #[serde(rename = "INTEGRITY_CHECK_FAILED")]
    IntegrityCheckFailed,
    #[serde(rename = "REUSED_PHOTO")]
    ReusedPhoto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WebAuthnDeviceType {
    #[serde(rename = "face_id")]
    FaceId,
    #[serde(rename = "touch_id")]
    TouchId,
    #[serde(rename = "fingerprint")]
    Fingerprint,
    #[serde(rename = "passkey_fallback")]
    PasskeyFallback,
    #[serde(rename = "unknown")]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WebAuthnAttachment {
    #[serde(rename = "platform")]
    Platform,
    #[serde(rename = "cross-platform")]
    CrossPlatform,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GpsConfidence {
    #[serde(rename = "high")]
    High,
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "low")]
    Low,
    #[serde(rename = "suspicious")]
    Suspicious,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GpsAnomaly {
    #[serde(rename = "type")]
    pub anomaly_type: GpsAnomalyType,
    #[serde(default)]
    pub severity: Severity,
    pub details: Option<String>,
    #[serde(with = "bson::serde_helpers::datetime::FromChrono04DateTime")]
    pub detected_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GpsAnomalyType {
    #[serde(rename = "ACCURACY_SUSPICIOUS")]
    AccuracySuspicious,
    #[serde(rename = "ACCURACY_VERY_SUSPICIOUS")]
    AccuracyVerySuspicious,
    #[serde(rename = "ALTITUDE_ZERO_OR_NULL")]
    AltitudeZeroOrNull,
    #[serde(rename = "SPEED_IMPOSSIBLE")]
    SpeedImpossible,
    #[serde(rename = "POSITION_JUMP")]
    PositionJump,
    #[serde(rename = "TIMESTAMP_DRIFT")]
    TimestampDrift,
    #[serde(rename = "ACCURACY_PATTERN")]
    AccuracyPattern,
    #[serde(rename = "PROVIDER_MISMATCH")]
    ProviderMismatch,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmulatorFlag {
    #[serde(rename = "type")]
    pub flag_type: EmulatorFlagType,
    #[serde(default)]
    pub severity: Severity,
    pub details: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EmulatorFlagType {
    #[serde(rename = "DESKTOP_GPU_DETECTED")]
    DesktopGpuDetected,
    #[serde(rename = "AUDIO_FINGERPRINT_EMULATOR")]
    AudioFingerprintEmulator,
    #[serde(rename = "TIMING_ANOMALY")]
    TimingAnomaly,
    #[serde(rename = "BATTERY_PATTERN_EMULATOR")]
    BatteryPatternEmulator,
    #[serde(rename = "SCREEN_RESOLUTION_SUSPICIOUS")]
    ScreenResolutionSuspicious,
    #[serde(rename = "DEVICE_MEMORY_ROUND")]
    DeviceMemoryRound,
    #[serde(rename = "WEBGL_RENDERER_EMULATOR")]
    WebglRendererEmulator,
    #[serde(rename = "PLATFORM_INCONSISTENCY")]
    PlatformInconsistency,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrityCheck {
    #[serde(rename = "type")]
    pub check_type: IntegrityCheckType,
    #[serde(default)]
    pub passed: bool,
    pub details: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IntegrityCheckType {
    #[serde(rename = "TIMING_MANIPULATION")]
    TimingManipulation,
    #[serde(rename = "BROWSER_API_INCONSISTENCY")]
    BrowserApiInconsistency,
    #[serde(rename = "POINTER_EVENTS_SUSPICIOUS")]
    PointerEventsSuspicious,
}

impl Attendance {
    pub fn collection_name() -> &'static str {
        "attendances"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttendanceSubmit {
    pub session_id: ObjectId,
    pub student_name: String,
    pub roll_number: String,
    pub photo_url: String,
    pub photo_public_id: String,
    pub student_latitude: f64,
    pub student_longitude: f64,
    pub distance_from_location: f64,
    pub device_fingerprint: Option<String>,
    pub totp_code: Option<String>,
    pub webauthn_verified: Option<bool>,
    pub webauthn_credential_id: Option<String>,
}
