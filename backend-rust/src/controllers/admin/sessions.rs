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
    error::{AppError, Result},
    middleware::AuthenticatedAdmin,
    models::{Attendance, Batch, Session, Student},
    utils::generate_qr_token,
};

// =================== Session Attendance Endpoints ===================

#[derive(Debug, Serialize)]
pub struct SessionAttendanceResponse {
    #[serde(flatten)]
    pub attendance: Attendance,
    pub signed_photo_url: Option<String>,
}

pub async fn get_session_attendance(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<impl IntoResponse> {
    let session_id = Uuid::parse_str(&id)
        .map_err(|e| AppError::BadRequest(format!("Invalid session ID: {}", e)))?;

    // Verify session ownership
    let _session: Session =
        sqlx::query_as("SELECT * FROM sessions WHERE id = $1 AND created_by = $2")
            .bind(session_id)
            .bind(auth.id)
            .fetch_optional(&state.db)
            .await?
            .ok_or_else(|| AppError::NotFound("Session not found".to_string()))?;

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

#[derive(Debug, Serialize)]
pub struct SessionStatsResponse {
    pub total_attendance: i64,
    pub verified_attendance: i64,
    pub unverified_attendance: i64,
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

    let session: Session =
        sqlx::query_as("SELECT * FROM sessions WHERE id = $1 AND created_by = $2")
            .bind(session_id)
            .bind(auth.id)
            .fetch_optional(&state.db)
            .await?
            .ok_or_else(|| AppError::NotFound("Session not found".to_string()))?;

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

    Ok(Json(SessionStatsResponse {
        total_attendance,
        verified_attendance,
        unverified_attendance: total_attendance - verified_attendance,
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

    let session: Session =
        sqlx::query_as("SELECT * FROM sessions WHERE id = $1 AND created_by = $2")
            .bind(session_id)
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

    let _session: Session =
        sqlx::query_as("SELECT * FROM sessions WHERE id = $1 AND created_by = $2")
            .bind(session_id)
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

    let session: Session =
        sqlx::query_as("SELECT * FROM sessions WHERE id = $1 AND created_by = $2")
            .bind(session_id)
            .bind(auth.id)
            .fetch_optional(&state.db)
            .await?
            .ok_or_else(|| AppError::NotFound("Session not found".to_string()))?;

    let batch_id = match session.batch_id {
        Some(id) => id,
        None => return Ok(Json::<Vec<AbsentStudent>>(vec![])),
    };

    let _batch: Batch = sqlx::query_as("SELECT * FROM batches WHERE id = $1")
        .bind(batch_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound("Batch not found".to_string()))?;

    let students: Vec<Student> =
        sqlx::query_as("SELECT * FROM students WHERE batch_id = $1 ORDER BY position")
            .bind(batch_id)
            .fetch_all(&state.db)
            .await?;

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
