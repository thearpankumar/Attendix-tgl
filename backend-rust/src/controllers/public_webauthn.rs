use axum::{
    extract::{ConnectInfo, Extension, Json, Path, State},
    response::IntoResponse,
};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::Json as SqlxJson;
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    error::{AppError, Result},
    models::{
        GpsAnomaly, GpsAnomalyType, Location, Session, Severity, ShortLink, WebAuthnChallenge,
        WebAuthnChallengeType, WebAuthnCredential,
    },
    services::IpInfo,
    utils::calculate_distance,
    AppState,
};

// =================== WebAuthn Status ===================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebAuthnStatusResponse {
    pub enrolled: bool,
    pub suspended: bool,
    pub already_submitted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub student_name: Option<String>,
}

/// Loads the active short link + its (active, unexpired) session for a scan flow.
/// Shared by every endpoint below that starts from a `short_code`.
async fn load_active_short_link_and_session(
    pool: &sqlx::PgPool,
    short_code: &str,
) -> Result<(ShortLink, Session)> {
    let short_link: ShortLink =
        sqlx::query_as("SELECT * FROM short_links WHERE short_code = $1 AND is_active = true")
            .bind(short_code.to_lowercase())
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| AppError::NotFound("Invalid session".to_string()))?;

    let session_id = short_link
        .session_id
        .ok_or_else(|| AppError::NotFound("No session associated with this link".to_string()))?;

    let session: Session = sqlx::query_as("SELECT * FROM sessions WHERE id = $1")
        .bind(session_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Session not found".to_string()))?;

    if !session.is_active || session.is_expired() {
        return Err(AppError::BadRequest("Session expired".to_string()));
    }

    Ok((short_link, session))
}

async fn insert_webauthn_challenge(
    pool: &sqlx::PgPool,
    challenge: &WebAuthnChallenge,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO webauthn_challenges \
         (id, student_id, challenge, challenge_type, session_id, short_code, student_name, expires_at, used, state, created_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
    )
    .bind(challenge.id)
    .bind(&challenge.student_id)
    .bind(&challenge.challenge)
    .bind(&challenge.challenge_type)
    .bind(challenge.session_id)
    .bind(&challenge.short_code)
    .bind(&challenge.student_name)
    .bind(challenge.expires_at)
    .bind(challenge.used)
    .bind(&challenge.state)
    .bind(challenge.created_at)
    .execute(pool)
    .await?;
    Ok(())
}

/// Consumes a pending ceremony, enforcing single use and binding to the short
/// code the request arrived on.
///
/// The old implementation matched on the challenge string alone, with no
/// `short_code` or `session_id` predicate, so a challenge issued for one
/// session could be replayed against another. It also let the request body's
/// roll number override the challenge's own subject.
async fn take_pending_challenge(
    pool: &sqlx::PgPool,
    short_code: &str,
    challenge_type: WebAuthnChallengeType,
) -> Result<Option<WebAuthnChallenge>> {
    // `FOR UPDATE SKIP LOCKED` + the `used` flip in one statement makes
    // consumption atomic, so two concurrent finishes cannot both succeed.
    let challenge: Option<WebAuthnChallenge> = sqlx::query_as(
        "UPDATE webauthn_challenges SET used = true \
         WHERE id = ( \
             SELECT id FROM webauthn_challenges \
             WHERE short_code = $1 AND challenge_type = $2 \
               AND used = false AND expires_at > now() AND state IS NOT NULL \
             ORDER BY created_at DESC \
             LIMIT 1 FOR UPDATE SKIP LOCKED \
         ) \
         RETURNING *",
    )
    .bind(short_code.to_lowercase())
    .bind(&challenge_type)
    .fetch_optional(pool)
    .await?;

    Ok(challenge)
}

/// Deserialises stored ceremony state, or reports a stale/corrupt row.
fn ceremony_state<T: serde::de::DeserializeOwned>(challenge: &WebAuthnChallenge) -> Result<T> {
    let raw = challenge
        .state
        .clone()
        .ok_or_else(|| AppError::BadRequest("Challenge has no stored state".to_string()))?;

    serde_json::from_value(raw)
        .map_err(|_| AppError::BadRequest("Challenge state is unreadable".to_string()))
}

pub async fn get_webauthn_status(
    State(state): State<Arc<AppState>>,
    Path((short_code, roll_number)): Path<(String, String)>,
) -> Result<impl IntoResponse> {
    let (_short_link, session) = load_active_short_link_and_session(&state.db, &short_code).await?;

    let roll_upper = roll_number.to_uppercase();

    let credential: Option<WebAuthnCredential> =
        sqlx::query_as("SELECT * FROM webauthn_credentials WHERE student_id = $1")
            .bind(&roll_upper)
            .fetch_optional(&state.db)
            .await?;

    let already_submitted: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM attendances WHERE session_id = $1 AND roll_number = $2)",
    )
    .bind(session.id)
    .bind(&roll_upper)
    .fetch_one(&state.db)
    .await?;

    if already_submitted {
        return Ok(Json(WebAuthnStatusResponse {
            enrolled: credential.is_some(),
            suspended: credential.as_ref().map(|c| c.is_suspended).unwrap_or(false),
            already_submitted: true,
            message: Some("Attendance already submitted".to_string()),
            student_name: None,
        }));
    }

    Ok(Json(WebAuthnStatusResponse {
        enrolled: credential.is_some(),
        suspended: credential.as_ref().map(|c| c.is_suspended).unwrap_or(false),
        already_submitted: false,
        message: None,
        student_name: credential.map(|c| c.device_label),
    }))
}

