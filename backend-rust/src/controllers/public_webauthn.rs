use axum::{
    extract::{Json, Path, State},
    response::IntoResponse,
};
use chrono::{Duration, Utc};
use rand::{Rng, RngExt};
use serde::{Deserialize, Serialize};
use sqlx::types::Json as SqlxJson;
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    error::{AppError, Result},
    models::{
        Location, Session, ShortLink, WebAuthnChallenge, WebAuthnChallengeType, WebAuthnCredential,
    },
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
         (id, student_id, challenge, challenge_type, session_id, short_code, student_name, expires_at, used, created_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
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
    .bind(challenge.created_at)
    .execute(pool)
    .await?;
    Ok(())
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

#[derive(Debug, Serialize)]
pub struct RegistrationOptionsResponse {
    pub challenge: String,
    pub rp: RpInfo,
    pub user: UserInfo,
    pub pub_key_cred_params: Vec<PubKeyCredParam>,
    pub authenticator_selection: AuthenticatorSelection,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attestation: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RpInfo {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct UserInfo {
    pub id: String,
    pub name: String,
    pub display_name: String,
}

#[derive(Debug, Serialize)]
pub struct PubKeyCredParam {
    #[serde(rename = "type")]
    pub cred_type: String,
    pub alg: i32,
}

#[derive(Debug, Serialize)]
pub struct AuthenticatorSelection {
    pub authenticator_attachment: Option<String>,
    pub resident_key: String,
    pub require_resident_key: bool,
    pub user_verification: String,
}

pub async fn start_registration(
    State(state): State<Arc<AppState>>,
    Path(short_code): Path<String>,
    Json(payload): Json<RegistrationStartRequest>,
) -> Result<impl IntoResponse> {
    let roll_upper = payload.roll_number.to_uppercase();

    let (_short_link, session) = load_active_short_link_and_session(&state.db, &short_code).await?;

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

    let challenge = generate_challenge();

    let webauthn_challenge = WebAuthnChallenge {
        id: Uuid::new_v4(),
        student_id: roll_upper.clone(),
        challenge: challenge.clone(),
        challenge_type: WebAuthnChallengeType::Registration,
        session_id: session.id,
        short_code: Some(short_code.to_lowercase()),
        student_name: Some(payload.student_name.clone()),
        expires_at: Utc::now() + Duration::minutes(5),
        used: false,
        created_at: Utc::now(),
    };

    insert_webauthn_challenge(&state.db, &webauthn_challenge).await?;

    let options = RegistrationOptionsResponse {
        challenge: challenge.clone(),
        rp: RpInfo {
            id: state.config.webauthn.rp_id.clone(),
            name: state.config.webauthn.rp_name.clone(),
        },
        user: UserInfo {
            id: roll_upper.clone(),
            name: roll_upper.clone(),
            display_name: payload.student_name,
        },
        pub_key_cred_params: vec![
            PubKeyCredParam {
                cred_type: "public-key".to_string(),
                alg: -7,
            },
            PubKeyCredParam {
                cred_type: "public-key".to_string(),
                alg: -257,
            },
        ],
        authenticator_selection: AuthenticatorSelection {
            authenticator_attachment: Some("platform".to_string()),
            resident_key: "required".to_string(),
            require_resident_key: true,
            user_verification: "required".to_string(),
        },
        timeout: Some(60000),
        attestation: Some("direct".to_string()),
    };

    Ok(Json(options))
}

// =================== Registration Finish ===================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistrationFinishRequest {
    pub roll_number: String,
    pub credential: CredentialResponse,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialResponse {
    pub id: String,
    pub raw_id: Option<String>,
    pub response: CredentialResponseData,
    #[serde(rename = "type")]
    pub cred_type: String,
    pub authenticator_attachment: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialResponseData {
    pub client_data_json: String,
    pub attestation_object: String,
}

#[derive(Debug, Serialize)]
pub struct RegistrationFinishResponse {
    pub verified: bool,
    pub credential_id: String,
    pub message: String,
}

pub async fn finish_registration(
    State(state): State<Arc<AppState>>,
    Path(_short_code): Path<String>,
    Json(payload): Json<RegistrationFinishRequest>,
) -> Result<impl IntoResponse> {
    let roll_upper = payload.roll_number.to_uppercase();

    let client_challenge = parse_client_challenge(&payload.credential.response.client_data_json)?;

    let stored_challenge: WebAuthnChallenge = sqlx::query_as(
        "SELECT * FROM webauthn_challenges \
         WHERE student_id = $1 AND challenge = $2 AND used = false AND expires_at > now()",
    )
    .bind(&roll_upper)
    .bind(&client_challenge)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::BadRequest("No valid registration challenge found".to_string()))?;

    let existing_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM webauthn_credentials WHERE student_id = $1)",
    )
    .bind(&roll_upper)
    .fetch_one(&state.db)
    .await?;

    if existing_exists {
        return Err(AppError::BadRequest("Device already enrolled".to_string()));
    }

    // Extract public key from attestation object
    let public_key = extract_public_key_from_attestation(
        &payload.credential.response.attestation_object,
        &state.config.webauthn.rp_id,
    )?;

    sqlx::query(
        "INSERT INTO webauthn_credentials \
         (id, student_id, credential_id, public_key, counter, device_label, device_type, transports, enrolled_at, sign_count) \
         VALUES ($1, $2, $3, $4, 0, $5, $6, $7, now(), 0)",
    )
    .bind(Uuid::new_v4())
    .bind(&roll_upper)
    .bind(&payload.credential.id)
    .bind(&public_key)
    .bind(
        stored_challenge
            .student_name
            .clone()
            .unwrap_or_else(|| "Unknown".to_string()),
    )
    .bind(
        payload
            .credential
            .authenticator_attachment
            .clone()
            .unwrap_or_else(|| "platform".to_string()),
    )
    .bind(Vec::<String>::new())
    .execute(&state.db)
    .await?;

    sqlx::query("UPDATE webauthn_challenges SET used = true WHERE id = $1")
        .bind(stored_challenge.id)
        .execute(&state.db)
        .await?;

    Ok(Json(RegistrationFinishResponse {
        verified: true,
        credential_id: payload.credential.id,
        message: "Device enrolled successfully".to_string(),
    }))
}

// =================== Authentication Start ===================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthenticationStartRequest {
    pub roll_number: String,
}

