use axum::{
    extract::{Json, State},
    response::IntoResponse,
    Extension,
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    controllers::{fetch_roster_students, resolve_roster_source},
    error::{AppError, Result},
    middleware::AuthenticatedAdmin,
    models::{Attendance, Session},
    utils::generate_qr_token,
};

// =================== Session Attendance Endpoints ===================

#[derive(Debug, Serialize)]
pub struct SessionAttendanceResponse {
    #[serde(flatten)]
    pub attendance: Attendance,
    pub signed_photo_url: Option<String>,
}

#[cfg(test)]
mod session_attendance_response_tests {
    use super::*;
    use crate::models::{AttendanceSource, AttendanceStatus};
    use sqlx::types::Json;

    // Regression test for the "selecting one student selects all" bug
    // (admin SessionDetail.tsx): `get_session_attendance` returns a
    // `Vec<SessionAttendanceResponse>`, which flattens `Attendance` straight
    // into the JSON array the frontend reads `record._id` from. If
    // `Attendance::id` ever lost its `_id` rename, every row would come back
    // keyed `"id"` instead, `record._id` would be `undefined` for every row,
    // and the frontend's `Set<string>` selection state would collapse every
    // row onto the same `undefined` key -- exactly the reported symptom.
    #[test]
    fn flattened_response_exposes_underscore_id_not_id() {
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
        let expected_id = attendance.id;

        let response = SessionAttendanceResponse {
            attendance,
            signed_photo_url: None,
        };

        let value = serde_json::to_value(&response).expect("response must serialize");
        assert_eq!(
            value["_id"],
            serde_json::json!(expected_id),
            "flattened response must expose the record's id under \"_id\" (frontend's AttendanceRecord._id)"
        );
        assert!(
            value.get("id").is_none(),
            "flattened response must not also expose a plain \"id\" key"
        );
    }
}

pub async fn get_session_attendance(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<impl IntoResponse> {
    let session_id = Uuid::parse_str(&id)
        .map_err(|e| AppError::BadRequest(format!("Invalid session ID: {}", e)))?;

    // Role-scoped: super-admins see sessions they created, mentors only
    // sessions assigned to them.
    let _session = crate::controllers::find_session_for_admin(&state.db, session_id, &auth).await?;

    let attendances: Vec<Attendance> =
        sqlx::query_as("SELECT * FROM attendances WHERE session_id = $1 ORDER BY captured_at DESC")
            .bind(session_id)
            .fetch_all(&state.db)
            .await?;

    let result: Vec<SessionAttendanceResponse> = attendances
        .into_iter()
        .map(|attendance| SessionAttendanceResponse {
            attendance,
            signed_photo_url: None,
        })
        .collect();

    Ok(Json(result))
}

/// Serialised camelCase to match what the admin frontend reads. It was plain
/// snake_case, so `stats.totalAttendance` was always `undefined` and the
/// session view's TOTAL ATTENDANCE card silently rendered 0 regardless of how
/// many students had marked attendance.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStatsResponse {
    pub total_attendance: i64,
    pub verified_attendance: i64,
    pub unverified_attendance: i64,
    /// Students on the session's batch roster. 0 when no batch is attached —
    /// a batch is optional, so absence is only meaningful when there is a
    /// roster to be absent from.
    pub roster_size: i64,
    /// Roster students with no attendance row for this session.
    pub absent_count: i64,
    /// True when a batch is attached, so the UI can distinguish "0 absent"
    /// from "absence is not tracked for this session".
    pub has_roster: bool,
    pub session: SessionStatus,
}

#[derive(Debug, Serialize)]
pub struct SessionStatus {
    pub is_active: bool,
    pub expires_at: DateTime<Utc>,
    pub rotation_count: i32,
}

