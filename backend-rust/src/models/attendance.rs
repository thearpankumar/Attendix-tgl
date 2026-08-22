use crate::constants::Severity;
use crate::models::text_enum_sqlx;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Attendance {
    #[serde(rename = "_id")]
    pub id: Uuid,
    pub session_id: Uuid,
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
    #[serde(default)]
    pub source: AttendanceSource,
    #[serde(default)]
    pub status: AttendanceStatus,
    pub marked_by_admin_id: Option<Uuid>,
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
    pub flag_reviewed_by: Option<Uuid>,
    pub flag_reviewed_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub flagged: bool,
    pub flag_reason: Option<String>,
    pub flag_details: Option<String>,
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
    pub gps_anomalies: Json<Vec<GpsAnomaly>>,
    pub gps_confidence: Option<GpsConfidence>,
    #[serde(default)]
    pub emulator_detected: bool,
    #[serde(default)]
    pub emulator_flags: Json<Vec<EmulatorFlag>>,
    #[serde(default)]
    pub integrity_checks: Json<Vec<IntegrityCheck>>,
}

fn default_true() -> bool {
    true
}

/// Whether an attendance row came from the student's own self-submission
/// (photo + geofence + anti-fraud checks) or was marked in-person by a
/// mentor/admin during an exam session. `source` is the durable signal that
/// explains why a manual row's anti-fraud columns are empty rather than
/// missing due to a bug.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AttendanceSource {
    #[serde(rename = "self_submitted")]
    #[default]
    SelfSubmitted,
    #[serde(rename = "manual")]
    Manual,
}
text_enum_sqlx!(AttendanceSource);

/// Present/absent state of an attendance row. Self-submitted rows are always
/// `Present` (submitting a photo from inside the geofence is the presence
/// event); manually-marked rows can be either, since a mentor can explicitly
/// record a student as absent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AttendanceStatus {
    #[serde(rename = "present")]
    #[default]
    Present,
    #[serde(rename = "absent")]
    Absent,
}
text_enum_sqlx!(AttendanceStatus);

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
text_enum_sqlx!(AttendanceDeviceFlag);

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
text_enum_sqlx!(WebAuthnDeviceType);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WebAuthnAttachment {
    #[serde(rename = "platform")]
    Platform,
    #[serde(rename = "cross-platform")]
    CrossPlatform,
}
text_enum_sqlx!(WebAuthnAttachment);

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
text_enum_sqlx!(GpsConfidence);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GpsAnomaly {
    #[serde(rename = "type")]
    pub anomaly_type: GpsAnomalyType,
    #[serde(default)]
    pub severity: Severity,
    pub details: Option<String>,
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
    #[serde(rename = "IP_GEO_MISMATCH")]
    IpGeoMismatch,
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
    pub fn table_name() -> &'static str {
        "attendances"
    }
}

#[cfg(test)]
mod serialization_tests {
    use super::*;

    // Regression test for the "selecting one student selects all" bug: this
    // struct is `#[serde(flatten)]`-ed directly into `SessionAttendanceResponse`
    // (controllers::admin::sessions), so if `id` ever loses its `_id` rename
    // again, every admin SessionDetail row would collapse onto the same
    // `undefined` selection key, exactly like before the fix.
    #[test]
    fn serializes_id_as_underscore_id_not_id() {
        let attendance = Attendance {
            id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            student_name: "Test Student".to_string(),
            roll_number: "CS101".to_string(),
            photo_url: "https://example.com/photo.jpg".to_string(),
            photo_public_id: "photo-id".to_string(),
            photo_hash: None,
            photo_reuse_detected: false,
            student_latitude: 12.9716,
            student_longitude: 77.5946,
            distance_from_location: 10.0,
            ip_address: None,
            user_agent: None,
            network_provider: None,
            network_org: None,
            verified: false,
            source: AttendanceSource::default(),
            status: AttendanceStatus::default(),
            marked_by_admin_id: None,
            face_detected: true,
            device_fingerprint: None,
            device_fingerprint_hash: None,
            device_first_seen: false,
            totp_code: None,
            totp_valid: None,
            device_flag: None,
            webauthn_credential_id: None,
            webauthn_verified: false,
            webauthn_device_type: None,
            webauthn_authenticator_attachment: None,
            webauthn_counter: None,
            webauthn_replay_attack: false,
            flag_reviewed: false,
            flag_reviewed_by: None,
            flag_reviewed_at: None,
            flagged: false,
            flag_reason: None,
            flag_details: None,
            captured_at: Utc::now(),
            gps_accuracy: None,
            gps_altitude: None,
            gps_altitude_accuracy: None,
            gps_speed: None,
            gps_heading: None,
            gps_timestamp: None,
            gps_mock_location: false,
            gps_provider: None,
            gps_anomalies: Json(Vec::new()),
            gps_confidence: None,
            emulator_detected: false,
            emulator_flags: Json(Vec::new()),
            integrity_checks: Json(Vec::new()),
        };

        let value = serde_json::to_value(&attendance).expect("Attendance must serialize");
        assert!(
            value.get("_id").is_some(),
            "Attendance JSON must have an \"_id\" key (was: {:?})",
            value.as_object().map(|o| o.keys().collect::<Vec<_>>())
        );
        assert!(
            value.get("id").is_none(),
            "Attendance JSON must NOT have a plain \"id\" key alongside \"_id\""
        );
        assert_eq!(value["_id"], serde_json::json!(attendance.id));
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttendanceSubmit {
    pub session_id: Uuid,
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
