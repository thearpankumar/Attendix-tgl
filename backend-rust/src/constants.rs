// Named constants for magic numbers throughout the codebase
// This improves maintainability and self-documentation

// GPS Speed thresholds (in km/h)
pub const MAX_REASONABLE_SPEED_KMH: f64 = 200.0;
pub const IMPOSSIBLE_SPEED_KMH: f64 = 500.0;

// Photo similarity threshold (for detecting photo reuse)
pub const PHOTO_SIMILARITY_THRESHOLD: f32 = 0.15;
pub const PHOTO_SIMILARITY_HIGH_THRESHOLD: f32 = 0.85;

// Photo reuse detection previously only compared against the current
// session; this bounds how far back (in days) the cross-session check looks.
pub const PHOTO_REUSE_WINDOW_DAYS: i32 = 30;

// Distance thresholds (in meters)
pub const GEOGENCE_MAX_DISTANCE_M: f64 = 100.0;
pub const POSITION_JUMP_THRESHOLD_M: f64 = 500.0;

// IP-geolocation sanity cross-check: how far (in meters) the claimed GPS
// coordinates may be from the coarse, ISP-level location of the request IP
// before it's flagged. Deliberately generous — mobile carrier NAT and city-
// level IP geolocation are both coarse — but a mismatch of this size means
// the claimed GPS position and the network the request actually came from
// disagree about which country/region the device is in, which is a much
// harder signal to fake than the GPS coordinates themselves.
pub const IP_GEO_MISMATCH_THRESHOLD_M: f64 = 300_000.0;

// GPS accuracy thresholds (in meters)
pub const GPS_ACCURACY_GOOD_THRESHOLD: f64 = 3.0;
pub const GPS_ACCURACY_MEDIUM_THRESHOLD: f64 = 10.0;
pub const GPS_ACCURACY_SUSPICIOUS_THRESHOLD: f64 = 100.0;

// Pagination limits
pub const DASHBOARD_PAGE_SIZE: i64 = 50;
pub const RECENT_ACTIVITY_LIMIT: i64 = 5;

// Device verification confidence adjustment
pub const DEVICE_CONFIDENCE_PENALTY: f64 = 0.15;

// Default speed threshold (softer bound than max reasonable)
pub const DEFAULT_SPEED_THRESHOLD: f64 = 50.0;

// Short-link code resolution: a dedicated, tighter cap distinct from the
// general per-IP student rate limit, since a short code (10 chars from a
// 33-char alphabet) is much lower-entropy than the session token it stands
// in for and is otherwise only defended by that general limiter.
pub const SHORT_LINK_GUESS_MAX_ATTEMPTS: u32 = 15;
pub const SHORT_LINK_GUESS_WINDOW_SECS: u64 = 300;

// Per-identity submission caps (in addition to the general per-IP student
// limiter) — a legitimate student submits attendance once per session, so
// these are deliberately generous relative to normal use but bound both a
// shared-NAT false-positive scenario and a rotating-IP attacker.
pub const ROLL_NUMBER_SUBMIT_MAX_ATTEMPTS: u32 = 10;
pub const ROLL_NUMBER_SUBMIT_WINDOW_SECS: u64 = 300;
pub const DEVICE_FINGERPRINT_SUBMIT_MAX_ATTEMPTS: u32 = 20;
pub const DEVICE_FINGERPRINT_SUBMIT_WINDOW_SECS: u64 = 300;

// Dashboard pagination limits
pub const DASHBOARD_RESCUE_LIST_LIMIT: usize = 10;
pub const DASHBOARD_QUARANTINE_LIST_LIMIT: i64 = 5;
pub const DASHBOARD_LOW_BATCHES_LIMIT: usize = 5;
pub const ATTENDANCE_EXPORT_LIMIT: i64 = 100;

// Percentage thresholds for risk classification (0-100 scale)
pub const RISK_THRESHOLD_LOW: i64 = 85;
pub const RISK_THRESHOLD_MEDIUM: i64 = 75;
pub const HEALTH_THRESHOLD_GOOD: i64 = 85;
pub const HEALTH_THRESHOLD_MEDIUM: i64 = 50;

// WebAuthn grace period (minutes)
pub const WEBAUTHN_GRACE_PERIOD_MINUTES: i64 = 15;

// ========== Severity Enum for GPS Anomalies and Emulator Flags ==========
use serde::{Deserialize, Serialize};
use std::fmt;

/// Severity level for GPS anomalies and emulator detection flags.
/// Using an enum instead of heap-allocated strings reduces memory overhead
/// and provides type safety.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    High,
    #[default]
    Medium,
    Low,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Severity::High => write!(f, "high"),
            Severity::Medium => write!(f, "medium"),
            Severity::Low => write!(f, "low"),
        }
    }
}

// ========== Static String Constants ==========
// These replace repeated .to_string() calls for static values

// Dashboard status strings
pub const STATUS_ON_TRACK: &str = "On Track";
pub const STATUS_AT_RISK: &str = "At Risk";
pub const STATUS_CRITICAL: &str = "Critical";
pub const STATUS_UNKNOWN: &str = "Unknown";

// Health status strings
pub const HEALTH_STATUS_HEALTHY: &str = "healthy";
pub const HEALTH_STATUS_DEGRADED: &str = "degraded";
pub const HEALTH_STATUS_UNHEALTHY: &str = "unhealthy";