// =================== Registration Start ===================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistrationStartRequest {
    pub roll_number: String,
    pub student_name: String,
}

pub async fn start_registration(
    State(state): State<Arc<AppState>>,
    Path(short_code): Path<String>,
    Json(payload): Json<RegistrationStartRequest>,
) -> Result<impl IntoResponse> {
    let roll_upper = payload.roll_number.to_uppercase();

    let (_short_link, session) = load_active_short_link_and_session(&state.db, &short_code).await?;

    // A batch is entirely optional and never gates enrollment or attendance.
    // Where one is attached it is a roster for reporting (present vs absent)
    // and a signal for review — not an allowlist. An earlier revision refused
    // off-roster enrollments outright, which blocked legitimate students
    // whenever a roster was incomplete or a session had no batch at all.
    //
    // The anti-hijack concern this replaces (anyone could claim the credential
    // for any roll number, permanently) is now handled by surfacing off-roster
    // enrollments to the admin rather than by blocking the student.
    if let Some(batch_id) = session.batch_id {
        let on_roster: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM students WHERE batch_id = $1 AND upper(roll_number) = $2)",
        )
        .bind(batch_id)
        .bind(&roll_upper)
        .fetch_one(&state.db)
        .await?;

        if !on_roster {
            tracing::warn!(
                roll_number = %roll_upper,
                session_id = %session.id,
                "WebAuthn enrollment for a roll number that is not on this session's roster"
            );
        }
    }

    let existing_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM webauthn_credentials WHERE student_id = $1)",
    )
    .bind(&roll_upper)
    .fetch_one(&state.db)
    .await?;

    if existing_exists {
        return Err(AppError::BadRequest(
            "Device already enrolled. Contact admin to re-enroll on a new device.".to_string(),
        ));
    }

    // webauthn-rs generates the challenge and retains the matching state
    // (challenge bytes, user-verification policy, exclude list) for finish.
    let (creation_challenge, registration_state) = state
        .webauthn
        .start_passkey_registration(
            crate::webauthn_user_handle(&roll_upper),
            &roll_upper,
            &payload.student_name,
            None,
        )
        .map_err(|e| AppError::Internal(format!("Failed to start registration: {e}")))?;

    let webauthn_challenge = WebAuthnChallenge {
        id: Uuid::new_v4(),
        student_id: roll_upper.clone(),
        // Retained only for auditing; verification uses `state`, not this.
        challenge: b64url(creation_challenge.public_key.challenge.as_ref()),
        challenge_type: WebAuthnChallengeType::Registration,
        session_id: session.id,
        short_code: Some(short_code.to_lowercase()),
        student_name: Some(payload.student_name.clone()),
        expires_at: Utc::now() + Duration::minutes(5),
        used: false,
        state: Some(
            serde_json::to_value(&registration_state)
                .map_err(|e| AppError::Internal(format!("Failed to store ceremony state: {e}")))?,
        ),
        created_at: Utc::now(),
    };

    insert_webauthn_challenge(&state.db, &webauthn_challenge).await?;

    tracing::info!(
        roll_number = %roll_upper,
        session_id = %session.id,
        "WebAuthn enrollment started"
    );

    Ok(Json(creation_challenge))
}

// =================== Registration Finish ===================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistrationFinishRequest {
    /// Parsed and verified by `webauthn-rs`. The roll number is deliberately
    /// not accepted here — it is taken from the server-issued challenge.
    pub credential: webauthn_rs::prelude::RegisterPublicKeyCredential,
}

#[derive(Debug, Serialize)]
pub struct RegistrationFinishResponse {
    pub verified: bool,
    pub credential_id: String,
    pub message: String,
}