#[derive(Debug, Serialize)]
pub struct AuthenticationOptionsResponse {
    pub challenge: String,
    pub timeout: u32,
    pub rp_id: String,
    pub allow_credentials: Vec<AllowCredential>,
    pub user_verification: String,
}

#[derive(Debug, Serialize)]
pub struct AllowCredential {
    pub id: String,
    #[serde(rename = "type")]
    pub cred_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transports: Option<Vec<String>>,
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

    let challenge = generate_challenge();

    let webauthn_challenge = WebAuthnChallenge {
        id: Uuid::new_v4(),
        student_id: roll_upper,
        challenge: challenge.clone(),
        challenge_type: WebAuthnChallengeType::Authentication,
        session_id: session.id,
        short_code: Some(short_code.to_lowercase()),
        student_name: None,
        expires_at: Utc::now() + Duration::minutes(5),
        used: false,
        created_at: Utc::now(),
    };

    insert_webauthn_challenge(&state.db, &webauthn_challenge).await?;

    let options = AuthenticationOptionsResponse {
        challenge: challenge.clone(),
        timeout: 60000,
        rp_id: state.config.webauthn.rp_id.clone(),
        allow_credentials: vec![AllowCredential {
            id: credential.credential_id,
            cred_type: "public-key".to_string(),
            transports: Some(credential.transports),
        }],
        user_verification: "required".to_string(),
    };

    Ok(Json(options))
}

// =================== Conditional Authentication ===================

pub async fn start_conditional_authentication(
    State(state): State<Arc<AppState>>,
    Path(short_code): Path<String>,
) -> Result<impl IntoResponse> {
    let (_short_link, session) = load_active_short_link_and_session(&state.db, &short_code).await?;

    let challenge = generate_challenge();

    let webauthn_challenge = WebAuthnChallenge {
        id: Uuid::new_v4(),
        student_id: String::new(),
        challenge: challenge.clone(),
        challenge_type: WebAuthnChallengeType::Authentication,
        session_id: session.id,
        short_code: Some(short_code.to_lowercase()),
        student_name: None,
        expires_at: Utc::now() + Duration::minutes(5),
        used: false,
        created_at: Utc::now(),
    };

    insert_webauthn_challenge(&state.db, &webauthn_challenge).await?;

    let options = AuthenticationOptionsResponse {
        challenge: challenge.clone(),
        timeout: 60000,
        rp_id: state.config.webauthn.rp_id.clone(),
        allow_credentials: vec![],
        user_verification: "required".to_string(),
    };

    Ok(Json(options))
}

