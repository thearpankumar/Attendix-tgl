pub mod csrf;
pub mod validators;

mod auth;
mod device_check;
mod device_fingerprint;
mod device_integrity;
mod device_trust;
pub mod emulator_detection;
mod gps_validation;
mod mobile_check;
mod rate_limit_middleware;
mod rate_limiter;
mod security_extractors;
mod session_cache;

pub use auth::*;
pub use csrf::{csrf_middleware, generate_csrf_token, validate_csrf_token};
pub use device_check::{device_check_middleware, DeviceCheckResult};
pub use device_fingerprint::{check_device_blocked, record_device_success};
pub use device_trust::{
    flag_suspicious_device, get_device_trust_score, update_device_trust, DeviceTrustScore,
};
pub use emulator_detection::*;
pub use gps_validation::{
    check_altitude_issue, check_gps_accuracy, check_gps_speed, check_mock_location,
    check_position_jump, check_provider_mismatch, check_suspicious_accuracy, check_timestamp_drift,
    GpsAnomalyResult,
};
pub use mobile_check::{check_mobile, detect_ua_spoofing, mobile_check_middleware, DeviceInfo};
pub use rate_limit_middleware::{
    admin_rate_limit_middleware, login_rate_limit_middleware, student_rate_limit_middleware,
};
pub use rate_limiter::RateLimiter;
pub use security_extractors::{
    device_integrity_middleware, emulator_detection_middleware, gps_validation_middleware,
    DeviceIntegrityResult, EmulatorDetectionResult, GpsDataPayload, GpsValidationResult,
};
pub use session_cache::{get_or_fetch_session, CachedSession, SessionCache};
pub use validators::*;