pub async fn finish_registration(
    State(state): State<Arc<AppState>>,
    Path(short_code): Path<String>,
    Json(payload): Json<RegistrationFinishRequest>,
) -> Result<impl IntoResponse> {
    let stored_challenge =
        take_pending_challenge(&state.db, &short_code, WebAuthnChallengeType::Registration)
            .await?
            .ok_or_else(|| {
                AppError::BadRequest("No valid registration challenge found".to_string())
            })?;

    // The subject comes from the challenge the server issued, never from the
    // request body. Previously the body's roll number took precedence, so a
    // challenge issued for one student could enroll a credential for another.
    let roll_upper = stored_challenge.student_id.to_uppercase();

    let registration_state: webauthn_rs::prelude::PasskeyRegistration =
        ceremony_state(&stored_challenge)?;

    // Verifies the attestation, the challenge, the origin, the rpIdHash and
    // the user-verification flag. Anything short of a genuine authenticator
    // response fails here.
    let passkey = state
        .webauthn
        .finish_passkey_registration(&payload.credential, &registration_state)
        .map_err(|e| {
            tracing::warn!(roll_number = %roll_upper, "WebAuthn registration rejected: {e}");
            AppError::BadRequest("Device registration could not be verified".to_string())
        })?;

    let credential_id = base64::Engine::encode(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD,
        passkey.cred_id(),
    );

    let passkey_blob = serde_json::to_vec(&passkey)
        .map_err(|e| AppError::Internal(format!("Failed to serialise credential: {e}")))?;

    // The unique index on `student_id` makes the enrollment race safe: a second
    // concurrent registration for the same roll number loses here rather than
    // silently overwriting the first.
    let inserted = sqlx::query(
        "INSERT INTO webauthn_credentials \
         (id, student_id, credential_id, passkey, user_handle, counter, device_label, device_type, transports, enrolled_at, sign_count) \
         VALUES ($1, $2, $3, $4, $5, 0, $6, $7, $8, now(), 0) \
         ON CONFLICT DO NOTHING",
    )
    .bind(Uuid::new_v4())
    .bind(&roll_upper)
    .bind(&credential_id)
    .bind(&passkey_blob)
    .bind(crate::webauthn_user_handle(&roll_upper))
    .bind(
        stored_challenge
            .student_name
            .clone()
            .unwrap_or_else(|| "Unknown".to_string()),
    )
    .bind(primary_transport_label(&payload.credential))
    .bind(transport_labels(&payload.credential))
    .execute(&state.db)
    .await?;

    if inserted.rows_affected() == 0 {
        return Err(AppError::BadRequest("Device already enrolled".to_string()));
    }

    tracing::info!(
        roll_number = %roll_upper,
        session_id = %stored_challenge.session_id,
        "WebAuthn credential enrolled"
    );

    Ok(Json(RegistrationFinishResponse {
        verified: true,
        credential_id,
        message: "Device enrolled successfully".to_string(),
    }))
}

// =================== Authentication Start ===================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthenticationStartRequest {
    pub roll_number: String,
}

pub async fn start_authentication(
    State(state): State<Arc<AppState>>,
    Path(short_code): Path<String>,
    Json(payload): Json<AuthenticationStartRequest>,
) -> Result<impl IntoResponse> {
    let roll_upper = payload.roll_number.to_uppercase();

    let (_short_link, session) = load_active_short_link_and_session(&state.db, &short_code).await?;

    let credential: WebAuthnCredential =
        sqlx::query_as("SELECT * FROM webauthn_credentials WHERE student_id = $1")
            .bind(&roll_upper)
            .fetch_optional(&state.db)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(
                    "No credential found. Please enroll your device first.".to_string(),
                )
            })?;

    if credential.is_suspended {
        return Err(AppError::BadRequest(
            "Your credential has been suspended. Please contact admin.".to_string(),
        ));
    }

    let passkey = decode_passkey(&credential)?;

    let (request_challenge, auth_state) =
        state
            .webauthn
            .start_passkey_authentication(&[passkey])
            .map_err(|e| AppError::Internal(format!("Failed to start authentication: {e}")))?;

    let webauthn_challenge = WebAuthnChallenge {
        id: Uuid::new_v4(),
        student_id: roll_upper,
        challenge: b64url(request_challenge.public_key.challenge.as_ref()),
        challenge_type: WebAuthnChallengeType::Authentication,
        session_id: session.id,
        short_code: Some(short_code.to_lowercase()),
        student_name: None,
        expires_at: Utc::now() + Duration::minutes(5),
        used: false,
        state: Some(
            serde_json::to_value(&auth_state)
                .map_err(|e| AppError::Internal(format!("Failed to store ceremony state: {e}")))?,
        ),
        created_at: Utc::now(),
    };

    insert_webauthn_challenge(&state.db, &webauthn_challenge).await?;

    Ok(Json(request_challenge))
}

// =================== Conditional Authentication ===================

/// Conditional-UI ("passkey autofill") authentication. The student is not
/// known up front, so this uses discoverable credentials: the authenticator
/// returns a user handle, which `finish_authentication` maps back to a roll
/// number. The assertion is verified against that student's stored passkey
/// exactly as in the non-discoverable flow.
pub async fn start_conditional_authentication(
    State(state): State<Arc<AppState>>,
    Path(short_code): Path<String>,
) -> Result<impl IntoResponse> {
    let (_short_link, session) = load_active_short_link_and_session(&state.db, &short_code).await?;

    let (request_challenge, auth_state) = state
        .webauthn
        .start_discoverable_authentication()
        .map_err(|e| AppError::Internal(format!("Failed to start authentication: {e}")))?;

    let webauthn_challenge = WebAuthnChallenge {
        id: Uuid::new_v4(),
        // Empty: the subject is resolved from the assertion's user handle.
        student_id: String::new(),
        challenge: b64url(request_challenge.public_key.challenge.as_ref()),
        challenge_type: WebAuthnChallengeType::Authentication,
        session_id: session.id,
        short_code: Some(short_code.to_lowercase()),
        student_name: None,
        expires_at: Utc::now() + Duration::minutes(5),
        used: false,
        state: Some(
            serde_json::to_value(&auth_state)
                .map_err(|e| AppError::Internal(format!("Failed to store ceremony state: {e}")))?,
        ),
        created_at: Utc::now(),
    };

    insert_webauthn_challenge(&state.db, &webauthn_challenge).await?;

    Ok(Json(request_challenge))
}