// =================== Authentication Finish ===================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthenticationFinishRequest {
    pub roll_number: Option<String>,
    pub credential: AuthenticationCredentialResponse,
    pub student_name: Option<String>,
    pub photo_url: Option<String>,
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthenticationCredentialResponse {
    pub id: String,
    pub raw_id: Option<String>,
    pub response: AuthResponseData,
    #[serde(rename = "type")]
    pub cred_type: String,
    pub authenticator_attachment: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthResponseData {
    pub client_data_json: String,
    pub authenticator_data: String,
    pub signature: String,
    pub user_handle: Option<String>,
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
    Json(payload): Json<AuthenticationFinishRequest>,
) -> Result<impl IntoResponse> {
    let sys_config = crate::models::SystemConfig::load(&state.db)
        .await?
        .unwrap_or_default();
    let is_dev_bypass_all = sys_config.dev_bypass_enabled
        || std::env::var("DEV_BYPASS_ALL").unwrap_or_default() == "true";

    let (_short_link, session) = load_active_short_link_and_session(&state.db, &short_code).await?;
    let session_id = session.id;

    // Parse client data to get challenge
    let client_challenge = parse_client_challenge(&payload.credential.response.client_data_json)?;

    // Find stored challenge
    let stored_challenge: WebAuthnChallenge = sqlx::query_as(
        "SELECT * FROM webauthn_challenges WHERE challenge = $1 AND used = false AND expires_at > now()",
    )
    .bind(&client_challenge)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::BadRequest("No valid authentication challenge found".to_string()))?;

    // Determine roll number
    let roll_upper = payload
        .roll_number
        .clone()
        .or_else(|| {
            if stored_challenge.student_id.is_empty() {
                None
            } else {
                Some(stored_challenge.student_id.clone())
            }
        })
        .ok_or_else(|| AppError::BadRequest("Roll number required".to_string()))?
        .to_uppercase();

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

    // Parse authenticator data
    let auth_data = base64::Engine::decode(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD,
        &payload.credential.response.authenticator_data,
    )
    .map_err(|e| AppError::BadRequest(format!("Invalid authenticator data: {}", e)))?;

    // Extract counter from authenticator data (bytes 32-36)
    let counter: u32 = if auth_data.len() >= 37 {
        u32::from_be_bytes([auth_data[33], auth_data[34], auth_data[35], auth_data[36]])
    } else {
        0
    };

    // Check user verification flag (byte 32 bit 0)
    let user_verified = auth_data.len() > 32 && (auth_data[32] & 0x04) != 0;

    if !user_verified {
        return Err(AppError::Unauthorized(
            "Biometric verification required. Please use Face ID, Touch ID, or device PIN."
                .to_string(),
        ));
    }

    // Counter-based replay attack detection (stored counter is i64; DB has no unsigned type)
    let counter_i64 = counter as i64;
    let replay_attack = counter_i64 > 0
        && stored_credential.counter > 0
        && counter_i64 <= stored_credential.counter;

    if replay_attack {
        sqlx::query("UPDATE webauthn_credentials SET counter = $1 WHERE id = $2")
            .bind(counter_i64)
            .bind(stored_credential.id)
            .execute(&state.db)
            .await?;

        return Err(AppError::Unauthorized(
            "Security violation detected. Authentication rejected.".to_string(),
        ));
    }

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

    let face_detected_result = if is_dev_bypass_all && payload.dev_bypass_camera.unwrap_or(false) {
        Some(true)
    } else if let (Some(photo_public_id), true) =
        (&payload.photo_public_id, payload.photo_url.is_some())
    {
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

    let mut gps_anomalies = super::attendance::build_gps_anomalies(&gps_validation);
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
    .bind(payload.photo_url.clone().unwrap_or_default()) // 5 photo_url
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
    .bind(Some(counter as i32)) // 28 webauthn_counter
    .bind(replay_attack) // 29 webauthn_replay_attack
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
    State(_state): State<Arc<AppState>>,
    Path(_short_code): Path<String>,
) -> Result<impl IntoResponse> {
    let captcha_text = generate_captcha_text(6);
    let timestamp = chrono::Utc::now().timestamp_millis();
    let signature = sign_captcha(&captcha_text, timestamp);

    Ok(Json(CaptchaResponse {
        captcha_id: format!("{}.{}", timestamp, signature),
        captcha_svg: format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="150" height="50"><text x="10" y="35" font-size="30">{}</text></svg>"#,
            captcha_text
        ),
    }))
}

