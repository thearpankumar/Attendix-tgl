use axum::{
    extract::{Json, Path, Query, State},
    http::{header, StatusCode},
    response::IntoResponse,
    Extension,
};
use chrono::{DateTime, Utc};
use mongodb::{
    bson::{doc, oid::ObjectId},
    Collection,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::{
    error::{AppError, Result},
    middleware::{
        validators::{validate_request, SessionCreateRequest},
        AuthenticatedAdmin,
    },
    models::{Admin, Attendance, Batch, Location, Session, ShortLink},
};

#[derive(Debug, Deserialize)]
pub struct CreateSessionRequest {
    pub location_id: String,
    pub batch_id: Option<String>,
    pub description: Option<String>,
    pub expires_in_hours: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct SessionResponse {
    #[serde(rename = "_id")]
    pub id: String,
    pub token: String,
    #[serde(rename = "locationId")]
    pub location_id: String,
    pub description: Option<String>,
    #[serde(rename = "isActive")]
    pub is_active: bool,
    #[serde(rename = "expiresAt")]
    pub expires_at: DateTime<Utc>,
}

pub async fn create_session(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Json(payload): Json<CreateSessionRequest>,
) -> Result<impl IntoResponse> {
    let validation_req = SessionCreateRequest {
        location_id: payload.location_id.clone(),
        duration_minutes: payload.expires_in_hours.map(|h| (h * 60) as i32),
        batch_id: payload.batch_id.clone(),
        description: payload.description.clone(),
    };
    validate_request(&validation_req)?;

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

    let location_id = ObjectId::parse_str(&payload.location_id)
        .map_err(|e| AppError::BadRequest(format!("Invalid location ID: {}", e)))?;

    let batch_id = payload
        .batch_id
        .and_then(|id| ObjectId::parse_str(&id).ok());

    let token = Session::generate_token();
    let expires_at = Utc::now() + chrono::Duration::hours(payload.expires_in_hours.unwrap_or(24));

    let session = Session {
        id: None,
        location_id,
        batch_id,
        token_hash: Session::hash_token(&token),
        token_prefix: Session::get_token_prefix(&token),
        description: payload.description,
        created_by: auth.id,
        is_active: true,
        expires_at,
        rotation_count: 0,
        totp_secret: Some(Session::generate_totp_secret()),
        created_at: Utc::now(),
    };

    let result = collection.insert_one(&session).await?;
    let session_id = result
        .inserted_id
        .as_object_id()
        .ok_or_else(|| AppError::Internal("Failed to get inserted ID".to_string()))?;

    Ok((
        StatusCode::CREATED,
        Json(SessionResponse {
            id: session_id.to_hex(),
            token,
            location_id: payload.location_id,
            description: session.description,
            is_active: true,
            expires_at: session.expires_at,
        }),
    ))
}

pub async fn get_sessions(
    State(state): State<Arc<crate::AppState>>,
    Extension(_auth): Extension<AuthenticatedAdmin>,
    Query(_query): Query<serde_json::Value>,
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

    let mut cursor = collection
        .find(doc! {})
        .sort(doc! { "createdAt": -1 })
        .limit(50)
        .await?;
    let mut sessions = Vec::new();

    while cursor.advance().await? {
        let session = cursor.deserialize_current()?;
        sessions.push(SessionResponse {
            id: session
                .id
                .ok_or_else(|| AppError::Internal("No ID".to_string()))?
                .to_hex(),
            token: String::new(),
            location_id: session.location_id.to_hex(),
            description: session.description,
            is_active: session.is_active,
            expires_at: session.expires_at,
        });
    }

    Ok(Json(sessions))
}

pub async fn get_session(
    State(state): State<Arc<crate::AppState>>,
    Extension(_auth): Extension<AuthenticatedAdmin>,
    Path(id): Path<String>,
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

    let session_id = ObjectId::parse_str(&id)
        .map_err(|e| AppError::BadRequest(format!("Invalid session ID: {}", e)))?;

    let session = collection
        .find_one(doc! { "_id": session_id })
        .await?
        .ok_or_else(|| AppError::NotFound("Session not found".to_string()))?;

    Ok(Json(SessionResponse {
        id: session
            .id
            .ok_or_else(|| AppError::Internal("No ID".to_string()))?
            .to_hex(),
        token: String::new(),
        location_id: session.location_id.to_hex(),
        description: session.description,
        is_active: session.is_active,
        expires_at: session.expires_at,
    }))
}

pub async fn deactivate_session(
    State(state): State<Arc<crate::AppState>>,
    Extension(_auth): Extension<AuthenticatedAdmin>,
    Path(id): Path<String>,
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

    let session_id = ObjectId::parse_str(&id)
        .map_err(|e| AppError::BadRequest(format!("Invalid session ID: {}", e)))?;

    collection
        .update_one(
            doc! { "_id": session_id },
            doc! { "$set": { "isActive": false } },
        )
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

/// Request body for deleting a session (requires password re-verification)
#[derive(Debug, Deserialize)]
pub struct DeleteSessionRequest {
    pub password: String,
}

/// Delete a session with password re-verification
/// This is a destructive operation that requires the admin to re-enter their password
pub async fn delete_session(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Path(id): Path<String>,
    Json(payload): Json<DeleteSessionRequest>,
) -> Result<impl IntoResponse> {
    let db = state.db.database(
        state.config.mongodb_uri.split('/').next_back().unwrap_or("default"),
    );

    let sessions: Collection<Session> = db.collection(Session::collection_name());
    let attendances: Collection<Attendance> = db.collection(Attendance::collection_name());
    let admins: Collection<Admin> = db.collection(Admin::collection_name());
    let short_links: Collection<ShortLink> = db.collection(ShortLink::collection_name());

    // Parse session ID
    let session_id = ObjectId::parse_str(&id)
        .map_err(|e| AppError::BadRequest(format!("Invalid session ID: {}", e)))?;

    // First verify session ownership (so cross-admin deletions get 404, not 401)
    let _session = sessions
        .find_one(doc! { "_id": session_id, "createdBy": auth.id })
        .await?
        .ok_or_else(|| AppError::NotFound("Session not found".to_string()))?;

    // Verify password - re-fetch admin with password field
    let admin = admins
        .find_one(doc! { "_id": auth.id })
        .await?
        .ok_or_else(|| AppError::Unauthorized("Admin not found".to_string()))?;

    // Verify the password using Admin::verify_password()
    let password_valid = admin.verify_password(&payload.password)
        .map_err(|e| AppError::Internal(format!("Password verification failed: {}", e)))?;

    if !password_valid {
        return Err(AppError::Unauthorized("Incorrect password".to_string()));
    }

    // Delete all attendance records for this session
    attendances
        .delete_many(doc! { "sessionId": session_id })
        .await?;

    // Detach any short links that pointed to this session so they can be reattached
    short_links
        .update_many(
            doc! { "sessionId": session_id },
            doc! { "$set": { "sessionId": null, "isActive": false } },
        )
        .await?;

    // Delete the session
    sessions
        .delete_one(doc! { "_id": session_id })
        .await?;

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "success": true,
            "message": "Session and all attendance records deleted successfully"
        })),
    ))
}