/// Base64url-encodes without padding, matching the WebAuthn wire format.
fn b64url(bytes: &[u8]) -> String {
    base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, bytes)
}

/// Deserialises the stored passkey for a credential row.
fn decode_passkey(credential: &WebAuthnCredential) -> Result<webauthn_rs::prelude::Passkey> {
    serde_json::from_slice(&credential.passkey)
        .map_err(|e| AppError::Internal(format!("Stored credential could not be decoded: {e}")))
}

fn primary_transport_label(
    credential: &webauthn_rs::prelude::RegisterPublicKeyCredential,
) -> String {
    transport_labels(credential)
        .first()
        .cloned()
        .unwrap_or_else(|| "platform".to_string())
}

fn transport_labels(credential: &webauthn_rs::prelude::RegisterPublicKeyCredential) -> Vec<String> {
    credential
        .response
        .transports
        .as_ref()
        .map(|transports| {
            transports
                .iter()
                .map(|transport| format!("{transport:?}").to_lowercase())
                .collect()
        })
        .unwrap_or_default()
}

// =================== Authentication Finish ===================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthenticationFinishRequest {
    pub roll_number: Option<String>,
    pub credential: webauthn_rs::prelude::PublicKeyCredential,
    pub student_name: Option<String>,
    // `photoUrl` is deliberately absent: the stored URL is derived from the
    // validated object key. It used to be accepted verbatim and rendered in
    // the admin dashboard.
    pub photo_public_id: Option<String>,
    pub latitude: f64,
    pub longitude: f64,
    pub device_fingerprint: Option<String>,
    pub gps_data: Option<crate::middleware::GpsDataPayload>,
    pub gps_metadata: Option<super::attendance::GpsMetadataPayload>,
    pub face_detected: Option<bool>,
    pub dev_bypass_camera: Option<bool>,
    pub dev_bypass_gps: Option<bool>,
    pub dev_bypass_webauthn: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct AuthenticationFinishResponse {
    pub message: String,
    pub attendance: Option<AttendanceSummary>,
    pub replay_attack: bool,
}

#[derive(Debug, Serialize)]
pub struct AttendanceSummary {
    pub id: String,
    pub student_name: String,
    pub roll_number: String,
    pub distance_from_location: f64,
    pub verified: bool,
    pub captured_at: String,
    pub webauthn_verified: bool,
}

