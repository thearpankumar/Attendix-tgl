use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use sqlx::Row;
use uuid::Uuid;

use crate::constants::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemConfig {
    #[serde(default)]
    pub id: Uuid,
    #[serde(default)]
    pub dev_bypass_enabled: bool,
    #[serde(default)]
    pub gps_validation: GpsValidationConfig,
    #[serde(default)]
    pub emulator_detection: EmulatorDetectionConfig,
    #[serde(default)]
    pub trust_score: TrustScoreConfig,
    #[serde(default)]
    pub rate_limits: RateLimitsConfig,
    #[serde(default)]
    pub webauthn_config: WebAuthnSystemConfig,
    #[serde(default)]
    pub photo_verification: PhotoVerificationConfig,
    #[serde(default)]
    pub session_config: SessionConfig,
    #[serde(default)]
    pub lockout_config: LockoutConfig,
    #[serde(default)]
    pub attendance_config: AttendanceConfig,
    pub updated_by: Option<Uuid>,
    #[serde(default = "Utc::now")]
    pub updated_at: DateTime<Utc>,
}

/// Shape of the JSONB `config` column — everything except the columns that are
/// broken out for direct querying (`id`, `dev_bypass_enabled`, `updated_by`,
/// `updated_at`).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct SystemConfigJson {
    #[serde(default)]
    gps_validation: GpsValidationConfig,
    #[serde(default)]
    emulator_detection: EmulatorDetectionConfig,
    #[serde(default)]
    trust_score: TrustScoreConfig,
    #[serde(default)]
    rate_limits: RateLimitsConfig,
    #[serde(default)]
    webauthn_config: WebAuthnSystemConfig,
    #[serde(default)]
    photo_verification: PhotoVerificationConfig,
    #[serde(default)]
    session_config: SessionConfig,
    #[serde(default)]
    lockout_config: LockoutConfig,
    #[serde(default)]
    attendance_config: AttendanceConfig,
}

impl sqlx::FromRow<'_, sqlx::postgres::PgRow> for SystemConfig {
    fn from_row(row: &sqlx::postgres::PgRow) -> sqlx::Result<Self> {
        let id: Uuid = row.try_get("id")?;
        let dev_bypass_enabled: bool = row.try_get("dev_bypass_enabled")?;
        let config: Json<SystemConfigJson> = row.try_get("config")?;
        let updated_by: Option<Uuid> = row.try_get("updated_by")?;
        let updated_at: DateTime<Utc> = row.try_get("updated_at")?;
        let SystemConfigJson {
            gps_validation,
            emulator_detection,
            trust_score,
            rate_limits,
            webauthn_config,
            photo_verification,
            session_config,
            lockout_config,
            attendance_config,
        } = config.0;

        Ok(SystemConfig {
            id,
            dev_bypass_enabled,
            gps_validation,
            emulator_detection,
            trust_score,
            rate_limits,
            webauthn_config,
            photo_verification,
            session_config,
            lockout_config,
            attendance_config,
            updated_by,
            updated_at,
        })
    }
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
    #[serde(default = "default_geofence_max_distance")]
    pub geofence_max_distance_m: f64,
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
fn default_geofence_max_distance() -> f64 {
    GEOGENCE_MAX_DISTANCE_M
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
            geofence_max_distance_m: GEOGENCE_MAX_DISTANCE_M,
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

impl SystemConfig {
    pub fn table_name() -> &'static str {
        "system_configs"
    }

    /// The JSONB payload to persist in the `config` column.
    fn as_json_blob(&self) -> Json<SystemConfigJson> {
        Json(SystemConfigJson {
            gps_validation: self.gps_validation.clone(),
            emulator_detection: self.emulator_detection.clone(),
            trust_score: self.trust_score.clone(),
            rate_limits: self.rate_limits.clone(),
            webauthn_config: self.webauthn_config.clone(),
            photo_verification: self.photo_verification.clone(),
            session_config: self.session_config.clone(),
            lockout_config: self.lockout_config.clone(),
            attendance_config: self.attendance_config.clone(),
        })
    }

    /// Loads the singleton config row, if one exists.
    pub async fn load(pool: &sqlx::PgPool) -> sqlx::Result<Option<SystemConfig>> {
        sqlx::query_as::<_, SystemConfig>(
            "SELECT id, dev_bypass_enabled, config, updated_by, updated_at FROM system_configs LIMIT 1",
        )
        .fetch_optional(pool)
        .await
    }

    /// Upserts the singleton config row (updating the existing row if present,
    /// inserting one otherwise) and returns the persisted row.
    pub async fn save(&self, pool: &sqlx::PgPool) -> sqlx::Result<SystemConfig> {
        let existing_id: Option<Uuid> = sqlx::query_scalar("SELECT id FROM system_configs LIMIT 1")
            .fetch_optional(pool)
            .await?;

        let blob = self.as_json_blob();

        match existing_id {
            Some(id) => {
                sqlx::query_as::<_, SystemConfig>(
                    "UPDATE system_configs SET dev_bypass_enabled = $1, config = $2, updated_by = $3, updated_at = now() \
                     WHERE id = $4 \
                     RETURNING id, dev_bypass_enabled, config, updated_by, updated_at",
                )
                .bind(self.dev_bypass_enabled)
                .bind(blob)
                .bind(self.updated_by)
                .bind(id)
                .fetch_one(pool)
                .await
            }
            None => {
                sqlx::query_as::<_, SystemConfig>(
                    "INSERT INTO system_configs (dev_bypass_enabled, config, updated_by, updated_at) \
                     VALUES ($1, $2, $3, now()) \
                     RETURNING id, dev_bypass_enabled, config, updated_by, updated_at",
                )
                .bind(self.dev_bypass_enabled)
                .bind(blob)
                .bind(self.updated_by)
                .fetch_one(pool)
                .await
            }
        }
    }
}