pub async fn rotate_token(
    State(state): State<Arc<crate::AppState>>,
    Extension(_auth): Extension<AuthenticatedAdmin>,
    Path(id): Path<String>,
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

    let session_id = ObjectId::parse_str(&id)
        .map_err(|e| AppError::BadRequest(format!("Invalid session ID: {}", e)))?;

    let session = collection
        .find_one(doc! { "_id": session_id })
        .await?
        .ok_or_else(|| AppError::NotFound("Session not found".to_string()))?;

    let new_token = Session::generate_token();

    collection
        .update_one(
            doc! { "_id": session_id },
            doc! {
                "$set": {
                    "tokenHash": Session::hash_token(&new_token),
                    "tokenPrefix": Session::get_token_prefix(&new_token),
                    "totpSecret": Session::generate_totp_secret(),
                    "rotationCount": session.rotation_count + 1
                }
            },
        )
        .await?;

    Ok(Json(serde_json::json!({ "token": new_token })))
}

#[derive(Debug, Deserialize)]
pub struct ExportQuery {
    pub format: Option<String>,
}

pub async fn export_session_attendance(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Path(id): Path<String>,
    Query(_query): Query<ExportQuery>,
) -> Result<impl IntoResponse> {
    let db = state.db.database(
        state.config.mongodb_uri.split('/').next_back().unwrap_or("default"),
    );

    let sessions: Collection<Session> = db.collection(Session::collection_name());
    let attendances: Collection<Attendance> = db.collection(Attendance::collection_name());
    let locations: Collection<Location> = db.collection(Location::collection_name());
    let batches: Collection<Batch> = db.collection(Batch::collection_name());

    let session_id = ObjectId::parse_str(&id)
        .map_err(|e| AppError::BadRequest(format!("Invalid session ID: {}", e)))?;

    let session = sessions
        .find_one(doc! { "_id": session_id, "createdBy": auth.id })
        .await?
        .ok_or_else(|| AppError::NotFound("Session not found".to_string()))?;

    let location = locations
        .find_one(doc! { "_id": session.location_id })
        .await?
        .ok_or_else(|| AppError::NotFound("Location not found".to_string()))?;

    let batch = if let Some(batch_id) = session.batch_id {
        batches.find_one(doc! { "_id": batch_id }).await?
    } else {
        None
    };

    let mut cursor = attendances
        .find(doc! { "sessionId": session_id })
        .sort(doc! { "capturedAt": 1 })
        .await?;

    let mut attendance_data: Vec<AttendanceExportRow> = Vec::new();

    while cursor.advance().await? {
        let attendance = cursor.deserialize_current()?;
        attendance_data.push(AttendanceExportRow {
            roll_number: attendance.roll_number.clone(),
            student_name: attendance.student_name.clone(),
            verified: attendance.verified,
            distance: attendance.distance_from_location,
            captured_at: attendance.captured_at.to_rfc3339(),
            webauthn_verified: attendance.webauthn_verified,
            device_flag: None,
        });
    }

    if let Some(ref batch) = batch {
        attendance_data = merge_with_batch(attendance_data, batch, &session, &location);
    }

    let excel_data = generate_excel(&attendance_data, &session, &location, batch.as_ref())?;

    let filename = format!(
        "attendance_{}_{}.xlsx",
        session_id.to_hex(),
        Utc::now().format("%Y%m%d_%H%M%S")
    );

    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet".to_string()),
            (header::CONTENT_DISPOSITION, format!("attachment; filename=\"{}\"", filename)),
        ],
        excel_data,
    ))
}