pub async fn finish_authentication(
    State(state): State<Arc<AppState>>,
    Path(short_code): Path<String>,
    axum::Extension(gps_validation): axum::Extension<crate::middleware::GpsValidationResult>,
    axum::Extension(emulator_detection): axum::Extension<
        crate::middleware::EmulatorDetectionResult,
    >,
    axum::Extension(device_integrity): axum::Extension<crate::middleware::DeviceIntegrityResult>,
    // Option<Extension<ConnectInfo<_>>>, not a bare ConnectInfo — see
    // middleware::deny_list_middleware's comment; a strict extractor here
    // would 500 in any test/context that doesn't go through
    // into_make_service_with_connect_info.
    connect_info: Option<Extension<ConnectInfo<std::net::SocketAddr>>>,
    Json(payload): Json<AuthenticationFinishRequest>,
) -> Result<impl IntoResponse> {
    let client_ip = connect_info
        .map(|Extension(ConnectInfo(addr))| addr.ip().to_string())
        .unwrap_or_else(|| "unknown-peer".to_string());
    let sys_config = crate::models::SystemConfig::load(&state.db)
        .await?
        .unwrap_or_default();
    let is_dev_bypass_all = !state.config.is_production()
        && (sys_config.dev_bypass_enabled
            || std::env::var("DEV_BYPASS_ALL").unwrap_or_default() == "true");

    let (_short_link, session) = load_active_short_link_and_session(&state.db, &short_code).await?;
    let session_id = session.id;

    // Consume the ceremony this short code issued. Bound to the short code and
    // single-use; the challenge string in the request body is never trusted to
    // select it.
    let stored_challenge = take_pending_challenge(
        &state.db,
        &short_code,
        WebAuthnChallengeType::Authentication,
    )
    .await?
    .ok_or_else(|| AppError::BadRequest("No valid authentication challenge found".to_string()))?;

    // Resolve the subject. For a targeted ceremony it is fixed by the
    // challenge; for a discoverable ("conditional UI") ceremony it comes from
    // the user handle inside the signed assertion. Either way, never from a
    // roll number in the request body — that override is what let a challenge
    // issued for student A mark student B present.
    let roll_upper = if stored_challenge.student_id.is_empty() {
        let (user_handle, _cred_id) = state
            .webauthn
            .identify_discoverable_authentication(&payload.credential)
            .map_err(|e| {
                tracing::warn!("Discoverable authentication could not be identified: {e}");
                AppError::BadRequest("Passkey could not be identified".to_string())
            })?;

        sqlx::query_scalar::<_, String>(
            "SELECT student_id FROM webauthn_credentials WHERE user_handle = $1",
        )
        .bind(user_handle)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound("No credential found".to_string()))?
    } else {
        stored_challenge.student_id.clone()
    }
    .to_uppercase();

    // Per-identity caps in addition to the router's per-IP student limiter —
    // see the identical rationale in controllers/attendance.rs::submit_attendance.
    if !state
        .rate_limiter
        .roll_number_rate_limit(
            &roll_upper,
            crate::constants::ROLL_NUMBER_SUBMIT_MAX_ATTEMPTS,
            crate::constants::ROLL_NUMBER_SUBMIT_WINDOW_SECS,
        )
        .await
    {
        return Err(AppError::TooManyRequests(
            "Too many attendance attempts for this roll number. Please try again later."
                .to_string(),
        ));
    }

    if let Some(ref fingerprint) = payload.device_fingerprint {
        if !state
            .rate_limiter
            .device_fingerprint_rate_limit(
                fingerprint,
                crate::constants::DEVICE_FINGERPRINT_SUBMIT_MAX_ATTEMPTS,
                crate::constants::DEVICE_FINGERPRINT_SUBMIT_WINDOW_SECS,
            )
            .await
        {
            return Err(AppError::TooManyRequests(
                "Too many attendance attempts from this device. Please try again later."
                    .to_string(),
            ));
        }
    }

    // Get credential
    let stored_credential: WebAuthnCredential =
        sqlx::query_as("SELECT * FROM webauthn_credentials WHERE student_id = $1")
            .bind(&roll_upper)
            .fetch_optional(&state.db)
            .await?
            .ok_or_else(|| AppError::NotFound("No credential found".to_string()))?;

    if stored_credential.is_suspended {
        return Err(AppError::BadRequest("Credential is suspended".to_string()));
    }

    // The actual cryptographic check. This verifies the assertion signature
    // against the enrolled public key, the challenge against the stored
    // ceremony state, the origin, the rpIdHash, the user-verification flag and
    // the signature counter. None of this happened before: `signature` was
    // parsed into a struct field and never read, and the enrolled public key
    // was never loaded back out of the database.
    let passkey = decode_passkey(&stored_credential)?;

    let auth_result = if stored_challenge.student_id.is_empty() {
        let auth_state: webauthn_rs::prelude::DiscoverableAuthentication =
            ceremony_state(&stored_challenge)?;
        let discoverable_key = webauthn_rs::prelude::DiscoverableKey::from(&passkey);
        state.webauthn.finish_discoverable_authentication(
            &payload.credential,
            auth_state,
            &[discoverable_key],
        )
    } else {
        let auth_state: webauthn_rs::prelude::PasskeyAuthentication =
            ceremony_state(&stored_challenge)?;
        state
            .webauthn
            .finish_passkey_authentication(&payload.credential, &auth_state)
    }
    .map_err(|e| {
        tracing::warn!(
            roll_number = %roll_upper,
            session_id = %session_id,
            "WebAuthn assertion rejected: {e}"
        );
        AppError::Unauthorized("Biometric verification failed".to_string())
    })?;

    if !auth_result.user_verified() {
        return Err(AppError::Unauthorized(
            "Biometric verification required. Please use Face ID, Touch ID, or device PIN."
                .to_string(),
        ));
    }

    // webauthn-rs raises this when the authenticator's signature counter went
    // backwards, which means the credential has been cloned.
    if auth_result.needs_update() && auth_result.counter() > 0 {
        tracing::info!(
            roll_number = %roll_upper,
            "WebAuthn credential counter advanced; state will be refreshed"
        );
    }

    // Check for existing attendance
    let already_submitted: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM attendances WHERE session_id = $1 AND roll_number = $2)",
    )
    .bind(session_id)
    .bind(&roll_upper)
    .fetch_one(&state.db)
    .await?;

    if already_submitted {
        return Err(AppError::BadRequest(
            "Attendance already submitted".to_string(),
        ));
    }

    // Counter, user-verification and clone detection are all handled by
    // `finish_passkey_authentication` above. The counter is recorded here only
    // so the admin views keep showing it.
    let counter_i64 = i64::from(auth_result.counter());

    // Get location
    let location: Location = sqlx::query_as("SELECT * FROM locations WHERE id = $1")
        .bind(session.location_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound("Location not found".to_string()))?;

    // Calculate distance
    let distance = crate::utils::calculate_distance(
        location.latitude,
        location.longitude,
        payload.latitude,
        payload.longitude,
    );

    if distance > location.radius_meters
        && !(is_dev_bypass_all && payload.dev_bypass_gps.unwrap_or(false))
    {
        return Err(AppError::BadRequest(format!(
            "You are {}m away from the location (max: {}m)",
            distance, location.radius_meters
        )));
    }

    let has_high_severity_gps = gps_validation
        .anomalies
        .iter()
        .any(|a| a.severity == crate::Severity::High)
        && !(is_dev_bypass_all && payload.dev_bypass_gps.unwrap_or(false));

    let has_security_flags = has_high_severity_gps
        || ((emulator_detection.has_high_severity || emulator_detection.detected)
            && !(is_dev_bypass_all && payload.dev_bypass_camera.unwrap_or(false)));

    let should_flag = has_security_flags || !device_integrity.passed;

    let (device_flag, flag_reason) = if has_high_severity_gps {
        (
            Some(crate::models::AttendanceDeviceFlag::GpsAnomalyDetected),
            Some("GPS anomalies detected".to_string()),
        )
    } else if emulator_detection.detected {
        (
            Some(crate::models::AttendanceDeviceFlag::EmulatorDetected),
            Some("Emulator detected".to_string()),
        )
    } else if !device_integrity.passed {
        (
            Some(crate::models::AttendanceDeviceFlag::IntegrityCheckFailed),
            Some("Integrity checks failed".to_string()),
        )
    } else {
        (None, None)
    };

    // Same as the plain submit path: gate on a validated key, not on a
    // client-supplied URL the direct-upload flow never sends.
    let photo_key: Option<&str> = payload.photo_public_id.as_deref().and_then(|key| {
        match crate::storage::validate_attendance_photo_key(key) {
            Ok(valid) => Some(valid),
            Err(_) => {
                tracing::warn!("Rejected out-of-prefix photo key from client");
                None
            }
        }
    });

    let face_detected_result = if is_dev_bypass_all && payload.dev_bypass_camera.unwrap_or(false) {
        Some(true)
    } else if let Some(photo_public_id) = photo_key {
        match state.storage.provider().download(photo_public_id).await {
            Ok(image_data) => {
                match crate::services::face_detection::detect_faces(&image_data).await {
                    Ok(result) => Some(result.face_detected),
                    Err(_) => None,
                }
            }
            Err(_) => None,
        }
    } else {
        None
    };

    // Create attendance record
    let student_name = payload
        .student_name
        .clone()
        .or_else(|| stored_challenge.student_name.clone())
        .unwrap_or_else(|| "Unknown".to_string());

    let device_fingerprint_hash = payload
        .device_fingerprint
        .as_ref()
        .map(|fp| crate::models::Device::hash_fingerprint(fp));

    // IP-geo sanity cross-check — same rationale as
    // controllers/attendance.rs::submit_attendance.
    let ip_info: IpInfo = crate::services::lookup_ip(&state.http_client, &client_ip)
        .await
        .unwrap_or_else(|_| IpInfo {
            isp: "Unknown".to_string(),
            org: "Unknown".to_string(),
            country: None,
            region: None,
            city: None,
            latitude: None,
            longitude: None,
        });
    let ip_geo_anomaly = match (ip_info.latitude, ip_info.longitude) {
        (Some(ip_lat), Some(ip_lon)) => {
            let ip_distance_m =
                calculate_distance(ip_lat, ip_lon, payload.latitude, payload.longitude);
            if ip_distance_m > crate::constants::IP_GEO_MISMATCH_THRESHOLD_M {
                Some(GpsAnomaly {
                    anomaly_type: GpsAnomalyType::IpGeoMismatch,
                    severity: Severity::Medium,
                    details: Some(format!(
                        "Claimed GPS position is ~{:.0}km from the request IP's approximate location ({})",
                        ip_distance_m / 1000.0,
                        ip_info.country.as_deref().unwrap_or("unknown region")
                    )),
                    detected_at: Utc::now(),
                })
            } else {
                None
            }
        }
        _ => None,
    };

    let mut gps_anomalies = super::attendance::build_gps_anomalies(&gps_validation);
    if let Some(anomaly) = ip_geo_anomaly {
        gps_anomalies.push(anomaly);
    }
    let emulator_flags = super::attendance::build_emulator_flags(&emulator_detection);
    let integrity_checks = super::attendance::build_integrity_checks(&device_integrity);
    let gps_confidence = super::attendance::gps_confidence_from_str(&gps_validation.confidence);

    let device_id = payload.device_fingerprint.as_deref().unwrap_or(&roll_upper);
    super::attendance::track_gps_history(
        &state,
        device_id,
        payload.latitude,
        payload.longitude,
        payload.gps_metadata.as_ref(),
        &mut gps_anomalies,
    )
    .await;

    let flag_reason_final = if is_dev_bypass_all
        && (payload.dev_bypass_camera.unwrap_or(false)
            || payload.dev_bypass_gps.unwrap_or(false)
            || payload.dev_bypass_webauthn.unwrap_or(false))
    {
        Some(format!(
            "Dev bypass used: Camera: {}, GPS: {}, Webauthn: {}",
            payload.dev_bypass_camera.unwrap_or(false),
            payload.dev_bypass_gps.unwrap_or(false),
            payload.dev_bypass_webauthn.unwrap_or(false)
        ))
    } else {
        flag_reason.clone()
    };

    let captured_at = Utc::now();

    let attendance_id: Uuid = sqlx::query_scalar(
        "INSERT INTO attendances (
            id, session_id, student_name, roll_number, photo_url, photo_public_id, photo_hash, photo_reuse_detected,
            student_latitude, student_longitude, distance_from_location, ip_address, user_agent, network_provider, network_org,
            verified, face_detected, device_fingerprint, device_fingerprint_hash, device_first_seen, totp_code, totp_valid,
            device_flag, webauthn_credential_id, webauthn_verified, webauthn_device_type, webauthn_authenticator_attachment,
            webauthn_counter, webauthn_replay_attack, flag_reviewed, flag_reviewed_by, flag_reviewed_at, flagged, flag_reason,
            flag_details, captured_at, gps_accuracy, gps_altitude, gps_altitude_accuracy, gps_speed, gps_heading, gps_timestamp,
            gps_mock_location, gps_provider, gps_anomalies, gps_confidence, emulator_detected, emulator_flags, integrity_checks
        ) VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8,
            $9, $10, $11, $12, $13, $14, $15,
            $16, $17, $18, $19, $20, $21, $22,
            $23, $24, $25, $26, $27,
            $28, $29, $30, $31, $32, $33, $34,
            $35, $36, $37, $38, $39, $40, $41, $42,
            $43, $44, $45, $46, $47, $48, $49
        ) RETURNING id",
    )
    .bind(Uuid::new_v4()) // 1 id
    .bind(session_id) // 2 session_id
    .bind(&student_name) // 3 student_name
    .bind(&roll_upper) // 4 roll_number
    .bind(
        photo_key
            .map(|key| state.storage.provider().get_file_url(key))
            .unwrap_or_default(),
    ) // 5 photo_url (derived, not client-supplied)
    .bind(payload.photo_public_id.clone().unwrap_or_default()) // 6 photo_public_id
    .bind(Option::<String>::None) // 7 photo_hash
    .bind(false) // 8 photo_reuse_detected
    .bind(payload.latitude) // 9 student_latitude
    .bind(payload.longitude) // 10 student_longitude
    .bind(distance) // 11 distance_from_location
    .bind(Option::<String>::None) // 12 ip_address
    .bind(Option::<String>::None) // 13 user_agent
    .bind(Option::<String>::None) // 14 network_provider
    .bind(Option::<String>::None) // 15 network_org
    .bind(true) // 16 verified
    .bind(face_detected_result.or(payload.face_detected).unwrap_or(true)) // 17 face_detected
    .bind(&payload.device_fingerprint) // 18 device_fingerprint
    .bind(&device_fingerprint_hash) // 19 device_fingerprint_hash
    .bind(false) // 20 device_first_seen
    .bind(Option::<String>::None) // 21 totp_code
    .bind(Option::<bool>::None) // 22 totp_valid
    .bind(device_flag) // 23 device_flag
    .bind(stored_credential.id.to_string()) // 24 webauthn_credential_id
    .bind(true) // 25 webauthn_verified
    .bind(Some(crate::models::WebAuthnDeviceType::Unknown)) // 26 webauthn_device_type
    .bind(Some(crate::models::WebAuthnAttachment::CrossPlatform)) // 27 webauthn_authenticator_attachment
    .bind(Some(auth_result.counter() as i32)) // 28 webauthn_counter
    .bind(false) // 29 webauthn_replay_attack (rejected outright above)
    .bind(false) // 30 flag_reviewed
    .bind(Option::<Uuid>::None) // 31 flag_reviewed_by
    .bind(Option::<chrono::DateTime<Utc>>::None) // 32 flag_reviewed_at
    .bind(should_flag) // 33 flagged
    .bind(&flag_reason_final) // 34 flag_reason
    .bind(&flag_reason) // 35 flag_details
    .bind(captured_at) // 36 captured_at
    .bind(payload.gps_data.as_ref().and_then(|g| g.accuracy)) // 37 gps_accuracy
    .bind(payload.gps_data.as_ref().and_then(|g| g.altitude)) // 38 gps_altitude
    .bind(Option::<f64>::None) // 39 gps_altitude_accuracy
    .bind(payload.gps_data.as_ref().and_then(|g| g.speed)) // 40 gps_speed
    .bind(Option::<f64>::None) // 41 gps_heading
    .bind(payload.gps_data.as_ref().and_then(|g| g.timestamp)) // 42 gps_timestamp
    .bind(
        payload
            .gps_data
            .as_ref()
            .and_then(|g| g.mock_location)
            .unwrap_or(false),
    ) // 43 gps_mock_location
    .bind(payload.gps_data.as_ref().and_then(|g| g.provider.clone())) // 44 gps_provider
    .bind(SqlxJson(gps_anomalies)) // 45 gps_anomalies
    .bind(gps_confidence) // 46 gps_confidence
    .bind(emulator_detection.detected) // 47 emulator_detected
    .bind(SqlxJson(emulator_flags)) // 48 emulator_flags
    .bind(SqlxJson(integrity_checks)) // 49 integrity_checks
    .fetch_one(&state.db)
    .await?;

    // Update credential counter and last used
    sqlx::query(
        "UPDATE webauthn_credentials \
         SET counter = $1, last_used_at = now(), last_session_id = $2, sign_count = sign_count + 1 \
         WHERE id = $3",
    )
    .bind(counter_i64)
    .bind(session_id)
    .bind(stored_credential.id)
    .execute(&state.db)
    .await?;

    // Mark challenge as used
    sqlx::query("UPDATE webauthn_challenges SET used = true WHERE id = $1")
        .bind(stored_challenge.id)
        .execute(&state.db)
        .await?;

    Ok(Json(AuthenticationFinishResponse {
        message: "Attendance submitted successfully".to_string(),
        attendance: Some(AttendanceSummary {
            id: attendance_id.to_string(),
            student_name,
            roll_number: roll_upper,
            distance_from_location: distance,
            verified: true,
            captured_at: captured_at.to_rfc3339(),
            webauthn_verified: true,
        }),
        replay_attack: false,
    }))
}