pub async fn get_session_stats(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<impl IntoResponse> {
    let session_id = Uuid::parse_str(&id)
        .map_err(|e| AppError::BadRequest(format!("Invalid session ID: {}", e)))?;

    let session = crate::controllers::find_session_for_admin(&state.db, session_id, &auth).await?;

    let total_attendance: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM attendances WHERE session_id = $1")
            .bind(session_id)
            .fetch_one(&state.db)
            .await?;
    let verified_attendance: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM attendances WHERE session_id = $1 AND verified = true",
    )
    .bind(session_id)
    .fetch_one(&state.db)
    .await?;

    // Absence is computed against the roster (regular batch or excel batch,
    // whichever this session was created against) when one is attached.
    // Matching is on upper(roll_number) because attendance stores the roll
    // number upper-cased while the roster keeps whatever the import supplied.
    let roster_source = resolve_roster_source(&session);
    let roster = fetch_roster_students(&state.db, &roster_source).await?;
    let roster_size = roster.len() as i64;
    let absent_count = if roster.is_empty() {
        0
    } else {
        let present_rolls: std::collections::HashSet<String> = sqlx::query_scalar::<_, String>(
            "SELECT roll_number FROM attendances WHERE session_id = $1",
        )
        .bind(session_id)
        .fetch_all(&state.db)
        .await?
        .into_iter()
        .map(|r| r.to_uppercase())
        .collect();
        roster
            .iter()
            .filter(|s| !present_rolls.contains(&s.roll_number.to_uppercase()))
            .count() as i64
    };

    Ok(Json(SessionStatsResponse {
        total_attendance,
        verified_attendance,
        unverified_attendance: total_attendance - verified_attendance,
        roster_size,
        absent_count,
        has_roster: !roster.is_empty(),
        session: SessionStatus {
            is_active: session.is_active,
            expires_at: session.expires_at,
            rotation_count: session.rotation_count,
        },
    }))
}

#[derive(Debug, Serialize)]
pub struct TOTPResponse {
    pub session_id: String,
    pub totp_code: Option<String>,
    #[serde(rename = "qrToken")]
    pub qr_token: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub window_seconds: Option<i64>,
    pub session_active: bool,
}

pub async fn get_session_totp(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<impl IntoResponse> {
    let session_id = Uuid::parse_str(&id)
        .map_err(|e| AppError::BadRequest(format!("Invalid session ID: {}", e)))?;

    let session: Session = sqlx::query_as(
        "SELECT * FROM sessions WHERE id = $1 AND ($2 = 'super_admin' OR created_by = $3)",
    )
    .bind(session_id)
    .bind(&auth.role)
    .bind(auth.id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("Session not found".to_string()))?;

    // Generate QR token for anti-sharing using session ID and totp_secret
    let qr_token = session
        .totp_secret
        .as_ref()
        .map(|totp_secret| generate_qr_token(&session_id.to_string(), totp_secret));

    Ok(Json(TOTPResponse {
        session_id: id,
        totp_code: session.totp_secret,
        qr_token,
        expires_at: Some(session.expires_at),
        window_seconds: Some(4), // 4 seconds validity window for QR token
        session_active: session.is_active,
    }))
}

pub async fn get_session_devices(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<impl IntoResponse> {
    let session_id = Uuid::parse_str(&id)
        .map_err(|e| AppError::BadRequest(format!("Invalid session ID: {}", e)))?;

    let _session: Session = sqlx::query_as(
        "SELECT * FROM sessions WHERE id = $1 AND ($2 = 'super_admin' OR created_by = $3)",
    )
    .bind(session_id)
    .bind(&auth.role)
    .bind(auth.id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("Session not found".to_string()))?;

    let devices: Vec<crate::models::Device> =
        sqlx::query_as("SELECT * FROM devices WHERE session_id = $1 ORDER BY last_seen_at DESC")
            .bind(session_id)
            .fetch_all(&state.db)
            .await?;

    Ok(Json(devices))
}

#[derive(Debug, Serialize)]
pub struct AbsentStudent {
    pub name: String,
    pub roll_number: String,
    pub college_name: Option<String>,
    pub email: Option<String>,
}

pub async fn get_session_absent(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<impl IntoResponse> {
    let session_id = Uuid::parse_str(&id)
        .map_err(|e| AppError::BadRequest(format!("Invalid session ID: {}", e)))?;

    let session = crate::controllers::find_session_for_admin(&state.db, session_id, &auth).await?;

    let source = resolve_roster_source(&session);
    let students = fetch_roster_students(&state.db, &source).await?;
    if students.is_empty() {
        return Ok(Json::<Vec<AbsentStudent>>(vec![]));
    }

    // Get present roll numbers
    let present_roll_numbers: Vec<String> = sqlx::query_scalar(
        "SELECT roll_number FROM attendances WHERE session_id = $1 AND verified = true",
    )
    .bind(session_id)
    .fetch_all(&state.db)
    .await?;

    let present_rolls: std::collections::HashSet<String> = present_roll_numbers
        .into_iter()
        .map(|r| r.to_uppercase())
        .collect();

    let absent_students: Vec<AbsentStudent> = students
        .into_iter()
        .filter(|s| !present_rolls.contains(&s.roll_number.to_uppercase()))
        .map(|s| AbsentStudent {
            name: s.name,
            roll_number: s.roll_number,
            college_name: s.college_name,
            email: s.email,
        })
        .collect();

    Ok(Json(absent_students))
}