#[derive(Debug, Serialize)]
pub struct AttendanceExportRow {
    pub roll_number: String,
    pub student_name: String,
    pub verified: bool,
    pub distance: f64,
    pub captured_at: String,
    pub webauthn_verified: bool,
    pub device_flag: Option<String>,
}

fn merge_with_batch(
    attendance: Vec<AttendanceExportRow>,
    batch: &Batch,
    _session: &Session,
    _location: &Location,
) -> Vec<AttendanceExportRow> {
    let mut result = Vec::new();
    let submitted: std::collections::HashMap<String, &AttendanceExportRow> = attendance
        .iter()
        .map(|a| (a.roll_number.to_uppercase(), a))
        .collect();

    for student in &batch.students {
        if let Some(att) = submitted.get(&student.roll_number.to_uppercase()) {
            result.push(AttendanceExportRow {
                roll_number: att.roll_number.clone(),
                student_name: att.student_name.clone(),
                verified: att.verified,
                distance: att.distance,
                captured_at: att.captured_at.clone(),
                webauthn_verified: att.webauthn_verified,
                device_flag: att.device_flag.clone(),
            });
        } else {
            result.push(AttendanceExportRow {
                roll_number: student.roll_number.clone(),
                student_name: student.name.clone(),
                verified: false,
                distance: 0.0,
                captured_at: String::new(),
                webauthn_verified: false,
                device_flag: Some("ABSENT".to_string()),
            });
        }
    }

    result
}