// =================== Upload URL ===================

#[derive(Debug, Serialize)]
pub struct UploadUrlResponse {
    pub upload_url: String,
    pub public_id: String,
}

pub async fn get_upload_url(
    State(state): State<Arc<AppState>>,
    Path(short_code): Path<String>,
) -> Result<impl IntoResponse> {
    let (_short_link, session) = load_active_short_link_and_session(&state.db, &short_code).await?;

    let key = format!(
        "attendance-photos/{}_{}.jpg",
        session.id,
        chrono::Utc::now().timestamp()
    );
    let presigned = state
        .storage
        .provider()
        .get_upload_url(&key, "image/jpeg")
        .await?;

    Ok(Json(UploadUrlResponse {
        upload_url: presigned.upload_url,
        public_id: presigned.public_id,
    }))
}

// =================== Captcha ===================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptchaResponse {
    pub captcha_id: String,
    pub captcha_svg: String,
}

pub async fn get_captcha(
    State(state): State<Arc<AppState>>,
    Path(_short_code): Path<String>,
) -> Result<impl IntoResponse> {
    let issued = crate::utils::captcha::issue(&state.config.jwt_secret)?;

    Ok(Json(CaptchaResponse {
        captcha_id: issued.captcha_id,
        captcha_svg: issued.image_html,
    }))
}

