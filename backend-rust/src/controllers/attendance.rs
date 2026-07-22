use axum::{
    extract::{ConnectInfo, Extension, Json, Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::Utc;
use hmac::{Hmac, KeyInit, Mac};
use mongodb::{bson::doc, Collection};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::sync::Arc;

use crate::{
    constants::*,
    error::{AppError, Result},
    middleware::{
        record_device_success, DeviceCheckResult, DeviceIntegrityResult, EmulatorDetectionResult,
        GpsValidationResult,
    },
    models::{
        Attendance, AttendanceDeviceFlag, Device, EmulatorFlag, EmulatorFlagType, GpsAnomaly,
        GpsAnomalyType, GpsConfidence, IntegrityCheck, IntegrityCheckType, Location, PhotoHash,
        Session, WebAuthnCredential, Severity,
    },
    services::{compute_image_hash, detect_faces, GpsPositionEntry, IpInfo},
    utils::{calculate_distance, is_same_photo},
};

#[derive(Debug, Deserialize)]
pub struct SubmitAttendanceRequest {
    pub student_name: String,
    pub roll_number: String,
    pub photo_url: Option<String>,
    pub photo_public_id: Option<String>,
    #[serde(default)]
    pub direct_upload: bool,
    pub latitude: f64,
    pub longitude: f64,
    pub device_fingerprint: Option<String>,
    pub totp_code: Option<String>,
    pub gps_data: Option<crate::middleware::GpsDataPayload>,
    pub webauthn_verified: Option<bool>,
    pub webauthn_credential_id: Option<String>,
    pub face_detected: Option<bool>,
    pub captcha_answer: Option<String>,
    pub captcha_id: Option<String>,
    pub gps_metadata: Option<GpsMetadataPayload>,
}

fn verify_captcha(captcha_answer: &str, captcha_id: &str, jwt_secret: &str) -> Result<()> {
    let parts: Vec<&str> = captcha_id.split('.').collect();
    if parts.len() != 2 {
        return Err(AppError::BadRequest(
            "Invalid captcha ID format".to_string(),
        ));
    }

    let timestamp: i64 = parts[0]
        .parse()
        .map_err(|_| AppError::BadRequest("Invalid captcha timestamp".to_string()))?;

    const FIVE_MINUTES_MS: i64 = 5 * 60 * 1000;
    let now = chrono::Utc::now().timestamp_millis();
    if now - timestamp > FIVE_MINUTES_MS {
        return Err(AppError::BadRequest(
            "Captcha expired. Please refresh and try again.".to_string(),
        ));
    }

    let mut mac = Hmac::<Sha256>::new_from_slice(jwt_secret.as_bytes())
        .map_err(|_| AppError::Internal("Failed to create HMAC".to_string()))?;
    mac.update(format!("{}:{}", captcha_answer.to_lowercase(), timestamp).as_bytes());
    let expected_signature = hex::encode(mac.finalize().into_bytes());

    if expected_signature != parts[1] {
        return Err(AppError::BadRequest(
            "Incorrect captcha. Please try again.".to_string(),
        ));
    }

    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct GpsMetadataPayload {
    pub accuracy: Option<f64>,
    pub altitude: Option<f64>,
    pub altitude_accuracy: Option<f64>,
    pub speed: Option<f64>,
    pub heading: Option<f64>,
    pub timestamp: Option<i64>,
    #[serde(rename = "isMockLocation")]
    pub is_mock_location: Option<bool>,
    pub provider: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AttendanceResponse {
    #[serde(rename = "_id")]
    pub id: String,
    #[serde(rename = "sessionId")]
    pub session_id: String,
    pub student_name: String,
    pub roll_number: String,
    pub verified: bool,
    #[serde(rename = "distanceFromLocation")]
    pub distance_from_location: f64,
}

pub async fn validate_token(
    State(state): State<Arc<crate::AppState>>,
    Path(token): Path<String>,
) -> Result<impl IntoResponse> {
    let collection: Collection<Session> = state
        .db
        .database(
            state
                .config
                .mongodb_uri
                .split('/')
                .next_back()
                .unwrap_or("default"),
        )
        .collection(Session::collection_name());

    let token_hash = Session::hash_token(&token);

    let session = collection
        .find_one(doc! { "tokenHash": &token_hash, "isActive": true })
        .await?
        .ok_or_else(|| AppError::NotFound("Invalid or expired session".to_string()))?;

    if session.is_expired() {
        return Err(AppError::BadRequest("Session has expired".to_string()));
    }

    let locations: Collection<Location> = state
        .db
        .database(
            state
                .config
                .mongodb_uri
                .split('/')
                .next_back()
                .unwrap_or("default"),
        )
        .collection(Location::collection_name());

    let location = locations
        .find_one(doc! { "_id": session.location_id })
        .await?
        .ok_or_else(|| AppError::NotFound("Location not found".to_string()))?;

    Ok(Json(serde_json::json!({
        "valid": true,
        "session": {
            "_id": session.id.unwrap().to_hex(),
            "locationName": location.name,
            "expiresAt": session.expires_at
        }
    })))
}

pub async fn check_attendance_status(
    State(state): State<Arc<crate::AppState>>,
    Path(token): Path<String>,
    Query(query): Query<StatusQuery>,
) -> Result<impl IntoResponse> {
    let sessions: Collection<Session> = state
        .db
        .database(
            state
                .config
                .mongodb_uri
                .split('/')
                .next_back()
                .unwrap_or("default"),
        )
        .collection(Session::collection_name());
    let attendances: Collection<Attendance> = state
        .db
        .database(
            state
                .config
                .mongodb_uri
                .split('/')
                .next_back()
                .unwrap_or("default"),
        )
        .collection(Attendance::collection_name());

    let token_hash = Session::hash_token(&token);

    let session = sessions
        .find_one(doc! { "tokenHash": &token_hash })
        .await?
        .ok_or_else(|| AppError::NotFound("Session not found".to_string()))?;

    let roll_number = query.roll_number.to_uppercase();

    let existing = attendances
        .find_one(doc! { "sessionId": session.id, "rollNumber": &roll_number })
        .await?;

    Ok(Json(serde_json::json!({
        "alreadySubmitted": existing.is_some(),
        "attendance": existing.map(|a| serde_json::json!({
            "studentName": a.student_name,
            "verified": a.verified,
            "capturedAt": a.captured_at,
        }))
    })))
}

#[derive(Debug, Deserialize)]
pub struct StatusQuery {
    pub roll_number: String,
}

pub async fn get_upload_url(
    State(state): State<Arc<crate::AppState>>,
    Path(token): Path<String>,
) -> Result<impl IntoResponse> {
    let sessions: Collection<Session> = state
        .db
        .database(
            state
                .config
                .mongodb_uri
                .split('/')
                .next_back()
                .unwrap_or("default"),
        )
        .collection(Session::collection_name());

    let token_hash = Session::hash_token(&token);

    let _session = sessions
        .find_one(doc! { "tokenHash": &token_hash, "isActive": true })
        .await?
        .ok_or_else(|| AppError::NotFound("Invalid or expired session".to_string()))?;

    let key = format!("attendance_{}.jpg", uuid::Uuid::new_v4());

    let presigned = state
        .storage
        .provider()
        .get_upload_url(&key, "image/jpeg")
        .await?;

    Ok(Json(serde_json::json!({
        "uploadUrl": presigned.upload_url,
        "publicId": presigned.public_id,
        "method": presigned.method,
        "contentType": presigned.content_type,
        "headers": presigned.headers
    })))
}

pub async fn get_captcha(
    State(state): State<Arc<crate::AppState>>,
    Path(_token): Path<String>,
) -> Result<impl IntoResponse> {
    use captcha::{
        filters::{Dots, Noise, Wave},
        Captcha,
    };

    let mut captcha = Captcha::new();
    captcha.add_chars(5);
    let captcha_text = captcha.chars_as_string();

    captcha
        .apply_filter(Noise::new(0.4))
        .apply_filter(Wave::new(2.0, 20.0).horizontal())
        .view(220, 120)
        .apply_filter(Dots::new(15));

    let png_data = captcha.as_png().unwrap_or_default();
    let svg = format!(
        "<img src=\"data:image/png;base64,{}\" />",
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, png_data)
    );

    let timestamp = chrono::Utc::now().timestamp_millis();
    let mut mac = Hmac::<Sha256>::new_from_slice(state.config.jwt_secret.as_bytes())
        .map_err(|_| AppError::Internal("Failed to create HMAC".to_string()))?;
    mac.update(format!("{}:{}", captcha_text.to_lowercase(), timestamp).as_bytes());
    let signature = hex::encode(mac.finalize().into_bytes());
    let captcha_id = format!("{}.{}", timestamp, signature);

    Ok(Json(serde_json::json!({
        "captchaSvg": svg,
        "captchaId": captcha_id,
    })))
}

#[allow(clippy::too_many_arguments)]
pub async fn submit_attendance(
    State(state): State<Arc<crate::AppState>>,
    Path(token): Path<String>,
    Extension(gps_validation): Extension<GpsValidationResult>,
    Extension(emulator_detection): Extension<EmulatorDetectionResult>,
    Extension(device_integrity): Extension<DeviceIntegrityResult>,
    Extension(device_check): Extension<DeviceCheckResult>,
    ConnectInfo(addr): ConnectInfo<std::net::SocketAddr>,
    headers: axum::http::HeaderMap,
    Json(payload): Json<SubmitAttendanceRequest>,
) -> Result<impl IntoResponse> {
    let bypass_captcha = std::env::var("DEV_BYPASS_ALL")
        .map(|v| v == "true")
        .unwrap_or(false);

    if !bypass_captcha {
        if let (Some(captcha_answer), Some(captcha_id)) =
            (&payload.captcha_answer, &payload.captcha_id)
        {
            let jwt_secret = &state.config.jwt_secret;
            verify_captcha(captcha_answer, captcha_id, jwt_secret)?;
        } else {
            return Err(AppError::BadRequest(
                "Captcha verification inputs missing".to_string(),
            ));
        }
    }

    if let Some(ref fingerprint) = payload.device_fingerprint {
        if let Some((is_blocked, block_reason)) =
            crate::middleware::check_device_blocked(&state, fingerprint).await?
        {
            if is_blocked {
                return Err(AppError::Forbidden(format!(
                    "This device has been blocked: {}",
                    block_reason.unwrap_or_else(|| "Suspicious activity detected".to_string())
                )));
            }
        }
    }

    let db_name = state
        .config
        .mongodb_uri
        .split('/')
        .next_back()
        .unwrap_or("default");

    let sessions: Collection<Session> = state
        .db
        .database(db_name)
        .collection(Session::collection_name());
    let attendances: Collection<Attendance> = state
        .db
        .database(db_name)
        .collection(Attendance::collection_name());
    let locations: Collection<Location> = state
        .db
        .database(db_name)
        .collection(Location::collection_name());

    let token_hash = Session::hash_token(&token);

    let session = sessions
        .find_one(doc! { "tokenHash": &token_hash, "isActive": true })
        .await?
        .ok_or_else(|| AppError::NotFound("Invalid or expired session".to_string()))?;

    if session.is_expired() {
        return Err(AppError::BadRequest("Session has expired".to_string()));
    }

    let roll_upper = payload.roll_number.to_uppercase();

    // Check WebAuthn credential enrollment time for grace period
    let webauthn_credentials: Collection<WebAuthnCredential> = state
        .db
        .database(db_name)
        .collection(WebAuthnCredential::collection_name());

    let webauthn_required = if let Some(credential) = webauthn_credentials
        .find_one(doc! { "studentId": &roll_upper })
        .await?
    {
        // Grace period: 15 minutes after enrollment
        let enrolled_at = credential.enrolled_at;
        let grace_period_end =
            enrolled_at + chrono::Duration::minutes(WEBAUTHN_GRACE_PERIOD_MINUTES);

        Utc::now() >= grace_period_end
    } else {
        false // No credential, no WebAuthn required
    };

    if webauthn_required && !payload.webauthn_verified.unwrap_or(false) {
        return Err(AppError::Forbidden(
            "Security policy requires biometric authentication. Please use your enrolled device."
                .to_string(),
        ));
    }

    let existing = attendances
        .find_one(doc! { "sessionId": session.id.unwrap(), "rollNumber": &roll_upper })
        .await?;

    if existing.is_some() {
        return Err(AppError::BadRequest(
            "Attendance already submitted".to_string(),
        ));
    }

    let location = locations
        .find_one(doc! { "_id": session.location_id })
        .await?
        .ok_or_else(|| AppError::NotFound("Location not found".to_string()))?;

    let distance = calculate_distance(
        location.latitude,
        location.longitude,
        payload.latitude,
        payload.longitude,
    );

    let is_within_geofence = distance <= location.radius_meters;

    let ip = addr.ip().to_string();
    let user_agent = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("Unknown")
        .to_string();

    let ip_info: IpInfo = crate::services::lookup_ip(&state.http_client, &ip)
        .await
        .unwrap_or_else(|_| IpInfo {
            isp: "Unknown".to_string(),
            org: "Unknown".to_string(),
            country: None,
            region: None,
            city: None,
        });

    let has_high_severity_gps = gps_validation
        .anomalies
        .iter()
        .any(|a| a.severity == Severity::High);
    let has_security_flags = has_high_severity_gps
        || emulator_detection.has_high_severity
        || emulator_detection.detected;

    let (device_flag, flag_reason) = if has_high_severity_gps {
        let anomaly_types: Vec<&str> = gps_validation
            .anomalies
            .iter()
            .filter(|a| a.severity == Severity::High)
            .map(|a| a.anomaly_type.as_str())
            .collect();
        (
            Some(AttendanceDeviceFlag::GpsAnomalyDetected),
            Some(format!("GPS anomalies: {}", anomaly_types.join(", "))),
        )
    } else if emulator_detection.detected {
        (
            Some(AttendanceDeviceFlag::EmulatorDetected),
            Some(format!(
                "Emulator detected: {}",
                emulator_detection.flags.join(", ")
            )),
        )
    } else if !device_integrity.passed {
        let check_names: Vec<&str> = device_integrity
            .checks
            .iter()
            .filter(|c| !c.passed)
            .map(|c| c.name.as_str())
            .collect();
        (
            Some(AttendanceDeviceFlag::IntegrityCheckFailed),
            Some(format!("Integrity issues: {}", check_names.join(", "))),
        )
    } else {
        (None, None)
    };

    let should_flag = has_security_flags || !device_integrity.passed;

    let gps_metadata = payload.gps_metadata.as_ref();

    let device_fingerprint_hash = payload
        .device_fingerprint
        .as_ref()
        .map(|fp| Device::hash_fingerprint(fp));

    let mut gps_anomalies: Vec<GpsAnomaly> = gps_validation
        .anomalies
        .iter()
        .map(|a| {
            let anomaly_type = match a.anomaly_type.as_str() {
                "ACCURACY_SUSPICIOUS" => GpsAnomalyType::AccuracySuspicious,
                "ACCURACY_VERY_SUSPICIOUS" => GpsAnomalyType::AccuracyVerySuspicious,
                "ALTITUDE_ZERO_OR_NULL" => GpsAnomalyType::AltitudeZeroOrNull,
                "SPEED_IMPOSSIBLE" => GpsAnomalyType::SpeedImpossible,
                "POSITION_JUMP" => GpsAnomalyType::PositionJump,
                "TIMESTAMP_DRIFT" => GpsAnomalyType::TimestampDrift,
                "ACCURACY_PATTERN" => GpsAnomalyType::AccuracyPattern,
                "PROVIDER_MISMATCH" => GpsAnomalyType::ProviderMismatch,
                _ => GpsAnomalyType::AccuracySuspicious,
            };
            GpsAnomaly {
                anomaly_type,
                severity: a.severity,
                details: Some(a.details.clone()),
                detected_at: Utc::now(),
            }
        })
        .collect();

    // GPS History Analysis - track position and detect anomalies
    // Use device fingerprint as device identifier, fall back to roll number
    let device_id = payload.device_fingerprint.as_deref().unwrap_or(&roll_upper);

    let gps_position_entry = GpsPositionEntry {
        latitude: payload.latitude,
        longitude: payload.longitude,
        accuracy: gps_metadata.and_then(|g| g.accuracy),
        altitude: gps_metadata.and_then(|g| g.altitude),
        speed: gps_metadata.and_then(|g| g.speed),
        heading: gps_metadata.and_then(|g| g.heading),
        timestamp: gps_metadata
            .and_then(|g| g.timestamp)
            .unwrap_or_else(|| Utc::now().timestamp_millis()),
        provider: gps_metadata.and_then(|g| g.provider.clone()),
        mock_location: gps_metadata.and_then(|g| g.is_mock_location),
    };

    // Add position to history and analyze (gracefully handle errors)
    if let Err(e) = state
        .gps_history
        .add_position(device_id, gps_position_entry.clone())
        .await
    {
        tracing::warn!("Failed to add GPS position to history: {}", e);
    }

    // Check for position jumps (threshold: POSITION_JUMP_THRESHOLD_M with speed > MAX_REASONABLE_SPEED_KMH indicates impossible travel)
    match state
        .gps_history
        .detect_position_jump(device_id, &gps_position_entry, POSITION_JUMP_THRESHOLD_M)
        .await
    {
        Ok(true) => {
            tracing::warn!(
                "Position jump detected for device {} at ({}, {})",
                device_id,
                payload.latitude,
                payload.longitude
            );
            gps_anomalies.push(GpsAnomaly {
                anomaly_type: GpsAnomalyType::PositionJump,
                severity: Severity::High,
                details: Some("GPS position jump detected from history analysis".to_string()),
                detected_at: Utc::now(),
            });
        }
        Ok(false) => {} // No jump detected
        Err(e) => {
            tracing::warn!("Failed to detect position jump: {}", e);
        }
    }

    // Check for impossible travel patterns
    match state
        .gps_history
        .detect_impossible_travel(device_id, &gps_position_entry)
        .await
    {
        Ok(history_anomalies) if !history_anomalies.is_empty() => {
            for anomaly in history_anomalies {
                tracing::warn!(
                    "Impossible travel detected for device {}: {} - {}",
                    device_id,
                    anomaly.anomaly_type,
                    anomaly.details
                );
                let anomaly_type = match anomaly.anomaly_type.as_str() {
                    "RAPID_POSITION_CHANGE" => GpsAnomalyType::PositionJump,
                    "IMPOSSIBLE_SPEED" => GpsAnomalyType::SpeedImpossible,
                    _ => GpsAnomalyType::SpeedImpossible,
                };
                gps_anomalies.push(GpsAnomaly {
                    anomaly_type,
                    severity: anomaly.severity,
                    details: Some(anomaly.details),
                    detected_at: Utc::now(),
                });
            }
        }
        Ok(_) => {} // No anomalies
        Err(e) => {
            tracing::warn!("Failed to detect impossible travel: {}", e);
        }
    }

    let gps_confidence = match gps_validation.confidence.as_str() {
        "high" => Some(GpsConfidence::High),
        "medium" => Some(GpsConfidence::Medium),
        "low" => Some(GpsConfidence::Low),
        "suspicious" => Some(GpsConfidence::Suspicious),
        _ => None,
    };

    let emulator_flags: Vec<EmulatorFlag> = emulator_detection
        .flags
        .iter()
        .map(|f| {
            let flag_type = match f.as_str() {
                "DESKTOP_GPU_DETECTED" => EmulatorFlagType::DesktopGpuDetected,
                "AUDIO_FINGERPRINT_EMULATOR" => EmulatorFlagType::AudioFingerprintEmulator,
                "TIMING_ANOMALY" => EmulatorFlagType::TimingAnomaly,
                "BATTERY_PATTERN_EMULATOR" => EmulatorFlagType::BatteryPatternEmulator,
                "SCREEN_RESOLUTION_SUSPICIOUS" => EmulatorFlagType::ScreenResolutionSuspicious,
                "DEVICE_MEMORY_ROUND" => EmulatorFlagType::DeviceMemoryRound,
                "WEBGL_RENDERER_EMULATOR" => EmulatorFlagType::WebglRendererEmulator,
                "PLATFORM_INCONSISTENCY" => EmulatorFlagType::PlatformInconsistency,
                _ => EmulatorFlagType::PlatformInconsistency,
            };
            EmulatorFlag {
                flag_type,
                severity: Severity::Medium,
                details: None,
            }
        })
        .collect();

    let integrity_checks: Vec<IntegrityCheck> = device_integrity
        .checks
        .iter()
        .map(|c| {
            let check_type = match c.name.as_str() {
                "TIMING_MANIPULATION" => IntegrityCheckType::TimingManipulation,
                "BROWSER_API_INCONSISTENCY" => IntegrityCheckType::BrowserApiInconsistency,
                "POINTER_EVENTS_SUSPICIOUS" => IntegrityCheckType::PointerEventsSuspicious,
                _ => IntegrityCheckType::BrowserApiInconsistency,
            };
            IntegrityCheck {
                check_type,
                passed: c.passed,
                details: c.details.clone(),
            }
        })
        .collect();

    // Perform face detection if photo_url is provided
    let face_detected_result = if let (Some(photo_public_id), true) =
        (&payload.photo_public_id, payload.photo_url.is_some())
    {
        match state.storage.provider().download(photo_public_id).await {
            Ok(image_data) => match detect_faces(&image_data).await {
                Ok(result) => {
                    tracing::info!(
                        "Face detection completed for attendance: detected={}, confidence={:.2}",
                        result.face_detected,
                        result.confidence
                    );
                    Some(result.face_detected)
                }
                Err(e) => {
                    tracing::warn!("Face detection failed for attendance submission: {}", e);
                    None
                }
            },
            Err(e) => {
                tracing::warn!("Failed to download photo for face detection: {}", e);
                None
            }
        }
    } else {
        None
    };

    // Use the detection result, or fall back to the client-provided value, or default to true
    let face_detected = face_detected_result
        .or(payload.face_detected)
        .unwrap_or(true);

    // Compute photo hash and check for reuse
    let photo_hashes: Collection<PhotoHash> = state
        .db
        .database(db_name)
        .collection(PhotoHash::collection_name());
    let (photo_hash_value, photo_reuse_detected) =
        if let Some(photo_public_id) = &payload.photo_public_id {
            match state.storage.provider().download(photo_public_id).await {
                Ok(image_data) => {
                    // Compute perceptual hash
                    let hash_result = compute_image_hash(&image_data);
                    let hash_str = match hash_result {
                        Ok(h) => Some(format!("{:016x}", h)),
                        Err(e) => {
                            tracing::warn!("Failed to compute photo hash: {}", e);
                            None
                        }
                    };

                    // Check for reuse in same session
                    let reuse_detected = if let Some(ref hash) = hash_str {
                        if let Some(session_id) = session.id {
                            match photo_hashes
                                .find_one(doc! {
                                    "sessionId": session_id,
                                    "rollNumber": { "$ne": &roll_upper }
                                })
                                .await
                            {
                                Ok(Some(existing)) => {
                                    // Compare with existing hash using similarity threshold
                                    let existing_hash = existing.photo_hash;
                                    let threshold = PHOTO_SIMILARITY_THRESHOLD;
                                    is_same_photo(hash, &existing_hash, threshold)
                                }
                                Ok(None) => false,
                                Err(e) => {
                                    tracing::warn!("Failed to check photo reuse: {}", e);
                                    false
                                }
                            }
                        } else {
                            false
                        }
                    } else {
                        false
                    };

                    if reuse_detected {
                        tracing::warn!(
                            "Photo reuse detected for roll_number={} in session={}",
                            roll_upper,
                            session.id.map(|id| id.to_hex()).unwrap_or_default()
                        );
                    }

                    (hash_str, reuse_detected)
                }
                Err(e) => {
                    tracing::warn!("Failed to download photo for hash computation: {}", e);
                    (None, false)
                }
            }
        } else {
            (None, false)
        };

    let attendance = Attendance {
        id: None,
        session_id: session.id.unwrap(),
        student_name: payload.student_name,
        roll_number: roll_upper.clone(),
        photo_url: payload.photo_url.unwrap_or_default(),
        photo_public_id: payload.photo_public_id.unwrap_or_default(),
        photo_hash: photo_hash_value.clone(),
        photo_reuse_detected,
        student_latitude: payload.latitude,
        student_longitude: payload.longitude,
        distance_from_location: distance,
        ip_address: Some(ip),
        user_agent: Some(user_agent.clone()),
        network_provider: Some(ip_info.isp),
        network_org: Some(ip_info.org),
        verified: is_within_geofence,
        face_detected,
        device_fingerprint: payload.device_fingerprint.clone(),
        device_fingerprint_hash,
        device_first_seen: device_check.first_seen,
        totp_code: payload.totp_code.clone(),
        totp_valid: None,
        device_flag: device_flag.clone(),
        webauthn_credential_id: payload.webauthn_credential_id,
        webauthn_verified: payload.webauthn_verified.unwrap_or(false),
        webauthn_device_type: None,
        webauthn_authenticator_attachment: None,
        webauthn_counter: None,
        webauthn_replay_attack: false,
        flag_reviewed: false,
        flag_reviewed_by: None,
        flag_reviewed_at: None,
        flagged: should_flag,
        flag_reason: flag_reason.clone(),
        flag_details: flag_reason,
        captured_at: Utc::now(),
        gps_accuracy: gps_metadata.and_then(|g| g.accuracy),
        gps_altitude: gps_metadata.and_then(|g| g.altitude),
        gps_altitude_accuracy: gps_metadata.and_then(|g| g.altitude_accuracy),
        gps_speed: gps_metadata.and_then(|g| g.speed),
        gps_heading: gps_metadata.and_then(|g| g.heading),
        gps_timestamp: gps_metadata.and_then(|g| g.timestamp),
        gps_mock_location: gps_metadata
            .and_then(|g| g.is_mock_location)
            .unwrap_or(false),
        gps_provider: gps_metadata.and_then(|g| g.provider.clone()),
        gps_anomalies,
        gps_confidence,
        emulator_detected: emulator_detection.detected,
        emulator_flags,
        integrity_checks,
    };

    let result = attendances.insert_one(&attendance).await?;
    let attendance_id = result
        .inserted_id
        .as_object_id()
        .ok_or_else(|| AppError::Internal("Failed to get inserted ID".to_string()))?;

    if let (Some(ref fingerprint), Some(session_id)) = (&payload.device_fingerprint, session.id) {
        if let Err(e) =
            record_device_success(&state, fingerprint, session_id, &roll_upper, &user_agent).await
        {
            tracing::warn!("Failed to record device success: {}", e);
        }
    }

    // Store photo hash for reuse detection
    if let (Some(hash), Some(session_id)) = (photo_hash_value, session.id) {
        let photo_hash_doc = PhotoHash {
            id: None,
            roll_number: roll_upper.clone(),
            photo_hash: hash,
            session_id,
            captured_at: Utc::now(),
            confidence: None,
        };
        if let Err(e) = photo_hashes.insert_one(&photo_hash_doc).await {
            tracing::warn!("Failed to store photo hash: {}", e);
        }
    }

    let response_message = if should_flag {
        "Attendance submitted. Note: Submission flagged for security review."
    } else if !device_check.flags.is_empty() {
        "Attendance submitted successfully. Note: Device flagged for review."
    } else {
        "Attendance submitted successfully"
    };

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "message": response_message,
            "attendance": {
                "_id": attendance_id.to_hex(),
                "sessionId": session.id.unwrap().to_hex(),
                "studentName": attendance.student_name,
                "rollNumber": attendance.roll_number,
                "verified": attendance.verified,
                "distance": distance.round(),
                "deviceFirstSeen": attendance.device_first_seen,
                "deviceFlag": device_flag,
            },
            "deviceWarning": if device_check.flags.is_empty() { None } else { Some(device_check.flags) }
        })),
    ))
}
