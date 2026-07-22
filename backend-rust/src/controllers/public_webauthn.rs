use axum::{
    extract::{Json, Path, State},
    response::IntoResponse,
};
use chrono::{Duration, Utc};
use mongodb::{bson::{doc, DateTime as BsonDateTime}, Collection};
use rand::{Rng, RngExt};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::{
    error::{AppError, Result},
    models::{
        Attendance, Location, Session, ShortLink, WebAuthnChallenge, WebAuthnChallengeType,
        WebAuthnCredential,
    },
    utils::calculate_distance,
    AppState,
};

// =================== WebAuthn Status ===================

#[derive(Debug, Serialize)]
pub struct WebAuthnStatusResponse {
    pub enrolled: bool,
    pub suspended: bool,
    pub already_submitted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub student_name: Option<String>,
}

pub async fn get_webauthn_status(
    State(state): State<Arc<AppState>>,
    Path((short_code, roll_number)): Path<(String, String)>,
) -> Result<impl IntoResponse> {
    let db = state.db.database(
        state.config.mongodb_uri.split('/').next_back().unwrap_or("default"),
    );

    let short_links: Collection<ShortLink> = db.collection(ShortLink::collection_name());
    let sessions: Collection<Session> = db.collection(Session::collection_name());
    let credentials: Collection<WebAuthnCredential> = db.collection(WebAuthnCredential::collection_name());
    let attendances: Collection<Attendance> = db.collection(Attendance::collection_name());

    let short_link = short_links
        .find_one(doc! { "shortCode": short_code.to_lowercase(), "isActive": true })
        .await?
        .ok_or_else(|| AppError::NotFound("Invalid session".to_string()))?;

    let session_id = short_link.session_id
        .ok_or_else(|| AppError::NotFound("No session associated with this link".to_string()))?;

    let session = sessions
        .find_one(doc! { "_id": session_id })
        .await?
        .ok_or_else(|| AppError::NotFound("Session not found".to_string()))?;

    if !session.is_active || session.is_expired() {
        return Err(AppError::BadRequest("Session expired".to_string()));
    }

    let roll_upper = roll_number.to_uppercase();

    let credential = credentials.find_one(doc! { "studentId": &roll_upper }).await?;

    let existing_attendance = attendances
        .find_one(doc! { "sessionId": session_id, "rollNumber": &roll_upper })
        .await?;

    if existing_attendance.is_some() {
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
    let db = state.db.database(
        state.config.mongodb_uri.split('/').next_back().unwrap_or("default"),
    );

    let short_links: Collection<ShortLink> = db.collection(ShortLink::collection_name());
    let sessions: Collection<Session> = db.collection(Session::collection_name());
    let credentials: Collection<WebAuthnCredential> = db.collection(WebAuthnCredential::collection_name());
    let challenges: Collection<WebAuthnChallenge> = db.collection(WebAuthnChallenge::collection_name());

    let roll_upper = payload.roll_number.to_uppercase();

    let short_link = short_links
        .find_one(doc! { "shortCode": short_code.to_lowercase(), "isActive": true })
        .await?
        .ok_or_else(|| AppError::NotFound("Invalid session".to_string()))?;

    let session_id = short_link.session_id
        .ok_or_else(|| AppError::NotFound("No session associated with this link".to_string()))?;

    let session = sessions
        .find_one(doc! { "_id": session_id })
        .await?
        .ok_or_else(|| AppError::NotFound("Session not found".to_string()))?;

    if !session.is_active || session.is_expired() {
        return Err(AppError::BadRequest("Session expired".to_string()));
    }

    let existing = credentials.find_one(doc! { "studentId": &roll_upper }).await?;

    if existing.is_some() {
        return Err(AppError::BadRequest(
            "Device already enrolled. Contact admin to re-enroll on a new device.".to_string(),
        ));
    }

    let challenge = generate_challenge();

    let webauthn_challenge = WebAuthnChallenge {
        id: None,
        student_id: roll_upper.clone(),
        challenge: challenge.clone(),
        challenge_type: WebAuthnChallengeType::Registration,
        session_id,
        short_code: Some(short_code.to_lowercase()),
        student_name: Some(payload.student_name.clone()),
        expires_at: Utc::now() + Duration::minutes(5),
        used: false,
        created_at: Utc::now(),
    };

    challenges.insert_one(&webauthn_challenge).await?;

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
            PubKeyCredParam { cred_type: "public-key".to_string(), alg: -7 },
            PubKeyCredParam { cred_type: "public-key".to_string(), alg: -257 },
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
pub struct RegistrationFinishRequest {
    pub roll_number: String,
    pub credential: CredentialResponse,
}

#[derive(Debug, Deserialize)]
pub struct CredentialResponse {
    pub id: String,
    pub raw_id: Option<String>,
    pub response: CredentialResponseData,
    #[serde(rename = "type")]
    pub cred_type: String,
    pub authenticator_attachment: Option<String>,
}

#[derive(Debug, Deserialize)]
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
    let db = state.db.database(
        state.config.mongodb_uri.split('/').next_back().unwrap_or("default"),
    );

    let credentials: Collection<WebAuthnCredential> = db.collection(WebAuthnCredential::collection_name());
    let challenges: Collection<WebAuthnChallenge> = db.collection(WebAuthnChallenge::collection_name());

    let roll_upper = payload.roll_number.to_uppercase();

    let client_challenge = parse_client_challenge(&payload.credential.response.client_data_json)?;

    let stored_challenge = challenges
        .find_one(doc! {
            "studentId": &roll_upper,
            "challenge": &client_challenge,
            "used": false,
            "expiresAt": { "$gt": BsonDateTime::now() }
        })
        .await?
        .ok_or_else(|| AppError::BadRequest("No valid registration challenge found".to_string()))?;

    let existing = credentials.find_one(doc! { "studentId": &roll_upper }).await?;

    if existing.is_some() {
        return Err(AppError::BadRequest("Device already enrolled".to_string()));
    }

    // Extract public key from attestation object
    let public_key = extract_public_key_from_attestation(
        &payload.credential.response.attestation_object,
        &state.config.webauthn.rp_id,
    )?;

    // Decode credential ID from base64url
    let _credential_id_bytes = base64::Engine::decode(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD,
        &payload.credential.id,
    ).unwrap_or_else(|_| payload.credential.id.as_bytes().to_vec());

    let new_credential = WebAuthnCredential {
        id: None,
        student_id: roll_upper,
        credential_id: payload.credential.id.clone(),
        public_key,
        counter: 0,
        device_label: stored_challenge.student_name.unwrap_or_else(|| "Unknown".to_string()),
        device_type: payload.credential.authenticator_attachment.unwrap_or_else(|| "platform".to_string()),
        transports: vec![],
        enrolled_at: Utc::now(),
        enrolled_ip_address: None,
        enrolled_user_agent: None,
        created_by_admin_id: None,
        sign_count: 0,
        last_used_at: None,
        last_session_id: None,
        is_suspended: false,
        suspended_reason: None,
        suspended_at: None,
        suspended_by: None,
        aaguid: None,
        reset_at: None,
        reset_by: None,
    };

    credentials.insert_one(&new_credential).await?;

    challenges
        .update_one(
            doc! { "_id": stored_challenge.id },
            doc! { "$set": { "used": true } },
        )
        .await?;

    Ok(Json(RegistrationFinishResponse {
        verified: true,
        credential_id: payload.credential.id,
        message: "Device enrolled successfully".to_string(),
    }))
}

// =================== Authentication Start ===================

#[derive(Debug, Deserialize)]
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
    let db = state.db.database(
        state.config.mongodb_uri.split('/').next_back().unwrap_or("default"),
    );

    let short_links: Collection<ShortLink> = db.collection(ShortLink::collection_name());
    let sessions: Collection<Session> = db.collection(Session::collection_name());
    let credentials: Collection<WebAuthnCredential> = db.collection(WebAuthnCredential::collection_name());
    let challenges: Collection<WebAuthnChallenge> = db.collection(WebAuthnChallenge::collection_name());

    let roll_upper = payload.roll_number.to_uppercase();

    let short_link = short_links
        .find_one(doc! { "shortCode": short_code.to_lowercase(), "isActive": true })
        .await?
        .ok_or_else(|| AppError::NotFound("Invalid session".to_string()))?;

    let session_id = short_link.session_id
        .ok_or_else(|| AppError::NotFound("No session associated with this link".to_string()))?;

    let session = sessions
        .find_one(doc! { "_id": session_id })
        .await?
        .ok_or_else(|| AppError::NotFound("Session not found".to_string()))?;

    if !session.is_active || session.is_expired() {
        return Err(AppError::BadRequest("Session expired".to_string()));
    }

    let credential = credentials
        .find_one(doc! { "studentId": &roll_upper })
        .await?
        .ok_or_else(|| AppError::NotFound("No credential found. Please enroll your device first.".to_string()))?;

    if credential.is_suspended {
        return Err(AppError::BadRequest(
            "Your credential has been suspended. Please contact admin.".to_string(),
        ));
    }

    let challenge = generate_challenge();

    let webauthn_challenge = WebAuthnChallenge {
        id: None,
        student_id: roll_upper,
        challenge: challenge.clone(),
        challenge_type: WebAuthnChallengeType::Authentication,
        session_id,
        short_code: Some(short_code.to_lowercase()),
        student_name: None,
        expires_at: Utc::now() + Duration::minutes(5),
        used: false,
        created_at: Utc::now(),
    };

    challenges.insert_one(&webauthn_challenge).await?;

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
    let db = state.db.database(
        state.config.mongodb_uri.split('/').next_back().unwrap_or("default"),
    );

    let short_links: Collection<ShortLink> = db.collection(ShortLink::collection_name());
    let sessions: Collection<Session> = db.collection(Session::collection_name());
    let challenges: Collection<WebAuthnChallenge> = db.collection(WebAuthnChallenge::collection_name());

    let short_link = short_links
        .find_one(doc! { "shortCode": short_code.to_lowercase(), "isActive": true })
        .await?
        .ok_or_else(|| AppError::NotFound("Invalid session".to_string()))?;

    let session_id = short_link.session_id
        .ok_or_else(|| AppError::NotFound("No session associated with this link".to_string()))?;

    let session = sessions
        .find_one(doc! { "_id": session_id })
        .await?
        .ok_or_else(|| AppError::NotFound("Session not found".to_string()))?;

    if !session.is_active || session.is_expired() {
        return Err(AppError::BadRequest("Session expired".to_string()));
    }

    let challenge = generate_challenge();

    let webauthn_challenge = WebAuthnChallenge {
        id: None,
        student_id: String::new(),
        challenge: challenge.clone(),
        challenge_type: WebAuthnChallengeType::Authentication,
        session_id,
        short_code: Some(short_code.to_lowercase()),
        student_name: None,
        expires_at: Utc::now() + Duration::minutes(5),
        used: false,
        created_at: Utc::now(),
    };

    challenges.insert_one(&webauthn_challenge).await?;

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
pub struct AuthenticationFinishRequest {
    pub roll_number: Option<String>,
    pub credential: AuthenticationCredentialResponse,
    pub student_name: Option<String>,
    pub photo: Option<String>,
    pub photo_public_id: Option<String>,
    pub latitude: f64,
    pub longitude: f64,
    pub device_fingerprint: Option<String>,
    pub gps_data: Option<crate::middleware::GpsDataPayload>,
}

#[derive(Debug, Deserialize)]
pub struct AuthenticationCredentialResponse {
    pub id: String,
    pub raw_id: Option<String>,
    pub response: AuthResponseData,
    #[serde(rename = "type")]
    pub cred_type: String,
    pub authenticator_attachment: Option<String>,
}

#[derive(Debug, Deserialize)]
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
    Json(payload): Json<AuthenticationFinishRequest>,
) -> Result<impl IntoResponse> {
    let db = state.db.database(
        state.config.mongodb_uri.split('/').next_back().unwrap_or("default"),
    );

    let short_links: Collection<ShortLink> = db.collection(ShortLink::collection_name());
    let sessions: Collection<Session> = db.collection(Session::collection_name());
    let locations: Collection<Location> = db.collection(Location::collection_name());
    let credentials: Collection<WebAuthnCredential> = db.collection(WebAuthnCredential::collection_name());
    let challenges: Collection<WebAuthnChallenge> = db.collection(WebAuthnChallenge::collection_name());
    let attendances: Collection<Attendance> = db.collection(Attendance::collection_name());

    // Find short link
    let short_link = short_links
        .find_one(doc! { "shortCode": short_code.to_lowercase(), "isActive": true })
        .await?
        .ok_or_else(|| AppError::NotFound("Invalid session".to_string()))?;

    let session_id = short_link.session_id
        .ok_or_else(|| AppError::NotFound("No session associated with this link".to_string()))?;

    // Get session
    let session = sessions
        .find_one(doc! { "_id": session_id })
        .await?
        .ok_or_else(|| AppError::NotFound("Session not found".to_string()))?;

    if !session.is_active || session.is_expired() {
        return Err(AppError::BadRequest("Session expired".to_string()));
    }

    // Parse client data to get challenge
    let client_challenge = parse_client_challenge(&payload.credential.response.client_data_json)?;

    // Find stored challenge
    let stored_challenge = challenges
        .find_one(doc! {
            "challenge": &client_challenge,
            "used": false,
            "expiresAt": { "$gt": BsonDateTime::now() }
        })
        .await?
        .ok_or_else(|| AppError::BadRequest("No valid authentication challenge found".to_string()))?;

    // Determine roll number
    let roll_upper = payload.roll_number
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
    let stored_credential = credentials
        .find_one(doc! { "studentId": &roll_upper })
        .await?
        .ok_or_else(|| AppError::NotFound("No credential found".to_string()))?;

    if stored_credential.is_suspended {
        return Err(AppError::BadRequest("Credential is suspended".to_string()));
    }

    // Check for existing attendance
    let existing = attendances
        .find_one(doc! { "sessionId": session_id, "rollNumber": &roll_upper })
        .await?;

    if existing.is_some() {
        return Err(AppError::BadRequest("Attendance already submitted".to_string()));
    }

    // Parse authenticator data
    let auth_data = base64::Engine::decode(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD,
        &payload.credential.response.authenticator_data,
    ).map_err(|e| AppError::BadRequest(format!("Invalid authenticator data: {}", e)))?;

    // Extract counter from authenticator data (bytes 32-36)
    let counter = if auth_data.len() >= 37 {
        u32::from_be_bytes([auth_data[33], auth_data[34], auth_data[35], auth_data[36]])
    } else {
        0
    };

    // Check user verification flag (byte 32 bit 0)
    let user_verified = auth_data.len() > 32 && (auth_data[32] & 0x04) != 0;

    if !user_verified {
        return Err(AppError::Unauthorized(
            "Biometric verification required. Please use Face ID, Touch ID, or device PIN.".to_string()
        ));
    }

    // Counter-based replay attack detection
    let replay_attack = counter > 0 && stored_credential.counter > 0 && counter <= stored_credential.counter;

    if replay_attack {
        // Update credential counter attempted replay
        credentials
            .update_one(
                doc! { "_id": stored_credential.id },
                doc! { "$set": { "counter": counter } },
            )
            .await?;
        
        return Err(AppError::Unauthorized(
            "Security violation detected. Authentication rejected.".to_string()
        ));
    }

    // Get location
    let location = locations
        .find_one(doc! { "_id": session.location_id })
        .await?
        .ok_or_else(|| AppError::NotFound("Location not found".to_string()))?;

    // Calculate distance
    let distance = calculate_distance(
        location.latitude,
        location.longitude,
        payload.latitude,
        payload.longitude,
    );

    if distance > location.radius_meters {
        return Err(AppError::BadRequest(format!(
            "You are {}m away from the location (max: {}m)",
            distance, location.radius_meters
        )));
    }

    // Create attendance record
    let student_name = payload.student_name
        .clone()
        .or_else(|| stored_challenge.student_name.clone())
        .unwrap_or_else(|| "Unknown".to_string());

    let device_fingerprint_hash = payload.device_fingerprint
        .as_ref()
        .map(|fp| crate::models::Device::hash_fingerprint(fp));

    let attendance = Attendance {
        id: None,
        session_id,
        student_name: student_name.clone(),
        roll_number: roll_upper.clone(),
        photo_url: payload.photo.clone().unwrap_or_default(),
        photo_public_id: payload.photo_public_id.clone().unwrap_or_default(),
        photo_hash: None,
        photo_reuse_detected: false,
        student_latitude: payload.latitude,
        student_longitude: payload.longitude,
        distance_from_location: distance,
        ip_address: None,
        user_agent: None,
        network_provider: None,
        network_org: None,
        verified: true,
        face_detected: true,
        device_fingerprint: payload.device_fingerprint.clone(),
        device_fingerprint_hash,
        device_first_seen: false,
        totp_code: None,
        totp_valid: None,
        device_flag: None,
        webauthn_credential_id: Some(payload.credential.id.clone()),
        webauthn_verified: true,
        webauthn_device_type: payload.credential.authenticator_attachment.as_ref().map(|_| {
            crate::models::WebAuthnDeviceType::Unknown
        }),
        webauthn_authenticator_attachment: payload.credential.authenticator_attachment.as_ref().map(|a| {
            match a.as_str() {
                "platform" => crate::models::WebAuthnAttachment::Platform,
                _ => crate::models::WebAuthnAttachment::CrossPlatform,
            }
        }),
        webauthn_counter: Some(counter as i32),
        webauthn_replay_attack: replay_attack,
        flag_reviewed: false,
        flag_reviewed_by: None,
        flag_reviewed_at: None,
        flagged: false,
        flag_reason: None,
        flag_details: None,
        captured_at: Utc::now(),
        gps_accuracy: payload.gps_data.as_ref().and_then(|g| g.accuracy),
        gps_altitude: payload.gps_data.as_ref().and_then(|g| g.altitude),
        gps_altitude_accuracy: None,
        gps_speed: payload.gps_data.as_ref().and_then(|g| g.speed),
        gps_heading: None,
        gps_timestamp: payload.gps_data.as_ref().and_then(|g| g.timestamp),
        gps_mock_location: payload.gps_data.as_ref().and_then(|g| g.mock_location).unwrap_or(false),
        gps_provider: payload.gps_data.as_ref().and_then(|g| g.provider.clone()),
        gps_anomalies: vec![],
        gps_confidence: None,
        emulator_detected: false,
        emulator_flags: vec![],
        integrity_checks: vec![],
    };

    let result = attendances.insert_one(&attendance).await?;
    let attendance_id = result
        .inserted_id
        .as_object_id()
        .ok_or_else(|| AppError::Internal("Failed to get inserted ID".to_string()))?;

    // Update credential counter and last used
    credentials
        .update_one(
            doc! { "_id": stored_credential.id },
            doc! {
                "$set": {
                    "counter": counter,
                    "lastUsedAt": BsonDateTime::from_millis(Utc::now().timestamp_millis()),
                    "lastSessionId": session_id,
                    "signCount": stored_credential.sign_count + 1
                }
            },
        )
        .await?;

    // Mark challenge as used
    challenges
        .update_one(
            doc! { "_id": stored_challenge.id },
            doc! { "$set": { "used": true } },
        )
        .await?;

    Ok(Json(AuthenticationFinishResponse {
        message: "Attendance submitted successfully".to_string(),
        attendance: Some(AttendanceSummary {
            id: attendance_id.to_hex(),
            student_name,
            roll_number: roll_upper,
            distance_from_location: distance,
            verified: true,
            captured_at: Utc::now().to_rfc3339(),
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
    let db = state.db.database(
        state.config.mongodb_uri.split('/').next_back().unwrap_or("default"),
    );

    let short_links: Collection<ShortLink> = db.collection(ShortLink::collection_name());
    let sessions: Collection<Session> = db.collection(Session::collection_name());

    let short_link = short_links
        .find_one(doc! { "shortCode": short_code.to_lowercase(), "isActive": true })
        .await?
        .ok_or_else(|| AppError::NotFound("Invalid session".to_string()))?;

    let session_id = short_link.session_id
        .ok_or_else(|| AppError::NotFound("No session associated with this link".to_string()))?;

    let session = sessions
        .find_one(doc! { "_id": session_id })
        .await?
        .ok_or_else(|| AppError::NotFound("Session not found".to_string()))?;

    if !session.is_active || session.is_expired() {
        return Err(AppError::BadRequest("Session expired".to_string()));
    }

    let key = format!("attendance-photos/{}_{}.jpg", session_id.to_hex(), chrono::Utc::now().timestamp());
    let presigned = state.storage.provider().get_upload_url(&key, "image/jpeg").await?;

    Ok(Json(UploadUrlResponse {
        upload_url: presigned.upload_url,
        public_id: presigned.public_id,
    }))
}

// =================== Captcha ===================

#[derive(Debug, Serialize)]
pub struct CaptchaResponse {
    pub captcha_id: String,
    pub captcha_url: String,
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
        captcha_url: format!(
            "data:image/svg+xml;base64,{}",
            base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                format!(
                    r#"<svg xmlns="http://www.w3.org/2000/svg" width="150" height="50"><text x="10" y="35" font-size="30">{}</text></svg>"#,
                    captcha_text
                )
            )
        ),
    }))
}

// =================== Submit Attendance ===================

#[derive(Debug, Deserialize)]
pub struct SubmitAttendanceRequest {
    pub roll_number: String,
    pub student_name: String,
    pub photo: String,
    pub photo_public_id: Option<String>,
    pub latitude: f64,
    pub longitude: f64,
    pub device_fingerprint: Option<String>,
    pub totp_code: Option<String>,
    pub captcha_answer: String,
    pub captcha_id: String,
    pub gps_data: Option<crate::middleware::GpsDataPayload>,
}

pub async fn submit_attendance(
    State(state): State<Arc<AppState>>,
    Path(short_code): Path<String>,
    Json(payload): Json<SubmitAttendanceRequest>,
) -> Result<impl IntoResponse> {
    let db = state.db.database(
        state.config.mongodb_uri.split('/').next_back().unwrap_or("default"),
    );

    let short_links: Collection<ShortLink> = db.collection(ShortLink::collection_name());
    let sessions: Collection<Session> = db.collection(Session::collection_name());
    let locations: Collection<Location> = db.collection(Location::collection_name());
    let attendances: Collection<Attendance> = db.collection(Attendance::collection_name());

    let short_link = short_links
        .find_one(doc! { "shortCode": short_code.to_lowercase(), "isActive": true })
        .await?
        .ok_or_else(|| AppError::NotFound("Invalid session".to_string()))?;

    let session_id = short_link.session_id
        .ok_or_else(|| AppError::NotFound("No session associated with this link".to_string()))?;

    let session = sessions
        .find_one(doc! { "_id": session_id })
        .await?
        .ok_or_else(|| AppError::NotFound("Session not found".to_string()))?;

    if !session.is_active || session.is_expired() {
        return Err(AppError::BadRequest("Session expired".to_string()));
    }

    // Verify captcha
    verify_captcha(&payload.captcha_id, &payload.captcha_answer)?;

    let roll_upper = payload.roll_number.to_uppercase();

    // Check for existing attendance
    let existing = attendances
        .find_one(doc! { "sessionId": session_id, "rollNumber": &roll_upper })
        .await?;

    if existing.is_some() {
        return Err(AppError::BadRequest("Attendance already submitted".to_string()));
    }

    // Get location
    let location = locations
        .find_one(doc! { "_id": session.location_id })
        .await?
        .ok_or_else(|| AppError::NotFound("Location not found".to_string()))?;

    // Calculate distance
    let distance = calculate_distance(
        location.latitude,
        location.longitude,
        payload.latitude,
        payload.longitude,
    );

    if distance > location.radius_meters {
        return Err(AppError::BadRequest(format!(
            "You are {}m away from the location (max: {}m)",
            distance, location.radius_meters
        )));
    }

    let device_fingerprint_hash = payload.device_fingerprint
        .as_ref()
        .map(|fp| crate::models::Device::hash_fingerprint(fp));

    let attendance = Attendance {
        id: None,
        session_id,
        student_name: payload.student_name.clone(),
        roll_number: roll_upper.clone(),
        photo_url: payload.photo.clone(),
        photo_public_id: payload.photo_public_id.clone().unwrap_or_default(),
        photo_hash: None,
        photo_reuse_detected: false,
        student_latitude: payload.latitude,
        student_longitude: payload.longitude,
        distance_from_location: distance,
        ip_address: None,
        user_agent: None,
        network_provider: None,
        network_org: None,
        verified: true,
        face_detected: true,
        device_fingerprint: payload.device_fingerprint.clone(),
        device_fingerprint_hash,
        device_first_seen: false,
        totp_code: payload.totp_code.clone(),
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
        gps_accuracy: payload.gps_data.as_ref().and_then(|g| g.accuracy),
        gps_altitude: payload.gps_data.as_ref().and_then(|g| g.altitude),
        gps_altitude_accuracy: None,
        gps_speed: payload.gps_data.as_ref().and_then(|g| g.speed),
        gps_heading: None,
        gps_timestamp: payload.gps_data.as_ref().and_then(|g| g.timestamp),
        gps_mock_location: payload.gps_data.as_ref().and_then(|g| g.mock_location).unwrap_or(false),
        gps_provider: payload.gps_data.as_ref().and_then(|g| g.provider.clone()),
        gps_anomalies: vec![],
        gps_confidence: None,
        emulator_detected: false,
        emulator_flags: vec![],
        integrity_checks: vec![],
    };

    let result = attendances.insert_one(&attendance).await?;
    let attendance_id = result
        .inserted_id
        .as_object_id()
        .ok_or_else(|| AppError::Internal("Failed to get inserted ID".to_string()))?;

    Ok(Json(serde_json::json!({
        "message": "Attendance submitted successfully",
        "attendance": {
            "id": attendance_id.to_hex(),
            "studentName": payload.student_name,
            "rollNumber": roll_upper,
            "distanceFromLocation": distance,
            "verified": true,
            "capturedAt": Utc::now().to_rfc3339(),
            "webauthnVerified": false
        }
    })))
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

fn verify_captcha(captcha_id: &str, answer: &str) -> Result<()> {
    let parts: Vec<&str> = captcha_id.split('.').collect();
    if parts.len() != 2 {
        return Err(AppError::BadRequest("Invalid captcha ID".to_string()));
    }

    let timestamp: i64 = parts[0]
        .parse()
        .map_err(|_| AppError::BadRequest("Invalid captcha timestamp".to_string()))?;

    // Check expiry (5 minutes)
    let now = chrono::Utc::now().timestamp_millis();
    if now - timestamp > 5 * 60 * 1000 {
        return Err(AppError::BadRequest("Captcha expired".to_string()));
    }

    // Verify signature
    let expected = sign_captcha(answer, timestamp);
    if parts[1] != expected {
        return Err(AppError::BadRequest("Incorrect captcha".to_string()));
    }

    Ok(())
}

fn extract_public_key_from_attestation(attestation_object: &str, _rp_id: &str) -> Result<Vec<u8>> {
    let decoded = base64::Engine::decode(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD,
        attestation_object,
    ).map_err(|e| AppError::BadRequest(format!("Invalid attestation object: {}", e)))?;

    let cbor_value: ciborium::Value = ciborium::from_reader(&decoded[..])
        .map_err(|e| AppError::BadRequest(format!("CBOR parsing failed: {}", e)))?;

    let cbor_map = cbor_value.as_map()
        .ok_or_else(|| AppError::BadRequest("Attestation object is not a CBOR map".to_string()))?;

    let mut auth_data: Option<Vec<u8>> = None;

    for (key, value) in cbor_map.iter() {
        if let Some(key_str) = key.as_text() {
            match key_str {
                "authData" => {
                    if let ciborium::Value::Bytes(bytes) = value {
                        auth_data = Some(bytes.clone());
                    }
                }
                _ => {}
            }
        }
    }

    let auth_data = auth_data.ok_or_else(|| AppError::BadRequest("Missing authData".to_string()))?;

    if auth_data.len() < 55 {
        return Err(AppError::BadRequest("authData too short".to_string()));
    }

    let offset = 37 + auth_data[37] as usize;
    
    if auth_data.len() < offset + 2 {
        return Err(AppError::BadRequest("authData missing credential data".to_string()));
    }

    let credential_id_len = ((auth_data[offset] as usize) << 8) | (auth_data[offset + 1] as usize);
    let pubkey_offset = offset + 2 + credential_id_len;

    if auth_data.len() < pubkey_offset {
        return Err(AppError::BadRequest("authData missing public key".to_string()));
    }

    let pubkey_cbor: ciborium::Value = ciborium::from_reader(&auth_data[pubkey_offset..])
        .map_err(|e| AppError::BadRequest(format!("Public key CBOR parsing failed: {}", e)))?;

    let mut pubkey_bytes = Vec::new();
    ciborium::into_writer(&pubkey_cbor, &mut pubkey_bytes)
        .map_err(|e| AppError::BadRequest(format!("Failed to serialize public key: {}", e)))?;

    Ok(pubkey_bytes)
}