#[cfg(test)]
mod contract_tests {
    use super::*;

    /// Regression test: StudentScan.tsx checks `data.alreadySubmitted`.
    /// Without `#[serde(rename_all = "camelCase")]` this field serializes as
    /// `already_submitted`, so the frontend's already-submitted check would
    /// always read `undefined` and silently fail to short-circuit.
    #[test]
    fn webauthn_status_response_serializes_camel_case() {
        let response = WebAuthnStatusResponse {
            enrolled: true,
            suspended: false,
            already_submitted: true,
            message: Some("Attendance already submitted".to_string()),
            student_name: None,
        };

        let json = serde_json::to_value(&response).unwrap();
        assert_eq!(json["alreadySubmitted"], true);
        assert!(json.get("already_submitted").is_none());
        // student_name is None and should be omitted entirely, not `null`.
        assert!(json.get("studentName").is_none());
    }

    /// Regression test: AuthenticationFinishRequest (the submit endpoint
    /// used whenever a WebAuthn credential is present) must accept
    /// `gpsMetadata` and `faceDetected` — StudentScan.tsx always sends both,
    /// and without these fields they were silently dropped by serde,
    /// disabling GPS position-history tracking and the face-detection
    /// fallback for every passkey-authenticated submission.
    #[test]
    fn authentication_finish_request_deserializes_gps_metadata_and_face_detected() {
        let payload: AuthenticationFinishRequest = serde_json::from_value(serde_json::json!({
            "rollNumber": "CS101",
            "credential": {
                "id": "cred-id",
                "rawId": "e30=",
                "response": {
                    "clientDataJSON": "e30=",
                    "authenticatorData": "e30=",
                    "signature": "e30=",
                },
                "type": "public-key",
            },
            "latitude": 12.9,
            "longitude": 77.6,
            "gpsMetadata": {
                "accuracy": 5.0,
                "isMockLocation": false,
            },
            "faceDetected": true,
        }))
        .unwrap();

        let gps_metadata = payload
            .gps_metadata
            .expect("gpsMetadata should have deserialized");
        assert_eq!(gps_metadata.accuracy, Some(5.0));
        assert_eq!(gps_metadata.is_mock_location, Some(false));
        assert_eq!(payload.face_detected, Some(true));
    }
}
