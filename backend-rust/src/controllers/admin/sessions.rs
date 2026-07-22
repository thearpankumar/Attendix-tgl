use axum::{
    extract::{Json, State},
    response::IntoResponse,
    Extension,
};
use chrono::{DateTime, Utc};
use mongodb::{bson::doc, Collection};
use serde::Serialize;
use std::sync::Arc;

use crate::{
    error::{AppError, Result},
    middleware::AuthenticatedAdmin,
    models::{Attendance, Batch, Session},
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
    let db = state.db.database(
        state
            .config
            .mongodb_uri
            .split('/')
            .next_back()
            .unwrap_or("default"),
    );

    let sessions_collection: Collection<Session> = db.collection(Session::collection_name());
    let attendances_collection: Collection<Attendance> =
        db.collection(Attendance::collection_name());

    use mongodb::bson::oid::ObjectId;
    let session_id = ObjectId::parse_str(&id)
        .map_err(|e| AppError::BadRequest(format!("Invalid session ID: {}", e)))?;

    // Verify session ownership
    sessions_collection
        .find_one(doc! { "_id": session_id, "createdBy": auth.id })
        .await?
        .ok_or_else(|| AppError::NotFound("Session not found".to_string()))?;

    let mut cursor = attendances_collection
        .find(doc! { "sessionId": session_id })
        .sort(doc! { "capturedAt": -1 })
        .await?;

    let mut result = Vec::new();
    while cursor.advance().await? {
        let attendance = cursor.deserialize_current()?;
        result.push(SessionAttendanceResponse {
            attendance,
            signed_photo_url: None,
        });
    }

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
    let db = state.db.database(
        state
            .config
            .mongodb_uri
            .split('/')
            .next_back()
            .unwrap_or("default"),
    );

    let sessions_collection: Collection<Session> = db.collection(Session::collection_name());
    let attendances_collection: Collection<Attendance> =
        db.collection(Attendance::collection_name());

    use mongodb::bson::oid::ObjectId;
    let session_id = ObjectId::parse_str(&id)
        .map_err(|e| AppError::BadRequest(format!("Invalid session ID: {}", e)))?;

    let session = sessions_collection
        .find_one(doc! { "_id": session_id, "createdBy": auth.id })
        .await?
        .ok_or_else(|| AppError::NotFound("Session not found".to_string()))?;

    let total_attendance = attendances_collection
        .count_documents(doc! { "sessionId": session_id })
        .await? as i64;
    let verified_attendance = attendances_collection
        .count_documents(doc! { "sessionId": session_id, "verified": true })
        .await? as i64;

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
    let db = state.db.database(
        state
            .config
            .mongodb_uri
            .split('/')
            .next_back()
            .unwrap_or("default"),
    );

    use mongodb::bson::oid::ObjectId;
    let sessions_collection: Collection<Session> = db.collection(Session::collection_name());

    let session_id = ObjectId::parse_str(&id)
        .map_err(|e| AppError::BadRequest(format!("Invalid session ID: {}", e)))?;

    let session = sessions_collection
        .find_one(doc! { "_id": session_id, "createdBy": auth.id })
        .await?
        .ok_or_else(|| AppError::NotFound("Session not found".to_string()))?;

    // Generate QR token for anti-sharing using session ID hex and totp_secret
    let qr_token = if let Some(ref totp_secret) = session.totp_secret {
        let session_hex = session_id.to_hex();
        Some(generate_qr_token(&session_hex, totp_secret))
    } else {
        None
    };

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
    let db = state.db.database(
        state
            .config
            .mongodb_uri
            .split('/')
            .next_back()
            .unwrap_or("default"),
    );

    use mongodb::bson::oid::ObjectId;
    let sessions_collection: Collection<Session> = db.collection(Session::collection_name());
    let devices_collection: Collection<crate::models::Device> =
        db.collection(crate::models::Device::collection_name());

    let session_id = ObjectId::parse_str(&id)
        .map_err(|e| AppError::BadRequest(format!("Invalid session ID: {}", e)))?;

    sessions_collection
        .find_one(doc! { "_id": session_id, "createdBy": auth.id })
        .await?
        .ok_or_else(|| AppError::NotFound("Session not found".to_string()))?;

    let mut cursor = devices_collection
        .find(doc! { "sessionId": session_id })
        .sort(doc! { "lastSeenAt": -1 })
        .await?;

    let mut result = Vec::new();
    while cursor.advance().await? {
        let device = cursor.deserialize_current()?;
        result.push(device);
    }

    Ok(Json(result))
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
    let db = state.db.database(
        state
            .config
            .mongodb_uri
            .split('/')
            .next_back()
            .unwrap_or("default"),
    );

    use mongodb::bson::oid::ObjectId;
    let sessions_collection: Collection<Session> = db.collection(Session::collection_name());
    let batches_collection: Collection<Batch> = db.collection(Batch::collection_name());
    let attendances_collection: Collection<Attendance> =
        db.collection(Attendance::collection_name());

    let session_id = ObjectId::parse_str(&id)
        .map_err(|e| AppError::BadRequest(format!("Invalid session ID: {}", e)))?;

    let session = sessions_collection
        .find_one(doc! { "_id": session_id, "createdBy": auth.id })
        .await?
        .ok_or_else(|| AppError::NotFound("Session not found".to_string()))?;

    let batch_id = match session.batch_id {
        Some(id) => id,
        None => return Ok(Json::<Vec<AbsentStudent>>(vec![])),
    };

    let batch = batches_collection
        .find_one(doc! { "_id": batch_id })
        .await?
        .ok_or_else(|| AppError::NotFound("Batch not found".to_string()))?;

    // Get present roll numbers
    let mut cursor = attendances_collection
        .find(doc! { "sessionId": session_id, "verified": true })
        .projection(doc! { "rollNumber": 1 })
        .await?;

    let mut present_rolls = std::collections::HashSet::new();
    while cursor.advance().await? {
        let attendance = cursor.deserialize_current()?;
        present_rolls.insert(attendance.roll_number.to_uppercase());
    }

    let absent_students: Vec<AbsentStudent> = batch
        .students
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