// =================== Helper Functions ===================

fn generate_challenge() -> String {
    let mut rng = rand::rng();
    let mut bytes = [0u8; 32];
    rng.fill_bytes(&mut bytes);
    base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, bytes)
}

fn parse_client_challenge(client_data_json: &str) -> Result<String> {
    let decoded = base64::Engine::decode(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD,
        client_data_json,
    )
    .map_err(|e| AppError::BadRequest(format!("Invalid clientDataJSON: {}", e)))?;

    let json_str = String::from_utf8(decoded)
        .map_err(|e| AppError::BadRequest(format!("Invalid UTF-8: {}", e)))?;

    let json: serde_json::Value = serde_json::from_str(&json_str)
        .map_err(|e| AppError::BadRequest(format!("Invalid JSON: {}", e)))?;

    json.get("challenge")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| AppError::BadRequest("No challenge in clientData".to_string()))
}

fn generate_captcha_text(length: usize) -> String {
    let chars = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let mut rng = rand::rng();
    (0..length)
        .map(|_| chars.chars().nth(rng.random_range(0..chars.len())).unwrap())
        .collect()
}

fn sign_captcha(text: &str, timestamp: i64) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(format!("{}:{}", text.to_lowercase(), timestamp).as_bytes());
    hex::encode(hasher.finalize())
}

fn extract_public_key_from_attestation(attestation_object: &str, _rp_id: &str) -> Result<Vec<u8>> {
    let decoded = base64::Engine::decode(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD,
        attestation_object,
    )
    .map_err(|e| AppError::BadRequest(format!("Invalid attestation object: {}", e)))?;

    let cbor_value: ciborium::Value = ciborium::from_reader(&decoded[..])
        .map_err(|e| AppError::BadRequest(format!("CBOR parsing failed: {}", e)))?;

    let cbor_map = cbor_value
        .as_map()
        .ok_or_else(|| AppError::BadRequest("Attestation object is not a CBOR map".to_string()))?;

    let mut auth_data: Option<Vec<u8>> = None;

    for (key, value) in cbor_map.iter() {
        if let Some("authData") = key.as_text() {
            if let ciborium::Value::Bytes(bytes) = value {
                auth_data = Some(bytes.clone());
            }
        }
    }

    let auth_data =
        auth_data.ok_or_else(|| AppError::BadRequest("Missing authData".to_string()))?;

    if auth_data.len() < 55 {
        return Err(AppError::BadRequest("authData too short".to_string()));
    }

    let offset = 37 + auth_data[37] as usize;

    if auth_data.len() < offset + 2 {
        return Err(AppError::BadRequest(
            "authData missing credential data".to_string(),
        ));
    }

    let credential_id_len = ((auth_data[offset] as usize) << 8) | (auth_data[offset + 1] as usize);
    let pubkey_offset = offset + 2 + credential_id_len;

    if auth_data.len() < pubkey_offset {
        return Err(AppError::BadRequest(
            "authData missing public key".to_string(),
        ));
    }

    let pubkey_cbor: ciborium::Value = ciborium::from_reader(&auth_data[pubkey_offset..])
        .map_err(|e| AppError::BadRequest(format!("Public key CBOR parsing failed: {}", e)))?;

    let mut pubkey_bytes = Vec::new();
    ciborium::into_writer(&pubkey_cbor, &mut pubkey_bytes)
        .map_err(|e| AppError::BadRequest(format!("Failed to serialize public key: {}", e)))?;

    Ok(pubkey_bytes)
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
                "response": {
                    "clientDataJson": "e30=",
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
