use axum::{
    extract::State,
    routing::{get, post},
    Extension, Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::error::{AppError, Result};
use crate::middleware::AuthenticatedAdmin;
use crate::models::SystemConfig;
use crate::AppState;

use crate::middleware::auth_middleware;

pub fn create_routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(get_config).put(update_config))
        .route("/dev-bypass", post(toggle_dev_bypass))
        .route("/defaults", get(get_config_defaults))
        .route_layer(axum::middleware::from_fn_with_state(state, auth_middleware))
}

async fn get_config(
    State(state): State<Arc<AppState>>,
    Extension(_admin): Extension<AuthenticatedAdmin>,
) -> Result<impl axum::response::IntoResponse> {
    // Return the hot-reload cached config — fast path
    let config = state.get_system_config().await;
    Ok(Json(config))
}

/// The subset of `SystemConfig` an admin may change through `PUT /api/config`.
///
/// `dev_bypass_enabled` is deliberately absent. It used to be reachable here
/// because the handler deserialised a whole `SystemConfig`, so
/// `PUT {"devBypassEnabled": true}` flipped the global bypass without the
/// password re-confirmation that `POST /api/config/dev-bypass` enforces —
/// after which students could self-assert `devBypassGps`/`devBypassCamera`/
/// `devBypassWebauthn` and skip the geofence, camera and biometric checks.
///
/// Every field is optional and omitted fields keep their current value.
/// Previously omitted fields silently reset to defaults, so a partial PUT
/// wiped rate limits, lockout thresholds and GPS tolerances.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct UpdateConfigRequest {
    gps_validation: Option<crate::models::GpsValidationConfig>,
    emulator_detection: Option<crate::models::EmulatorDetectionConfig>,
    trust_score: Option<crate::models::TrustScoreConfig>,
    rate_limits: Option<crate::models::RateLimitsConfig>,
    webauthn_config: Option<crate::models::WebAuthnSystemConfig>,
    photo_verification: Option<crate::models::PhotoVerificationConfig>,
    session_config: Option<crate::models::SessionConfig>,
    lockout_config: Option<crate::models::LockoutConfig>,
    attendance_config: Option<crate::models::AttendanceConfig>,
}

async fn update_config(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Json(payload): Json<UpdateConfigRequest>,
) -> Result<impl axum::response::IntoResponse> {
    use chrono::Utc;

    // Merge onto the persisted config so `dev_bypass_enabled` survives
    // untouched and omitted sections are preserved.
    let mut current = SystemConfig::load(&state.db)
        .await?
        .unwrap_or_else(SystemConfig::default);

    if let Some(v) = payload.gps_validation {
        current.gps_validation = v;
    }
    if let Some(v) = payload.emulator_detection {
        current.emulator_detection = v;
    }
    if let Some(v) = payload.trust_score {
        current.trust_score = v;
    }
    if let Some(v) = payload.rate_limits {
        current.rate_limits = v;
    }
    if let Some(v) = payload.webauthn_config {
        current.webauthn_config = v;
    }
    if let Some(v) = payload.photo_verification {
        current.photo_verification = v;
    }
    if let Some(v) = payload.session_config {
        current.session_config = v;
    }
    if let Some(v) = payload.lockout_config {
        current.lockout_config = v;
    }
    if let Some(v) = payload.attendance_config {
        current.attendance_config = v;
    }

    current.updated_by = Some(auth.id);
    current.updated_at = Utc::now();

    let saved = current.save(&state.db).await?;

    // Flush in-memory hot-reload cache
    state.set_system_config(saved.clone()).await;

    Ok(Json(serde_json::json!({
        "message": "System configuration saved successfully",
        "config": saved
    })))
}

async fn get_config_defaults(
    Extension(_admin): Extension<AuthenticatedAdmin>,
) -> Result<impl axum::response::IntoResponse> {
    Ok(Json(SystemConfig::default()))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DevBypassRequest {
    enabled: bool,
    password: String,
}

#[derive(Debug, Serialize)]
struct DevBypassResponse {
    message: String,
    config: SystemConfig,
}

async fn toggle_dev_bypass(
    State(state): State<Arc<AppState>>,
    Extension(auth_admin): Extension<AuthenticatedAdmin>,
    Json(payload): Json<DevBypassRequest>,
) -> Result<impl axum::response::IntoResponse> {
    use chrono::Utc;

    let admin = sqlx::query_as::<_, crate::models::Admin>("SELECT * FROM admins WHERE id = $1")
        .bind(auth_admin.id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::Unauthorized("Admin not found".to_string()))?;

    if !admin.verify_password(&payload.password)? {
        return Err(AppError::Unauthorized("Invalid password".to_string()));
    }

    let mut config = SystemConfig::load(&state.db)
        .await?
        .unwrap_or_else(SystemConfig::default);

    config.dev_bypass_enabled = payload.enabled;
    config.updated_by = Some(auth_admin.id);
    config.updated_at = Utc::now();

    let saved = config.save(&state.db).await?;

    // Flush hot-reload cache
    state.set_system_config(saved.clone()).await;

    Ok(Json(DevBypassResponse {
        message: "Developer Bypass Mode updated successfully".to_string(),
        config: saved,
    }))
}