fn generate_excel(
    data: &[AttendanceExportRow],
    session: &Session,
    location: &Location,
    batch: Option<&Batch>,
) -> Result<Vec<u8>> {
    use rust_xlsxwriter::Workbook;

    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();

    worksheet.write_string(0, 0, "Roll Number").map_err(|e| AppError::Internal(format!("Excel error: {}", e)))?;
    worksheet.write_string(0, 1, "Student Name").map_err(|e| AppError::Internal(format!("Excel error: {}", e)))?;
    worksheet.write_string(0, 2, "Status").map_err(|e| AppError::Internal(format!("Excel error: {}", e)))?;
    worksheet.write_string(0, 3, "Verified").map_err(|e| AppError::Internal(format!("Excel error: {}", e)))?;
    worksheet.write_string(0, 4, "Distance (m)").map_err(|e| AppError::Internal(format!("Excel error: {}", e)))?;
    worksheet.write_string(0, 5, "Captured At").map_err(|e| AppError::Internal(format!("Excel error: {}", e)))?;
    worksheet.write_string(0, 6, "WebAuthn").map_err(|e| AppError::Internal(format!("Excel error: {}", e)))?;
    worksheet.write_string(0, 7, "Device Flag").map_err(|e| AppError::Internal(format!("Excel error: {}", e)))?;

    let mut row = 1u32;
    for record in data {
        let status = if record.device_flag.as_ref().map(|f| f == "ABSENT").unwrap_or(false) {
            "Absent"
        } else if record.verified {
            "Present"
        } else {
            "Pending"
        };

        worksheet.write_string(row, 0, &record.roll_number).map_err(|e| AppError::Internal(format!("Excel error: {}", e)))?;
        worksheet.write_string(row, 1, &record.student_name).map_err(|e| AppError::Internal(format!("Excel error: {}", e)))?;
        worksheet.write_string(row, 2, status).map_err(|e| AppError::Internal(format!("Excel error: {}", e)))?;
        worksheet.write_string(row, 3, if record.verified { "Yes" } else { "No" }).map_err(|e| AppError::Internal(format!("Excel error: {}", e)))?;
        worksheet.write_number(row, 4, record.distance).map_err(|e| AppError::Internal(format!("Excel error: {}", e)))?;
        worksheet.write_string(row, 5, &record.captured_at).map_err(|e| AppError::Internal(format!("Excel error: {}", e)))?;
        worksheet.write_string(row, 6, if record.webauthn_verified { "Yes" } else { "No" }).map_err(|e| AppError::Internal(format!("Excel error: {}", e)))?;
        worksheet.write_string(row, 7, record.device_flag.as_deref().unwrap_or("")).map_err(|e| AppError::Internal(format!("Excel error: {}", e)))?;
        row += 1;
    }

    worksheet.write_string(row + 2, 0, "Session Information").map_err(|e| AppError::Internal(format!("Excel error: {}", e)))?;
    worksheet.write_string(row + 3, 0, "Location:").map_err(|e| AppError::Internal(format!("Excel error: {}", e)))?;
    worksheet.write_string(row + 3, 1, &location.name).map_err(|e| AppError::Internal(format!("Excel error: {}", e)))?;
    worksheet.write_string(row + 4, 0, "Session ID:").map_err(|e| AppError::Internal(format!("Excel error: {}", e)))?;
    worksheet.write_string(row + 4, 1, session.id.map(|id| id.to_hex()).unwrap_or_default()).map_err(|e| AppError::Internal(format!("Excel error: {}", e)))?;
    worksheet.write_string(row + 5, 0, "Description:").map_err(|e| AppError::Internal(format!("Excel error: {}", e)))?;
    worksheet.write_string(row + 5, 1, session.description.as_deref().unwrap_or("")).map_err(|e| AppError::Internal(format!("Excel error: {}", e)))?;
    
    if let Some(b) = batch {
        worksheet.write_string(row + 6, 0, "Batch:").map_err(|e| AppError::Internal(format!("Excel error: {}", e)))?;
        worksheet.write_string(row + 6, 1, &b.name).map_err(|e| AppError::Internal(format!("Excel error: {}", e)))?;
    }

    worksheet.autofit();

    let data = workbook.save_to_buffer()
        .map_err(|e| AppError::Internal(format!("Excel generation failed: {}", e)))?;

    Ok(data)
}
