use axum::{
    extract::{Json, Path, Query, State},
    http::{header, StatusCode},
    response::IntoResponse,
    Extension,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    constants::*,
    error::{AppError, Result},
    middleware::{
        validators::{validate_request, SessionCreateRequest},
        AuthenticatedAdmin,
    },
    models::{Admin, Attendance, Batch, Location, Session, ShortLink, Student},
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSessionRequest {
    pub location_id: String,
    pub batch_id: Option<String>,
    pub description: Option<String>,
    #[serde(alias = "durationMinutes", alias = "expiresInHours")]
    pub duration_minutes: Option<i32>,
    pub shortlink_mode: Option<String>,
    pub custom_short_code: Option<String>,
    pub existing_short_code: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SessionResponse {
    #[serde(rename = "_id")]
    pub id: String,
    pub token: String,
    #[serde(rename = "locationId")]
    pub location_id: String,
    #[serde(rename = "locationName")]
    pub location_name: Option<String>,
    #[serde(rename = "batchId")]
    pub batch_id: Option<String>,
    #[serde(rename = "batchName")]
    pub batch_name: Option<String>,
    pub description: Option<String>,
    #[serde(rename = "isActive")]
    pub is_active: bool,
    #[serde(rename = "expiresAt")]
    pub expires_at: DateTime<Utc>,
    #[serde(rename = "tokenPrefix")]
    pub token_prefix: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(rename = "attendanceCount")]
    pub attendance_count: Option<i64>,
    #[serde(rename = "shortCode")]
    pub short_code: Option<String>,
}

pub async fn create_session(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Json(payload): Json<CreateSessionRequest>,
) -> Result<impl IntoResponse> {
    let validation_req = SessionCreateRequest {
        location_id: payload.location_id.clone(),
        duration_minutes: payload.duration_minutes,
        batch_id: payload.batch_id.clone(),
        description: payload.description.clone(),
    };
    validate_request(&validation_req)?;

    let mode = payload.shortlink_mode.as_deref().unwrap_or("auto");

    // Pre-validate short link requirements before creating session
    let custom_code_to_use = if mode == "custom" {
        let code = payload
            .custom_short_code
            .as_deref()
            .unwrap_or("")
            .trim()
            .to_lowercase();
        if code.len() < 3
            || code.len() > 20
            || !code.chars().all(|c| c.is_alphanumeric() || c == '-')
        {
            return Err(AppError::BadRequest(
                "Custom short code must be 3-20 alphanumeric characters or hyphens".to_string(),
            ));
        }
        let existing: Option<ShortLink> =
            sqlx::query_as("SELECT * FROM short_links WHERE short_code = $1")
                .bind(&code)
                .fetch_optional(&state.db)
                .await?;
        if existing.is_some() {
            return Err(AppError::BadRequest(format!(
                "Short link '/s/{}' is already taken. Please choose another custom code.",
                code
            )));
        }
        Some(code)
    } else {
        None
    };

    let existing_code_to_use = if mode == "existing" {
        let code = payload.existing_short_code.as_deref().unwrap_or("").trim();
        if code.is_empty() {
            return Err(AppError::BadRequest(
                "Please select an existing short link".to_string(),
            ));
        }
        let existing: Option<ShortLink> =
            sqlx::query_as("SELECT * FROM short_links WHERE short_code = $1")
                .bind(code)
                .fetch_optional(&state.db)
                .await?;
        if existing.is_none() {
            return Err(AppError::NotFound(format!(
                "Existing short link '/s/{}' not found",
                code
            )));
        }
        Some(code.to_string())
    } else {
        None
    };

    let location_id = Uuid::parse_str(&payload.location_id)
        .map_err(|e| AppError::BadRequest(format!("Invalid location ID: {}", e)))?;

    let batch_id = payload
        .batch_id
        .as_ref()
        .and_then(|id| Uuid::parse_str(id).ok());

    let token = Session::generate_token();
    let duration_minutes = payload.duration_minutes.unwrap_or(30) as i64;
    let expires_at = Utc::now() + chrono::Duration::minutes(duration_minutes);

    let session: Session = sqlx::query_as(
        "INSERT INTO sessions (id, location_id, batch_id, token_hash, token_prefix, description, created_by, is_active, expires_at, rotation_count, totp_secret, created_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, true, $8, 0, $9, now()) \
         RETURNING *",
    )
    .bind(Uuid::new_v4())
    .bind(location_id)
    .bind(batch_id)
    .bind(Session::hash_token(&token))
    .bind(Session::get_token_prefix(&token))
    .bind(&payload.description)
    .bind(auth.id)
    .bind(expires_at)
    .bind(Session::generate_totp_secret())
    .fetch_one(&state.db)
    .await?;

    let session_id = session.id;

    // Atomically create or assign short link
    let created_short_code = match mode {
        "custom" => {
            let code = custom_code_to_use.unwrap();
            let insert_res: sqlx::Result<ShortLink> = sqlx::query_as(
                "INSERT INTO short_links (id, short_code, session_id, created_by, is_active, expires_at, click_count, created_at) \
                 VALUES ($1, $2, $3, $4, true, $5, 0, now()) RETURNING *",
            )
            .bind(Uuid::new_v4())
            .bind(&code)
            .bind(session_id)
            .bind(auth.id)
            .bind(session.expires_at)
            .fetch_one(&state.db)
            .await;
            if let Err(e) = insert_res {
                let _ = sqlx::query("DELETE FROM sessions WHERE id = $1")
                    .bind(session_id)
                    .execute(&state.db)
                    .await;
                return Err(AppError::Internal(format!(
                    "Failed to create short link: {}",
                    e
                )));
            }
            code
        }
        "existing" => {
            let code = existing_code_to_use.unwrap();
            let update_res = sqlx::query(
                "UPDATE short_links SET session_id = $1, is_active = true, expires_at = $2 WHERE short_code = $3",
            )
            .bind(session_id)
            .bind(session.expires_at)
            .bind(&code)
            .execute(&state.db)
            .await;
            if let Err(e) = update_res {
                let _ = sqlx::query("DELETE FROM sessions WHERE id = $1")
                    .bind(session_id)
                    .execute(&state.db)
                    .await;
                return Err(AppError::Internal(format!(
                    "Failed to attach short link: {}",
                    e
                )));
            }
            code
        }
        _ => {
            let mut attempts = 0;
            let code = loop {
                let candidate = ShortLink::generate_short_code(6);
                let existing: Option<ShortLink> =
                    sqlx::query_as("SELECT * FROM short_links WHERE short_code = $1")
                        .bind(&candidate)
                        .fetch_optional(&state.db)
                        .await?;
                if existing.is_none() {
                    break candidate;
                }
                attempts += 1;
                if attempts > 10 {
                    let _ = sqlx::query("DELETE FROM sessions WHERE id = $1")
                        .bind(session_id)
                        .execute(&state.db)
                        .await;
                    return Err(AppError::Internal(
                        "Failed to generate unique short code".to_string(),
                    ));
                }
            };
            let insert_res: sqlx::Result<ShortLink> = sqlx::query_as(
                "INSERT INTO short_links (id, short_code, session_id, created_by, is_active, expires_at, click_count, created_at) \
                 VALUES ($1, $2, $3, $4, true, $5, 0, now()) RETURNING *",
            )
            .bind(Uuid::new_v4())
            .bind(&code)
            .bind(session_id)
            .bind(auth.id)
            .bind(session.expires_at)
            .fetch_one(&state.db)
            .await;
            if let Err(e) = insert_res {
                let _ = sqlx::query("DELETE FROM sessions WHERE id = $1")
                    .bind(session_id)
                    .execute(&state.db)
                    .await;
                return Err(AppError::Internal(format!(
                    "Failed to create short link: {}",
                    e
                )));
            }
            code
        }
    };

    let location: Option<Location> = sqlx::query_as("SELECT * FROM locations WHERE id = $1")
        .bind(location_id)
        .fetch_optional(&state.db)
        .await?;

    Ok((
        StatusCode::CREATED,
        Json(SessionResponse {
            id: session_id.to_string(),
            token,
            location_id: payload.location_id,
            location_name: location.as_ref().map(|l| l.name.clone()),
            batch_id: session.batch_id.map(|b| b.to_string()),
            batch_name: None,
            description: session.description,
            is_active: true,
            expires_at: session.expires_at,
            token_prefix: Some(session.token_prefix),
            created_at: Some(session.created_at),
            attendance_count: Some(0),
            short_code: Some(created_short_code),
        }),
    ))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetSessionsQuery {
    #[serde(alias = "location_id")]
    pub location_id: Option<String>,
    pub date: Option<String>,
}

pub async fn get_sessions(
    State(state): State<Arc<crate::AppState>>,
    Extension(_auth): Extension<AuthenticatedAdmin>,
    Query(query): Query<GetSessionsQuery>,
) -> Result<impl IntoResponse> {
    let location_filter = query
        .location_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .and_then(|s| Uuid::parse_str(s).ok());

    let (start_utc, end_utc) = if let Some(ref date_str) = query.date {
        let trimmed = date_str.trim();
        if trimmed.is_empty() {
            (None, None)
        } else if let Ok(naive_date) = chrono::NaiveDate::parse_from_str(trimmed, "%Y-%m-%d") {
            let start = naive_date
                .and_hms_opt(0, 0, 0)
                .map(|d| DateTime::<Utc>::from_naive_utc_and_offset(d, Utc));
            let end = naive_date
                .and_hms_opt(23, 59, 59)
                .map(|d| DateTime::<Utc>::from_naive_utc_and_offset(d, Utc));
            (start, end)
        } else {
            (None, None)
        }
    } else {
        (None, None)
    };

    let sessions: Vec<Session> = sqlx::query_as(
        "SELECT * FROM sessions \
         WHERE ($1::uuid IS NULL OR location_id = $1) \
           AND ($2::timestamptz IS NULL OR created_at >= $2) \
           AND ($3::timestamptz IS NULL OR created_at <= $3) \
         ORDER BY created_at DESC \
         LIMIT $4",
    )
    .bind(location_filter)
    .bind(start_utc)
    .bind(end_utc)
    .bind(DASHBOARD_PAGE_SIZE)
    .fetch_all(&state.db)
    .await?;

    let mut sessions_list = Vec::with_capacity(sessions.len());

    for session in sessions {
        let location: Option<Location> = sqlx::query_as("SELECT * FROM locations WHERE id = $1")
            .bind(session.location_id)
            .fetch_optional(&state.db)
            .await?;

        let batch: Option<Batch> = if let Some(batch_id) = session.batch_id {
            sqlx::query_as("SELECT * FROM batches WHERE id = $1")
                .bind(batch_id)
                .fetch_optional(&state.db)
                .await?
        } else {
            None
        };

        let attendance_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM attendances WHERE session_id = $1")
                .bind(session.id)
                .fetch_one(&state.db)
                .await?;

        let short_link: Option<ShortLink> =
            sqlx::query_as("SELECT * FROM short_links WHERE session_id = $1 AND is_active = true")
                .bind(session.id)
                .fetch_optional(&state.db)
                .await?;

        sessions_list.push(SessionResponse {
            id: session.id.to_string(),
            token: String::new(),
            location_id: session.location_id.to_string(),
            location_name: location.map(|l| l.name),
            batch_id: session.batch_id.map(|b| b.to_string()),
            batch_name: batch.map(|b| b.name),
            description: session.description,
            is_active: session.is_active,
            expires_at: session.expires_at,
            token_prefix: Some(session.token_prefix),
            created_at: Some(session.created_at),
            attendance_count: Some(attendance_count),
            short_code: short_link.map(|s| s.short_code),
        });
    }

    Ok(Json(sessions_list))
}

pub async fn get_session(
    State(state): State<Arc<crate::AppState>>,
    Extension(_auth): Extension<AuthenticatedAdmin>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse> {
    let session_id = Uuid::parse_str(&id)
        .map_err(|e| AppError::BadRequest(format!("Invalid session ID: {}", e)))?;

    let session: Session = sqlx::query_as("SELECT * FROM sessions WHERE id = $1")
        .bind(session_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound("Session not found".to_string()))?;

    let location: Option<Location> = sqlx::query_as("SELECT * FROM locations WHERE id = $1")
        .bind(session.location_id)
        .fetch_optional(&state.db)
        .await?;

    let batch: Option<Batch> = if let Some(batch_id) = session.batch_id {
        sqlx::query_as("SELECT * FROM batches WHERE id = $1")
            .bind(batch_id)
            .fetch_optional(&state.db)
            .await?
    } else {
        None
    };

    let attendance_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM attendances WHERE session_id = $1")
            .bind(session.id)
            .fetch_one(&state.db)
            .await?;

    let short_link: Option<ShortLink> =
        sqlx::query_as("SELECT * FROM short_links WHERE session_id = $1 AND is_active = true")
            .bind(session.id)
            .fetch_optional(&state.db)
            .await?;

    Ok(Json(SessionResponse {
        id: session.id.to_string(),
        token: String::new(),
        location_id: session.location_id.to_string(),
        location_name: location.map(|l| l.name),
        batch_id: session.batch_id.map(|b| b.to_string()),
        batch_name: batch.map(|b| b.name),
        description: session.description,
        is_active: session.is_active,
        expires_at: session.expires_at,
        token_prefix: Some(session.token_prefix),
        created_at: Some(session.created_at),
        attendance_count: Some(attendance_count),
        short_code: short_link.map(|s| s.short_code),
    }))
}

pub async fn deactivate_session(
    State(state): State<Arc<crate::AppState>>,
    Extension(_auth): Extension<AuthenticatedAdmin>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse> {
    let session_id = Uuid::parse_str(&id)
        .map_err(|e| AppError::BadRequest(format!("Invalid session ID: {}", e)))?;

    sqlx::query("UPDATE sessions SET is_active = false WHERE id = $1")
        .bind(session_id)
        .execute(&state.db)
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
    // Parse session ID
    let session_id = Uuid::parse_str(&id)
        .map_err(|e| AppError::BadRequest(format!("Invalid session ID: {}", e)))?;

    // First verify session ownership (so cross-admin deletions get 404, not 401)
    let _session: Session =
        sqlx::query_as("SELECT * FROM sessions WHERE id = $1 AND created_by = $2")
            .bind(session_id)
            .bind(auth.id)
            .fetch_optional(&state.db)
            .await?
            .ok_or_else(|| AppError::NotFound("Session not found".to_string()))?;

    // Verify password - re-fetch admin with password field
    let admin: Admin = sqlx::query_as("SELECT * FROM admins WHERE id = $1")
        .bind(auth.id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::Unauthorized("Admin not found".to_string()))?;

    // Verify the password using Admin::verify_password()
    let password_valid = admin
        .verify_password(&payload.password)
        .map_err(|e| AppError::Internal(format!("Password verification failed: {}", e)))?;

    if !password_valid {
        return Err(AppError::Unauthorized("Incorrect password".to_string()));
    }

    // Find all attendance records with photos before deleting them
    let photo_ids_to_delete: Vec<String> = sqlx::query_scalar(
        "SELECT photo_public_id FROM attendances WHERE session_id = $1 AND photo_public_id <> ''",
    )
    .bind(session_id)
    .fetch_all(&state.db)
    .await?;

    // Delete photos from storage
    for public_id in &photo_ids_to_delete {
        match state.storage.provider().delete(public_id).await {
            Ok(_) => {}
            Err(e) => {
                tracing::warn!("Failed to delete photo {}: {}", public_id, e);
            }
        }
    }

    // Delete all attendance records for this session
    sqlx::query("DELETE FROM attendances WHERE session_id = $1")
        .bind(session_id)
        .execute(&state.db)
        .await?;

    // Detach any short links that pointed to this session so they can be reattached
    sqlx::query(
        "UPDATE short_links SET session_id = NULL, is_active = false WHERE session_id = $1",
    )
    .bind(session_id)
    .execute(&state.db)
    .await?;

    // Delete the session
    sqlx::query("DELETE FROM sessions WHERE id = $1")
        .bind(session_id)
        .execute(&state.db)
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
    let session_id = Uuid::parse_str(&id)
        .map_err(|e| AppError::BadRequest(format!("Invalid session ID: {}", e)))?;

    let session: Session = sqlx::query_as("SELECT * FROM sessions WHERE id = $1")
        .bind(session_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound("Session not found".to_string()))?;

    let new_token = Session::generate_token();

    sqlx::query(
        "UPDATE sessions SET token_hash = $1, token_prefix = $2, totp_secret = $3, rotation_count = $4 WHERE id = $5",
    )
    .bind(Session::hash_token(&new_token))
    .bind(Session::get_token_prefix(&new_token))
    .bind(Session::generate_totp_secret())
    .bind(session.rotation_count + 1)
    .bind(session_id)
    .execute(&state.db)
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
    let session_id = Uuid::parse_str(&id)
        .map_err(|e| AppError::BadRequest(format!("Invalid session ID: {}", e)))?;

    let session: Session =
        sqlx::query_as("SELECT * FROM sessions WHERE id = $1 AND created_by = $2")
            .bind(session_id)
            .bind(auth.id)
            .fetch_optional(&state.db)
            .await?
            .ok_or_else(|| AppError::NotFound("Session not found".to_string()))?;

    let location: Location = sqlx::query_as("SELECT * FROM locations WHERE id = $1")
        .bind(session.location_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound("Location not found".to_string()))?;

    let batch: Option<Batch> = if let Some(batch_id) = session.batch_id {
        sqlx::query_as("SELECT * FROM batches WHERE id = $1")
            .bind(batch_id)
            .fetch_optional(&state.db)
            .await?
    } else {
        None
    };

    let students: Vec<Student> = if let Some(batch_id) = session.batch_id {
        sqlx::query_as("SELECT * FROM students WHERE batch_id = $1 ORDER BY position")
            .bind(batch_id)
            .fetch_all(&state.db)
            .await?
    } else {
        Vec::new()
    };

    let attendances: Vec<Attendance> =
        sqlx::query_as("SELECT * FROM attendances WHERE session_id = $1 ORDER BY captured_at ASC")
            .bind(session_id)
            .fetch_all(&state.db)
            .await?;

    let mut attendance_data: Vec<AttendanceExportRow> = attendances
        .into_iter()
        .map(|attendance| AttendanceExportRow {
            roll_number: attendance.roll_number.clone(),
            student_name: attendance.student_name.clone(),
            verified: attendance.verified,
            distance: attendance.distance_from_location,
            captured_at: attendance.captured_at.to_rfc3339(),
            webauthn_verified: attendance.webauthn_verified,
            device_flag: None,
        })
        .collect();

    if batch.is_some() {
        attendance_data = merge_with_batch(attendance_data, &students);
    }

    let excel_data = generate_excel(&attendance_data, &session, &location, batch.as_ref())?;

    let filename = format!(
        "attendance_{}_{}.xlsx",
        session_id,
        Utc::now().format("%Y%m%d_%H%M%S")
    );

    Ok((
        StatusCode::OK,
        [
            (
                header::CONTENT_TYPE,
                "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet".to_string(),
            ),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{}\"", filename),
            ),
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
    students: &[Student],
) -> Vec<AttendanceExportRow> {
    let mut result = Vec::new();
    let submitted: std::collections::HashMap<String, &AttendanceExportRow> = attendance
        .iter()
        .map(|a| (a.roll_number.to_uppercase(), a))
        .collect();

    for student in students {
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

    worksheet
        .write_string(0, 0, "Roll Number")
        .map_err(|e| AppError::Internal(format!("Excel error: {}", e)))?;
    worksheet
        .write_string(0, 1, "Student Name")
        .map_err(|e| AppError::Internal(format!("Excel error: {}", e)))?;
    worksheet
        .write_string(0, 2, "Status")
        .map_err(|e| AppError::Internal(format!("Excel error: {}", e)))?;
    worksheet
        .write_string(0, 3, "Verified")
        .map_err(|e| AppError::Internal(format!("Excel error: {}", e)))?;
    worksheet
        .write_string(0, 4, "Distance (m)")
        .map_err(|e| AppError::Internal(format!("Excel error: {}", e)))?;
    worksheet
        .write_string(0, 5, "Captured At")
        .map_err(|e| AppError::Internal(format!("Excel error: {}", e)))?;
    worksheet
        .write_string(0, 6, "WebAuthn")
        .map_err(|e| AppError::Internal(format!("Excel error: {}", e)))?;
    worksheet
        .write_string(0, 7, "Device Flag")
        .map_err(|e| AppError::Internal(format!("Excel error: {}", e)))?;

    let mut row = 1u32;
    for record in data {
        let status = if record
            .device_flag
            .as_ref()
            .map(|f| f == "ABSENT")
            .unwrap_or(false)
        {
            "Absent"
        } else if record.verified {
            "Present"
        } else {
            "Pending"
        };

        worksheet
            .write_string(row, 0, &record.roll_number)
            .map_err(|e| AppError::Internal(format!("Excel error: {}", e)))?;
        worksheet
            .write_string(row, 1, &record.student_name)
            .map_err(|e| AppError::Internal(format!("Excel error: {}", e)))?;
        worksheet
            .write_string(row, 2, status)
            .map_err(|e| AppError::Internal(format!("Excel error: {}", e)))?;
        worksheet
            .write_string(row, 3, if record.verified { "Yes" } else { "No" })
            .map_err(|e| AppError::Internal(format!("Excel error: {}", e)))?;
        worksheet
            .write_number(row, 4, record.distance)
            .map_err(|e| AppError::Internal(format!("Excel error: {}", e)))?;
        worksheet
            .write_string(row, 5, &record.captured_at)
            .map_err(|e| AppError::Internal(format!("Excel error: {}", e)))?;
        worksheet
            .write_string(
                row,
                6,
                if record.webauthn_verified {
                    "Yes"
                } else {
                    "No"
                },
            )
            .map_err(|e| AppError::Internal(format!("Excel error: {}", e)))?;
        worksheet
            .write_string(row, 7, record.device_flag.as_deref().unwrap_or(""))
            .map_err(|e| AppError::Internal(format!("Excel error: {}", e)))?;
        row += 1;
    }

    worksheet
        .write_string(row + 2, 0, "Session Information")
        .map_err(|e| AppError::Internal(format!("Excel error: {}", e)))?;
    worksheet
        .write_string(row + 3, 0, "Location:")
        .map_err(|e| AppError::Internal(format!("Excel error: {}", e)))?;
    worksheet
        .write_string(row + 3, 1, &location.name)
        .map_err(|e| AppError::Internal(format!("Excel error: {}", e)))?;
    worksheet
        .write_string(row + 4, 0, "Session ID:")
        .map_err(|e| AppError::Internal(format!("Excel error: {}", e)))?;
    worksheet
        .write_string(row + 4, 1, session.id.to_string())
        .map_err(|e| AppError::Internal(format!("Excel error: {}", e)))?;
    worksheet
        .write_string(row + 5, 0, "Description:")
        .map_err(|e| AppError::Internal(format!("Excel error: {}", e)))?;
    worksheet
        .write_string(row + 5, 1, session.description.as_deref().unwrap_or(""))
        .map_err(|e| AppError::Internal(format!("Excel error: {}", e)))?;

    if let Some(b) = batch {
        worksheet
            .write_string(row + 6, 0, "Batch:")
            .map_err(|e| AppError::Internal(format!("Excel error: {}", e)))?;
        worksheet
            .write_string(row + 6, 1, &b.name)
            .map_err(|e| AppError::Internal(format!("Excel error: {}", e)))?;
    }

    worksheet.autofit();

    let data = workbook
        .save_to_buffer()
        .map_err(|e| AppError::Internal(format!("Excel generation failed: {}", e)))?;

    Ok(data)
}