// =================== Rate Limits Config ===================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RateLimitsConfig {
    #[serde(default = "default_rl_admin_window")]
    pub admin_window_secs: u64,
    #[serde(default = "default_rl_admin_max")]
    pub admin_max_requests: u32,
    #[serde(default = "default_rl_student_window")]
    pub student_window_secs: u64,
    #[serde(default = "default_rl_student_max")]
    pub student_max_requests: u32,
    #[serde(default = "default_rl_login_window")]
    pub login_window_secs: u64,
    #[serde(default = "default_rl_login_max")]
    pub login_max_requests: u32,
    #[serde(default = "default_rl_clientlog_window")]
    pub client_log_window_secs: u64,
    #[serde(default = "default_rl_clientlog_max")]
    pub client_log_max_requests: u32,
    /// New admin self-registration is gated only by the shared ADMIN_SECRET
    /// with no dedicated throttle beyond the generic login bucket — tighter
    /// on purpose, since a legitimate deployment registers new admins rarely.
    #[serde(default = "default_rl_registration_window")]
    pub registration_window_secs: u64,
    #[serde(default = "default_rl_registration_max")]
    pub registration_max_requests: u32,
}

fn default_rl_admin_window() -> u64 {
    60
}
fn default_rl_admin_max() -> u32 {
    1000
}
fn default_rl_student_window() -> u64 {
    60
}
fn default_rl_student_max() -> u32 {
    100
}
fn default_rl_login_window() -> u64 {
    60
}
fn default_rl_login_max() -> u32 {
    20
}
fn default_rl_clientlog_window() -> u64 {
    60
}
fn default_rl_clientlog_max() -> u32 {
    100
}
fn default_rl_registration_window() -> u64 {
    3600
}
fn default_rl_registration_max() -> u32 {
    3
}

impl Default for RateLimitsConfig {
    fn default() -> Self {
        Self {
            admin_window_secs: 60,
            admin_max_requests: 1000,
            student_window_secs: 60,
            student_max_requests: 100,
            login_window_secs: 60,
            login_max_requests: 20,
            client_log_window_secs: 60,
            client_log_max_requests: 100,
            registration_window_secs: 3600,
            registration_max_requests: 3,
        }
    }
}

// =================== WebAuthn System Config ===================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebAuthnSystemConfig {
    #[serde(default = "default_webauthn_grace")]
    pub grace_period_minutes: i64,
}

fn default_webauthn_grace() -> i64 {
    15
}

impl Default for WebAuthnSystemConfig {
    fn default() -> Self {
        Self {
            grace_period_minutes: 15,
        }
    }
}

// =================== Photo Verification Config ===================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhotoVerificationConfig {
    #[serde(default = "default_photo_low")]
    pub similarity_threshold: f32,
    #[serde(default = "default_photo_high")]
    pub high_similarity_threshold: f32,
}

fn default_photo_low() -> f32 {
    0.15
}
fn default_photo_high() -> f32 {
    0.85
}

impl Default for PhotoVerificationConfig {
    fn default() -> Self {
        Self {
            similarity_threshold: 0.15,
            high_similarity_threshold: 0.85,
        }
    }
}

// =================== Session Config ===================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionConfig {
    #[serde(default = "default_session_expire")]
    pub expire_minutes: u64,
}

fn default_session_expire() -> u64 {
    60
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self { expire_minutes: 60 }
    }
}

// =================== Lockout Config ===================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LockoutConfig {
    #[serde(default = "default_max_login_attempts")]
    pub max_login_attempts: u32,
    #[serde(default = "default_lockout_duration")]
    pub lockout_duration_minutes: u64,
}

fn default_max_login_attempts() -> u32 {
    5
}
fn default_lockout_duration() -> u64 {
    15
}

impl Default for LockoutConfig {
    fn default() -> Self {
        Self {
            max_login_attempts: 5,
            lockout_duration_minutes: 15,
        }
    }
}

// =================== Attendance Config ===================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttendanceConfig {
    #[serde(default = "default_max_attendance_attempts")]
    pub max_attendance_attempts: u32,
}

fn default_max_attendance_attempts() -> u32 {
    3
}

impl Default for AttendanceConfig {
    fn default() -> Self {
        Self {
            max_attendance_attempts: 3,
        }
    }
}

impl Default for SystemConfig {
    fn default() -> Self {
        Self {
            id: Uuid::nil(),
            dev_bypass_enabled: false,
            gps_validation: GpsValidationConfig::default(),
            emulator_detection: EmulatorDetectionConfig::default(),
            trust_score: TrustScoreConfig::default(),
            rate_limits: RateLimitsConfig::default(),
            webauthn_config: WebAuthnSystemConfig::default(),
            photo_verification: PhotoVerificationConfig::default(),
            session_config: SessionConfig::default(),
            lockout_config: LockoutConfig::default(),
            attendance_config: AttendanceConfig::default(),
            updated_by: None,
            updated_at: Utc::now(),
        }
    }
}