// Delta type strings for dashboard metrics
pub const DELTA_TYPE_UP: &str = "up";
pub const DELTA_TYPE_DOWN: &str = "down";
pub const DELTA_TYPE_RIGHT: &str = "right";

// Role strings
pub const ROLE_ADMIN: &str = "admin";
pub const ROLE_SUPER_ADMIN: &str = "super_admin";

// Canary admin account username (see migrations/0003_photo_hash_reuse_window.sql
// and controllers/admin/auth.rs::login) — a disabled decoy account. Any login
// attempt against it is treated as a near-zero-false-positive intrusion signal.
pub const CANARY_ADMIN_USERNAME: &str = "superadmin";

// Platform and browser strings (used in mobile_check)
pub const PLATFORM_ANDROID: &str = "Android";
pub const PLATFORM_IOS: &str = "iOS";
pub const PLATFORM_WINDOWS: &str = "Windows";
pub const PLATFORM_MAC: &str = "Mac";
pub const PLATFORM_LINUX: &str = "Linux";
pub const PLATFORM_UNKNOWN: &str = "Unknown";

pub const BROWSER_EDGE: &str = "Edge";
pub const BROWSER_CHROME: &str = "Chrome";
pub const BROWSER_FIREFOX: &str = "Firefox";
pub const BROWSER_SAFARI: &str = "Safari";
pub const BROWSER_OPERA: &str = "Opera";

// Provider strings
pub const PROVIDER_NETWORK: &str = "network";

// Confidence levels for GPS validation
pub const CONFIDENCE_HIGH: &str = "high";
pub const CONFIDENCE_MEDIUM: &str = "medium";
pub const CONFIDENCE_LOW: &str = "low";
pub const CONFIDENCE_SUSPICIOUS: &str = "suspicious";

// Flag type constants (used in device.rs)
pub const FLAG_MULTI_STUDENT_DEVICE: &str = "MULTI_STUDENT_DEVICE";
pub const FLAG_STUDENT_DEVICE_SWITCHED: &str = "STUDENT_DEVICE_SWITCHED";
pub const FLAG_RAPID_SUBMISSION: &str = "RAPID_SUBMISSION";
pub const FLAG_DEVICE_FINGERPRINT_CHANGE: &str = "DEVICE_FINGERPRINT_CHANGE";

// GPS anomaly type constants
pub const ANOMALY_CLIENT_REPORTED_MOCK: &str = "CLIENT_REPORTED_MOCK";
pub const ANOMALY_ACCURACY_VERY_SUSPICIOUS: &str = "ACCURACY_VERY_SUSPICIOUS";
pub const ANOMALY_ACCURACY_SUSPICIOUS: &str = "ACCURACY_SUSPICIOUS";
pub const ANOMALY_ALTITUDE_ZERO_OR_NULL: &str = "ALTITUDE_ZERO_OR_NULL";
pub const ANOMALY_POSITION_JUMP: &str = "POSITION_JUMP";
pub const ANOMALY_LOW_ACCURACY: &str = "LOW_ACCURACY";
pub const ANOMALY_IMPOSSIBLE_SPEED: &str = "IMPOSSIBLE_SPEED";
pub const ANOMALY_TIMESTAMP_DRIFT: &str = "TIMESTAMP_DRIFT";
pub const ANOMALY_PROVIDER_MISMATCH: &str = "PROVIDER_MISMATCH";
/// Raised when a submission carries no GPS metadata at all, so none of the
/// spoofing checks above could run. Absent evidence is not evidence of absence.
pub const ANOMALY_MISSING_METADATA: &str = "GPS_METADATA_MISSING";

// Emulator flag type constants
pub const EMULATOR_FLAG_DESKTOP_GPU: &str = "DESKTOP_GPU_DETECTED";
pub const EMULATOR_FLAG_AUDIO_FINGERPRINT: &str = "AUDIO_FINGERPRINT_EMULATOR";
pub const EMULATOR_FLAG_TIMING_ANOMALY: &str = "TIMING_ANOMALY";
pub const EMULATOR_FLAG_SCREEN_RESOLUTION: &str = "SCREEN_RESOLUTION_SUSPICIOUS";
pub const EMULATOR_FLAG_WEBGL_RENDERER: &str = "WEBGL_RENDERER_EMULATOR";
pub const EMULATOR_FLAG_PLATFORM_INCONSISTENCY: &str = "PLATFORM_INCONSISTENCY";
/// No device metrics were supplied, so emulator detection could not run.
pub const EMULATOR_FLAG_METRICS_MISSING: &str = "DEVICE_METRICS_MISSING";
/// The User-Agent the client reported about itself differs from the one the
/// server observed on the wire — an honest browser reports the same string.
pub const EMULATOR_FLAG_USER_AGENT_MISMATCH: &str = "USER_AGENT_MISMATCH";

// WebAuthn constants
pub const WEBAUTHN_DEVICE_UNKNOWN: &str = "Unknown Device";
pub const WEBAUTHN_AUTHENTICATOR_TYPE_MULTI: &str = "multiDevice";

/// Ceiling on any single request body. Attendance submissions are small JSON
/// documents (photos are uploaded to S3 out-of-band); the roster spreadsheet
/// upload is the largest legitimate payload.
pub const MAX_REQUEST_BODY_BYTES: usize = 8 * 1024 * 1024;
