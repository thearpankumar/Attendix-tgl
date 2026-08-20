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
    controllers::session_records::{
        query_session_records, session_record_stats_for, SessionRecordQueryParams,
    },
    error::{AppError, Result},
    middleware::{
        validators::{validate_request, SessionCreateRequest},
        AuthenticatedAdmin,
    },
    models::{
        Admin, Attendance, AttendanceStatus, Batch, ExcelBatch, Location, Session, ShortLink,
        Student,
    },
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSessionRequest {
    /// Required for a normal session (geofenced); irrelevant for an exam
    /// session, which is marked manually and has no location.
    pub location_id: Option<String>,
    /// Present + valid on every request; whether it's *required* to be
    /// non-empty depends on whether this is an exam session (mandatory) or
    /// a normal self-service session (optional) — see `is_exam_session()`.
    pub batch_id: Option<String>,
    /// One or more mentor IDs. Non-empty signals an exam session — see
    /// `SessionCreateRequest::is_exam_session`.
    #[serde(default)]
    pub assigned_admin_ids: Vec<String>,
    pub college_name: Option<String>,
    pub starts_at: Option<String>,
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
    pub location_id: Option<String>,
    #[serde(rename = "locationName")]
    pub location_name: Option<String>,
    #[serde(rename = "batchId")]
    pub batch_id: Option<String>,
    #[serde(rename = "batchName")]
    pub batch_name: Option<String>,
    /// Excel-upload (mentor) sessions only — lets the frontend correlate a
    /// session back to its 1:1 "session batch" entry in `GET /batches`
    /// (`batchId` above is manual-batch-only) for filter cascading.
    #[serde(rename = "excelBatchId")]
    pub excel_batch_id: Option<String>,
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
    #[serde(rename = "assignedAdminIds")]
    pub assigned_admin_ids: Vec<String>,
    #[serde(rename = "assignedAdminNames")]
    pub assigned_admin_names: Vec<String>,
    #[serde(rename = "collegeName")]
    pub college_name: Option<String>,
    #[serde(rename = "startsAt")]
    pub starts_at: Option<DateTime<Utc>>,
    /// Excel-upload (mentor) sessions only — `None` for GPS/manual-batch
    /// sessions, which have no track concept.
    pub track: Option<String>,
    #[serde(rename = "sessionTimeRaw")]
    pub session_time_raw: Option<String>,
    /// `"not_started" | "partial" | "complete" | "no_roster"` — see
    /// `SessionRecordStats::marking_status`.
    #[serde(rename = "markingStatus")]
    pub marking_status: String,
    #[serde(rename = "rosterSize")]
    pub roster_size: Option<i64>,
    #[serde(rename = "markedCount")]
    pub marked_count: Option<i64>,
    #[serde(rename = "attendancePercent")]
    pub attendance_percent: Option<f64>,
}

/// Fetches the mentors assigned to a session (id + display name), ordered by
/// name. Shared by every handler that returns a `SessionResponse`.
async fn fetch_assigned_mentors(
    db: &sqlx::PgPool,
    session_id: Uuid,
) -> Result<(Vec<String>, Vec<String>)> {
    let mentors: Vec<Admin> = sqlx::query_as(
        "SELECT a.* FROM admins a \
         JOIN session_admins sa ON sa.admin_id = a.id \
         WHERE sa.session_id = $1 \
         ORDER BY a.full_name NULLS LAST, a.username",
    )
    .bind(session_id)
    .fetch_all(db)
    .await?;

    let ids = mentors.iter().map(|m| m.id.to_string()).collect();
    let names = mentors
        .into_iter()
        .map(|m| m.full_name.unwrap_or(m.username))
        .collect();
    Ok((ids, names))
}

/// Role-scoped session lookup shared by every session read/sub-resource
/// handler: super-admins see every session regardless of who created it,
/// mentors only ever see sessions a super-admin has explicitly assigned to
/// them. Never trust a client-supplied filter for this — the role check
/// happens at the SQL level.
pub async fn find_session_for_admin(
    db: &sqlx::PgPool,
    session_id: Uuid,
    auth: &AuthenticatedAdmin,
) -> Result<Session> {
    sqlx::query_as(
        "SELECT * FROM sessions WHERE id = $1 \
         AND ($2 = 'super_admin' \
              OR EXISTS ( \
                    SELECT 1 FROM session_admins sa WHERE sa.session_id = sessions.id AND sa.admin_id = $3 \
              ))",
    )
    .bind(session_id)
    .bind(&auth.role)
    .bind(auth.id)
    .fetch_optional(db)
    .await?
    .ok_or_else(|| AppError::NotFound("Session not found".to_string()))
}

pub async fn create_session(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Json(payload): Json<CreateSessionRequest>,
) -> Result<impl IntoResponse> {
    // Only super-admins create sessions. Whether a batch/mentor/college/time
    // is mandatory depends on which of the two shapes this request is (see
    // SessionCreateRequest::is_exam_session): an exam session (assigned to a
    // mentor) requires all four; a normal self-service session keeps the
    // original optional-batch behavior. Mentors never create sessions of
    // either kind — they only ever see sessions assigned to them (see
    // get_sessions/get_session's role branch).
    auth.require_role(ROLE_SUPER_ADMIN)?;

    let validation_req = SessionCreateRequest {
        location_id: payload.location_id.clone(),
        duration_minutes: payload.duration_minutes,
        batch_id: payload.batch_id.clone(),
        assigned_admin_ids: payload.assigned_admin_ids.clone(),
        college_name: payload.college_name.clone(),
        starts_at: payload.starts_at.clone(),
        description: payload.description.clone(),
    };
    validate_request(&validation_req)?;
    validation_req
        .validate_with_objectids()
        .map_err(AppError::from)?;

    let is_exam_session = validation_req.is_exam_session();

    // Exam session: mentors/college/start-time are all mandatory and
    // validated together (validate_with_objectids already guarantees they're
    // present and well-formed above); location does not apply — exam
    // attendance is marked manually, not geofenced. Normal session: none of
    // that applies, but location is mandatory.
    let (assigned_admin_ids, location_id, college_name, starts_at) = if is_exam_session {
        let mut assigned_admin_ids = Vec::with_capacity(payload.assigned_admin_ids.len());
        for raw_id in &payload.assigned_admin_ids {
            let raw_id = raw_id.trim();
            if raw_id.is_empty() {
                continue;
            }
            let mentor_id = Uuid::parse_str(raw_id)
                .map_err(|e| AppError::BadRequest(format!("Invalid mentor ID: {}", e)))?;

            // Each assignee must exist, still hold the mentor ("admin") role,
            // and be active — otherwise a session could be handed to a
            // super-admin account or to a mentor who has since been
            // deactivated.
            sqlx::query_scalar::<_, Uuid>(
                "SELECT id FROM admins WHERE id = $1 AND role = $2 AND is_active = true",
            )
            .bind(mentor_id)
            .bind(ROLE_ADMIN)
            .fetch_optional(&state.db)
            .await?
            .ok_or_else(|| {
                AppError::BadRequest(
                    "Every assigned mentor must be an active admin/mentor account".to_string(),
                )
            })?;
            assigned_admin_ids.push(mentor_id);
        }

        let starts_at =
            DateTime::parse_from_rfc3339(payload.starts_at.as_deref().unwrap_or_default())
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|e| AppError::BadRequest(format!("Invalid start time: {}", e)))?;

        (
            assigned_admin_ids,
            None,
            payload.college_name.clone(),
            Some(starts_at),
        )
    } else {
        let location_id = Uuid::parse_str(payload.location_id.as_deref().unwrap_or_default())
            .map_err(|e| AppError::BadRequest(format!("Invalid location ID: {}", e)))?;
        (Vec::new(), Some(location_id), None, None)
    };

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

    let batch_id = payload
        .batch_id
        .as_ref()
        .and_then(|id| Uuid::parse_str(id).ok());

    let token = Session::generate_token();
    let duration_minutes = payload.duration_minutes.unwrap_or(30) as i64;
    // For an exam session, the window is anchored to the scheduled start
    // time, not to whenever this request happens to run — otherwise a
    // session created ahead of time (or with any admin/network delay) would
    // silently eat into the exam's actual duration, closing well before the
    // scheduled end. A normal self-check-in session has no starts_at, so it
    // opens immediately as before.
    let expires_at =
        starts_at.unwrap_or_else(Utc::now) + chrono::Duration::minutes(duration_minutes);

    let session: Session = sqlx::query_as(
        "INSERT INTO sessions (id, location_id, batch_id, token_hash, token_prefix, description, created_by, college_name, starts_at, is_active, expires_at, rotation_count, totp_secret, created_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, true, $10, 0, $11, now()) \
         RETURNING *",
    )
    .bind(Uuid::new_v4())
    .bind(location_id)
    .bind(batch_id)
    .bind(Session::hash_token(&token))
    .bind(Session::get_token_prefix(&token))
    .bind(&payload.description)
    .bind(auth.id)
    .bind(&college_name)
    .bind(starts_at)
    .bind(expires_at)
    .bind(Session::generate_totp_secret())
    .fetch_one(&state.db)
    .await?;

    let session_id = session.id;

    let mut assigned_admin_names = Vec::with_capacity(assigned_admin_ids.len());
    for mentor_id in &assigned_admin_ids {
        sqlx::query("INSERT INTO session_admins (session_id, admin_id) VALUES ($1, $2)")
            .bind(session_id)
            .bind(mentor_id)
            .execute(&state.db)
            .await?;
    }
    if !assigned_admin_ids.is_empty() {
        let (_, names) = fetch_assigned_mentors(&state.db, session_id).await?;
        assigned_admin_names = names;
    }

    // Atomically create or assign short link. Exam sessions have no
    // self-service check-in flow to link to, so "none" skips this entirely.
    let created_short_code: Option<String> = match mode {
        "none" => None,
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
            Some(code)
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
            Some(code)
        }
        _ => {
            let mut attempts = 0;
            let code = loop {
                let candidate = ShortLink::generate_short_code(10);
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
            Some(code)
        }
    };

    let location: Option<Location> = match location_id {
        Some(id) => {
            sqlx::query_as("SELECT * FROM locations WHERE id = $1")
                .bind(id)
                .fetch_optional(&state.db)
                .await?
        }
        None => None,
    };

    Ok((
        StatusCode::CREATED,
        Json(SessionResponse {
            id: session_id.to_string(),
            token,
            location_id: session.location_id.map(|id| id.to_string()),
            location_name: location.as_ref().map(|l| l.name.clone()),
            batch_id: session.batch_id.map(|b| b.to_string()),
            excel_batch_id: session.excel_batch_id.map(|b| b.to_string()),
            batch_name: None,
            description: session.description,
            is_active: true,
            expires_at: session.expires_at,
            token_prefix: Some(session.token_prefix),
            created_at: Some(session.created_at),
            attendance_count: Some(0),
            short_code: created_short_code,
            assigned_admin_ids: assigned_admin_ids.iter().map(|id| id.to_string()).collect(),
            assigned_admin_names,
            college_name: session.college_name,
            starts_at: session.starts_at,
            // `create_session` never sets `excel_batch_id` (only the
            // Excel-upload commit flow does), so there's no track/time-slot
            // to report; a freshly created session also has no attendance
            // yet, so marking status is trivially known without a query.
            track: None,
            session_time_raw: None,
            marking_status: if batch_id.is_some() {
                "not_started"
            } else {
                "no_roster"
            }
            .to_string(),
            roster_size: None,
            marked_count: None,
            attendance_percent: None,
        }),
    ))
}

pub async fn get_sessions(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Query(query): Query<SessionRecordQueryParams>,
) -> Result<impl IntoResponse> {
    // Infinite-scroll paging: a client-supplied `limit` is clamped to
    // SESSIONS_LIST_PAGE_SIZE (the Sessions page's scroll-triggered fetches
    // pass a much smaller one per request); omitting it keeps the original
    // unpaginated behavior for every other caller (e.g. dashboards) that
    // just wants "everything, capped generously".
    let limit = query
        .limit
        .map(|l| l.clamp(1, SESSIONS_LIST_PAGE_SIZE))
        .unwrap_or(SESSIONS_LIST_PAGE_SIZE);
    let offset = query.offset.unwrap_or(0).max(0);
    let filters = query.into_filters();

    // `query_session_records` is also the filter-matching engine for
    // /stats-overview and /export-filtered — using it here too guarantees
    // "what's shown in the list" and "what those two endpoints count/export"
    // never drift apart. Records already carry track/class/time-slot/roster
    // progress, so the per-session loop below only needs to fill in the
    // fields that live on `sessions`/`batches`/`short_links` directly.
    let records = query_session_records(&state.db, &auth, &filters, limit, offset).await?;

    let mut sessions_list = Vec::with_capacity(records.len());

    for record in &records {
        let Some(session): Option<Session> = sqlx::query_as("SELECT * FROM sessions WHERE id = $1")
            .bind(record.id)
            .fetch_optional(&state.db)
            .await?
        else {
            continue;
        };

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

        let (assigned_admin_ids, assigned_admin_names) =
            fetch_assigned_mentors(&state.db, session.id).await?;

        sessions_list.push(SessionResponse {
            id: session.id.to_string(),
            token: String::new(),
            location_id: session.location_id.map(|id| id.to_string()),
            // `record.class_label` is already `COALESCE(excel_batches.class_label, locations.name)` —
            // Excel-based (mentor) sessions have neither a `locations` row
            // nor a `batches` row, so the "class" column from the roster
            // upload IS the location; this surfaces it as `locationName`
            // instead of leaving the UI to show "Unknown".
            location_name: record.class_label.clone(),
            batch_id: session.batch_id.map(|b| b.to_string()),
            excel_batch_id: session.excel_batch_id.map(|b| b.to_string()),
            batch_name: batch.map(|b| b.name),
            description: session.description,
            is_active: session.is_active,
            expires_at: session.expires_at,
            token_prefix: Some(session.token_prefix),
            created_at: Some(session.created_at),
            attendance_count: Some(attendance_count),
            short_code: short_link.map(|s| s.short_code),
            assigned_admin_ids,
            assigned_admin_names,
            college_name: session.college_name,
            starts_at: session.starts_at,
            track: record.track.clone(),
            session_time_raw: record.session_time_raw.clone(),
            marking_status: record.marking_status().to_string(),
            roster_size: (record.roster_size > 0).then_some(record.roster_size),
            marked_count: (record.roster_size > 0).then_some(record.marked_count),
            attendance_percent: record.attendance_percent(),
        });
    }

    Ok(Json(sessions_list))
}

pub async fn get_session(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse> {
    let session_id = Uuid::parse_str(&id)
        .map_err(|e| AppError::BadRequest(format!("Invalid session ID: {}", e)))?;

    let session = find_session_for_admin(&state.db, session_id, &auth).await?;

    let batch: Option<Batch> = if let Some(batch_id) = session.batch_id {
        sqlx::query_as("SELECT * FROM batches WHERE id = $1")
            .bind(batch_id)
            .fetch_optional(&state.db)
            .await?
    } else {
        None
    };

    let record = session_record_stats_for(&state.db, &session).await?;

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

    let (assigned_admin_ids, assigned_admin_names) =
        fetch_assigned_mentors(&state.db, session.id).await?;

    Ok(Json(SessionResponse {
        id: session.id.to_string(),
        token: String::new(),
        location_id: session.location_id.map(|id| id.to_string()),
        // Excel-based (mentor) sessions have neither a `locations` row nor a
        // `batches` row — the "class" column from the roster upload IS the
        // location, so `record.class_label` (already
        // `COALESCE(excel_batches.class_label, locations.name)`) surfaces it
        // as `locationName` instead of leaving the UI to show "Unknown".
        location_name: record.class_label.clone(),
        batch_id: session.batch_id.map(|b| b.to_string()),
        excel_batch_id: session.excel_batch_id.map(|b| b.to_string()),
        batch_name: batch.map(|b| b.name),
        description: session.description,
        is_active: session.is_active,
        expires_at: session.expires_at,
        token_prefix: Some(session.token_prefix),
        created_at: Some(session.created_at),
        attendance_count: Some(attendance_count),
        short_code: short_link.map(|s| s.short_code),
        assigned_admin_ids,
        assigned_admin_names,
        college_name: session.college_name,
        starts_at: session.starts_at,
        track: record.track.clone(),
        session_time_raw: record.session_time_raw.clone(),
        marking_status: record.marking_status().to_string(),
        roster_size: (record.roster_size > 0).then_some(record.roster_size),
        marked_count: (record.roster_size > 0).then_some(record.marked_count),
        attendance_percent: record.attendance_percent(),
    }))
}

pub async fn deactivate_session(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse> {
    let session_id = Uuid::parse_str(&id)
        .map_err(|e| AppError::BadRequest(format!("Invalid session ID: {}", e)))?;

    let result = sqlx::query(
        "UPDATE sessions SET is_active = false WHERE id = $1 AND ($2 = 'super_admin' OR created_by = $3)",
    )
    .bind(session_id)
    .bind(&auth.role)
    .bind(auth.id)
    .execute(&state.db)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Session not found".to_string()));
    }

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

    // Any super-admin may delete any session; mentors never reach this route
    // via the frontend, but are denied outright rather than silently scoped.
    let _session: Session = sqlx::query_as(
        "SELECT * FROM sessions WHERE id = $1 AND ($2 = 'super_admin' OR created_by = $3)",
    )
    .bind(session_id)
    .bind(&auth.role)
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

    delete_session_records(&state, session_id).await?;

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "success": true,
            "message": "Session and all attendance records deleted successfully"
        })),
    ))
}

/// Deletes a session's attendance photos, attendance records, short-link
/// attachments, and the session row itself. Shared by the single-session
/// and bulk-delete handlers, both of which have already verified ownership
/// and the caller's password before calling this.
async fn delete_session_records(state: &crate::AppState, session_id: Uuid) -> Result<()> {
    // Find all attendance records with photos before deleting them
    let photo_ids_to_delete: Vec<String> = sqlx::query_scalar(
        "SELECT photo_public_id FROM attendances WHERE session_id = $1 AND photo_public_id <> ''",
    )
    .bind(session_id)
    .fetch_all(&state.db)
    .await?;

    // Delete photos from storage. Keys are re-validated here because they were
    // recorded from a client-supplied field; without this a submitter could
    // name an unrelated object and have it destroyed when the session ended.
    for public_id in &photo_ids_to_delete {
        let Ok(public_id) = crate::storage::validate_attendance_photo_key(public_id) else {
            tracing::warn!("Skipping deletion of out-of-prefix photo key");
            continue;
        };
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

    // Every table below has a `session_id` FK with no ON DELETE CASCADE, so
    // any session that ever had a WebAuthn ceremony, a photo-reuse check, or
    // a device seen against it would fail `DELETE FROM sessions` outright
    // with a foreign-key-violation 500 — this hit every session, not just
    // some, since webauthn_challenges.session_id is NOT NULL and gets a row
    // on every registration/authentication attempt.
    sqlx::query("DELETE FROM webauthn_challenges WHERE session_id = $1")
        .bind(session_id)
        .execute(&state.db)
        .await?;
    sqlx::query("DELETE FROM photo_hashes WHERE session_id = $1")
        .bind(session_id)
        .execute(&state.db)
        .await?;
    sqlx::query("DELETE FROM devices WHERE session_id = $1")
        .bind(session_id)
        .execute(&state.db)
        .await?;
    // webauthn_credentials outlive any single session (they're the student's
    // enrolled passkey) — detach the pointer to the last session it was used
    // in rather than deleting the credential.
    sqlx::query("UPDATE webauthn_credentials SET last_session_id = NULL WHERE last_session_id = $1")
        .bind(session_id)
        .execute(&state.db)
        .await?;

    // Delete the session
    sqlx::query("DELETE FROM sessions WHERE id = $1")
        .bind(session_id)
        .execute(&state.db)
        .await?;

    Ok(())
}

/// Request body for deleting multiple sessions at once (requires password
/// re-verification, same as the single-session delete).
#[derive(Debug, Deserialize)]
pub struct BulkDeleteSessionsRequest {
    #[serde(rename = "sessionIds")]
    pub session_ids: Vec<String>,
    pub password: String,
}

/// Deletes multiple sessions owned by the caller in one request. Ids that
/// don't parse or aren't owned by the caller are skipped and reported back
/// in `failedIds` rather than failing the whole batch.
pub async fn bulk_delete_sessions(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Json(payload): Json<BulkDeleteSessionsRequest>,
) -> Result<impl IntoResponse> {
    if payload.session_ids.is_empty() {
        return Err(AppError::BadRequest("No sessions specified".to_string()));
    }

    let admin: Admin = sqlx::query_as("SELECT * FROM admins WHERE id = $1")
        .bind(auth.id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::Unauthorized("Admin not found".to_string()))?;

    let password_valid = admin
        .verify_password(&payload.password)
        .map_err(|e| AppError::Internal(format!("Password verification failed: {}", e)))?;

    if !password_valid {
        return Err(AppError::Unauthorized("Incorrect password".to_string()));
    }

    let mut deleted_count = 0i64;
    let mut failed_ids = Vec::new();

    for raw_id in &payload.session_ids {
        let Ok(session_id) = Uuid::parse_str(raw_id) else {
            failed_ids.push(raw_id.clone());
            continue;
        };

        let owned: Option<Session> = sqlx::query_as(
            "SELECT * FROM sessions WHERE id = $1 AND ($2 = 'super_admin' OR created_by = $3)",
        )
        .bind(session_id)
        .bind(&auth.role)
        .bind(auth.id)
        .fetch_optional(&state.db)
        .await?;

        if owned.is_none() {
            failed_ids.push(raw_id.clone());
            continue;
        }

        delete_session_records(&state, session_id).await?;
        deleted_count += 1;
    }

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "success": true,
            "deletedCount": deleted_count,
            "failedIds": failed_ids,
        })),
    ))
}

pub async fn rotate_token(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse> {
    let session_id = Uuid::parse_str(&id)
        .map_err(|e| AppError::BadRequest(format!("Invalid session ID: {}", e)))?;

    // Ownership check. Without it a mentor could rotate any admin's session
    // and receive the new attendance token in the response body, hijacking
    // that session and breaking it for its owner at the same time. Any
    // super-admin, however, may rotate any session — they're peers, not
    // tenants of each other.
    let session: Session = sqlx::query_as(
        "SELECT * FROM sessions WHERE id = $1 AND ($2 = 'super_admin' OR created_by = $3)",
    )
    .bind(session_id)
    .bind(&auth.role)
    .bind(auth.id)
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

    let session: Session = sqlx::query_as(
        "SELECT * FROM sessions WHERE id = $1 AND ($2 = 'super_admin' OR created_by = $3)",
    )
    .bind(session_id)
    .bind(&auth.role)
    .bind(auth.id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("Session not found".to_string()))?;

    if let Some(excel_batch_id) = session.excel_batch_id {
        return export_mentor_format(&state, &session, excel_batch_id).await;
    }

    // Exam sessions have no location (manual attendance, not geofenced).
    let location: Option<Location> = match session.location_id {
        Some(id) => {
            sqlx::query_as("SELECT * FROM locations WHERE id = $1")
                .bind(id)
                .fetch_optional(&state.db)
                .await?
        }
        None => None,
    };

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

    let excel_data = generate_excel(
        &attendance_data,
        &session,
        location.as_ref(),
        batch.as_ref(),
    )?;

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
    location: Option<&Location>,
    batch: Option<&Batch>,
) -> Result<Vec<u8>> {
    use rust_xlsxwriter::Workbook;

    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();
    write_gps_sheet(worksheet, data, session, location, batch)?;

    let data = workbook
        .save_to_buffer()
        .map_err(|e| AppError::Internal(format!("Excel generation failed: {}", e)))?;

    Ok(data)
}

/// Writes the GPS/self-checkin attendance export columns (Roll Number,
/// Student Name, Status, Verified, Distance, Captured At, WebAuthn, Device
/// Flag) plus the session-info footer into `worksheet`. Split out of
/// `generate_excel` so the bulk multi-session export can reuse it against a
/// worksheet inside a shared multi-sheet workbook.
fn write_gps_sheet(
    worksheet: &mut rust_xlsxwriter::Worksheet,
    data: &[AttendanceExportRow],
    session: &Session,
    location: Option<&Location>,
    batch: Option<&Batch>,
) -> Result<()> {
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
        .write_string(
            row + 3,
            1,
            location
                .map(|l| l.name.as_str())
                .unwrap_or("N/A (exam session — manually marked)"),
        )
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

    Ok(())
}

// =================== Mentor-format export (Excel-upload sessions) ===================

struct MentorExportRow {
    student_name: String,
    registration_number: String,
    email: String,
    track: String,
    class_label: String,
    session_time_raw: String,
    round: String,
    status: String,
    marked_by: String,
    marked_at: String,
}

/// Exports a session whose roster came from the Excel-upload bulk creation
/// flow (`session.excel_batch_id` set) in the mentor attendance format —
/// columns `Student Name, Registration Number, Email, Track, Class, Batch,
/// Round, Status, Marked By, Marked At` (matching
/// `samplefiles/attendance-data-2026-08-15.csv`; the "Batch" column holds the
/// group's time-range, not a batch name). Reached only from
/// `export_session_attendance`, which has already scoped the session lookup
/// to sessions the caller may access — same authorization boundary as the
/// GPS path.
async fn export_mentor_format(
    state: &Arc<crate::AppState>,
    session: &Session,
    excel_batch_id: Uuid,
) -> Result<(StatusCode, [(header::HeaderName, String); 2], Vec<u8>)> {
    let excel_batch: ExcelBatch = sqlx::query_as("SELECT * FROM excel_batches WHERE id = $1")
        .bind(excel_batch_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound("Excel batch not found".to_string()))?;

    let students: Vec<(String, String, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT roll_number, name, email, round FROM excel_batch_students \
         WHERE excel_batch_id = $1 ORDER BY position",
    )
    .bind(excel_batch_id)
    .fetch_all(&state.db)
    .await?;

    let attendances: Vec<Attendance> =
        sqlx::query_as("SELECT * FROM attendances WHERE session_id = $1")
            .bind(session.id)
            .fetch_all(&state.db)
            .await?;
    let by_roll: std::collections::HashMap<String, &Attendance> = attendances
        .iter()
        .map(|a| (a.roll_number.to_uppercase(), a))
        .collect();

    let marker_ids: Vec<Uuid> = attendances
        .iter()
        .filter_map(|a| a.marked_by_admin_id)
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    let marker_emails: std::collections::HashMap<Uuid, String> = if marker_ids.is_empty() {
        std::collections::HashMap::new()
    } else {
        sqlx::query_as::<_, (Uuid, String)>("SELECT id, email FROM admins WHERE id = ANY($1)")
            .bind(&marker_ids)
            .fetch_all(&state.db)
            .await?
            .into_iter()
            .collect()
    };

    let rows: Vec<MentorExportRow> = students
        .into_iter()
        .map(|(roll_number, name, email, round)| {
            let att = by_roll.get(&roll_number.to_uppercase());
            let (status, marked_by, marked_at) = match att {
                Some(a) => {
                    let status = match a.status {
                        AttendanceStatus::Present => "Present",
                        AttendanceStatus::Absent => "Absent",
                    };
                    let marked_by = a
                        .marked_by_admin_id
                        .and_then(|id| marker_emails.get(&id).cloned())
                        .unwrap_or_default();
                    let marked_at = a.captured_at.format("%d/%m/%Y, %I:%M:%S %P").to_string();
                    (status.to_string(), marked_by, marked_at)
                }
                None => (String::new(), String::new(), String::new()),
            };
            MentorExportRow {
                student_name: name,
                registration_number: roll_number,
                email: email.unwrap_or_default(),
                track: excel_batch.track.clone(),
                class_label: excel_batch.class_label.clone(),
                session_time_raw: excel_batch.session_time_raw.clone(),
                round: round.unwrap_or_default(),
                status,
                marked_by,
                marked_at,
            }
        })
        .collect();

    let mut workbook = rust_xlsxwriter::Workbook::new();
    let worksheet = workbook.add_worksheet();
    write_mentor_sheet(worksheet, &rows)?;
    let excel_data = workbook
        .save_to_buffer()
        .map_err(|e| AppError::Internal(format!("Excel generation failed: {}", e)))?;

    let filename = format!(
        "attendance_{}_{}.xlsx",
        session.id,
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

fn write_mentor_sheet(
    worksheet: &mut rust_xlsxwriter::Worksheet,
    rows: &[MentorExportRow],
) -> Result<()> {
    let headers = [
        "Student Name",
        "Registration Number",
        "Email",
        "Track",
        "Class",
        "Batch",
        "Round",
        "Status",
        "Marked By",
        "Marked At",
    ];
    for (col, header) in headers.iter().enumerate() {
        worksheet
            .write_string(0, col as u16, *header)
            .map_err(|e| AppError::Internal(format!("Excel error: {}", e)))?;
    }

    for (i, r) in rows.iter().enumerate() {
        let row = (i + 1) as u32;
        let values = [
            &r.student_name,
            &r.registration_number,
            &r.email,
            &r.track,
            &r.class_label,
            &r.session_time_raw,
            &r.round,
            &r.status,
            &r.marked_by,
            &r.marked_at,
        ];
        for (col, value) in values.iter().enumerate() {
            worksheet
                .write_string(row, col as u16, value.as_str())
                .map_err(|e| AppError::Internal(format!("Excel error: {}", e)))?;
        }
    }

    worksheet.autofit();
    Ok(())
}

// =================== Bulk multi-session export ===================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportBulkRequest {
    pub session_ids: Vec<String>,
}

/// Exports multiple sessions into a single workbook, one worksheet per
/// session, auto-detecting GPS vs mentor format per session exactly as the
/// single-session export does. Sessions not owned by the caller are skipped
/// rather than erroring the whole batch — in practice this never happens
/// since the admin frontend only ever offers the caller's own sessions.
pub async fn export_bulk_attendance(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Json(payload): Json<ExportBulkRequest>,
) -> Result<impl IntoResponse> {
    if payload.session_ids.is_empty() {
        return Err(AppError::BadRequest(
            "At least one session ID is required".to_string(),
        ));
    }

    let session_ids: Vec<Uuid> = payload
        .session_ids
        .iter()
        .filter_map(|raw_id| Uuid::parse_str(raw_id).ok())
        .collect();

    build_sessions_workbook_response(&state, &auth, &session_ids).await
}

/// Shape returned by `build_sessions_workbook_response` — a concrete type
/// (rather than `impl IntoResponse`) so it can be `.await`ed from more than
/// one handler: `export_bulk_attendance` (explicit ids from a checkbox
/// selection) and `export_sessions_filtered` (ids resolved server-side from
/// a filter combination).
type XlsxAttachment = (StatusCode, [(header::HeaderName, String); 2], Vec<u8>);

/// Builds a single workbook containing one worksheet per session (skipping
/// any id the caller can't access — any session for a super-admin, only
/// sessions they created for a mentor), auto-detecting GPS vs mentor format
/// per session exactly as the single-session export does.
pub(crate) async fn build_sessions_workbook_response(
    state: &Arc<crate::AppState>,
    auth: &AuthenticatedAdmin,
    session_ids: &[Uuid],
) -> Result<XlsxAttachment> {
    let mut workbook = rust_xlsxwriter::Workbook::new();
    let mut used_sheet_names: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut included = 0usize;

    for &session_id in session_ids {
        let Some(session): Option<Session> = sqlx::query_as(
            "SELECT * FROM sessions WHERE id = $1 AND ($2 = 'super_admin' OR created_by = $3)",
        )
        .bind(session_id)
        .bind(&auth.role)
        .bind(auth.id)
        .fetch_optional(&state.db)
        .await?
        else {
            continue;
        };

        let sheet_name = unique_sheet_name(
            &sheet_label_for(state, &session).await?,
            &mut used_sheet_names,
        );
        let worksheet = workbook.add_worksheet();
        worksheet
            .set_name(sheet_name)
            .map_err(|e| AppError::Internal(format!("Excel error: {}", e)))?;

        if let Some(excel_batch_id) = session.excel_batch_id {
            let excel_batch: ExcelBatch =
                sqlx::query_as("SELECT * FROM excel_batches WHERE id = $1")
                    .bind(excel_batch_id)
                    .fetch_optional(&state.db)
                    .await?
                    .ok_or_else(|| AppError::NotFound("Excel batch not found".to_string()))?;

            let students: Vec<(String, String, Option<String>, Option<String>)> = sqlx::query_as(
                "SELECT roll_number, name, email, round FROM excel_batch_students \
                 WHERE excel_batch_id = $1 ORDER BY position",
            )
            .bind(excel_batch_id)
            .fetch_all(&state.db)
            .await?;

            let attendances: Vec<Attendance> =
                sqlx::query_as("SELECT * FROM attendances WHERE session_id = $1")
                    .bind(session.id)
                    .fetch_all(&state.db)
                    .await?;
            let by_roll: std::collections::HashMap<String, &Attendance> = attendances
                .iter()
                .map(|a| (a.roll_number.to_uppercase(), a))
                .collect();
            let marker_ids: Vec<Uuid> = attendances
                .iter()
                .filter_map(|a| a.marked_by_admin_id)
                .collect::<std::collections::HashSet<_>>()
                .into_iter()
                .collect();
            let marker_emails: std::collections::HashMap<Uuid, String> = if marker_ids.is_empty() {
                std::collections::HashMap::new()
            } else {
                sqlx::query_as::<_, (Uuid, String)>(
                    "SELECT id, email FROM admins WHERE id = ANY($1)",
                )
                .bind(&marker_ids)
                .fetch_all(&state.db)
                .await?
                .into_iter()
                .collect()
            };

            let rows: Vec<MentorExportRow> = students
                .into_iter()
                .map(|(roll_number, name, email, round)| {
                    let att = by_roll.get(&roll_number.to_uppercase());
                    let (status, marked_by, marked_at) = match att {
                        Some(a) => {
                            let status = match a.status {
                                AttendanceStatus::Present => "Present",
                                AttendanceStatus::Absent => "Absent",
                            };
                            let marked_by = a
                                .marked_by_admin_id
                                .and_then(|id| marker_emails.get(&id).cloned())
                                .unwrap_or_default();
                            let marked_at =
                                a.captured_at.format("%d/%m/%Y, %I:%M:%S %P").to_string();
                            (status.to_string(), marked_by, marked_at)
                        }
                        None => (String::new(), String::new(), String::new()),
                    };
                    MentorExportRow {
                        student_name: name,
                        registration_number: roll_number,
                        email: email.unwrap_or_default(),
                        track: excel_batch.track.clone(),
                        class_label: excel_batch.class_label.clone(),
                        session_time_raw: excel_batch.session_time_raw.clone(),
                        round: round.unwrap_or_default(),
                        status,
                        marked_by,
                        marked_at,
                    }
                })
                .collect();

            write_mentor_sheet(worksheet, &rows)?;
        } else {
            let location: Option<Location> = match session.location_id {
                Some(id) => {
                    sqlx::query_as("SELECT * FROM locations WHERE id = $1")
                        .bind(id)
                        .fetch_optional(&state.db)
                        .await?
                }
                None => None,
            };
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
            let attendances: Vec<Attendance> = sqlx::query_as(
                "SELECT * FROM attendances WHERE session_id = $1 ORDER BY captured_at ASC",
            )
            .bind(session.id)
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
            write_gps_sheet(
                worksheet,
                &attendance_data,
                &session,
                location.as_ref(),
                batch.as_ref(),
            )?;
        }

        included += 1;
    }

    if included == 0 {
        return Err(AppError::NotFound(
            "None of the requested sessions were found".to_string(),
        ));
    }

    let excel_data = workbook
        .save_to_buffer()
        .map_err(|e| AppError::Internal(format!("Excel generation failed: {}", e)))?;

    let filename = format!(
        "Attendance_Export_{}sessions_{}.xlsx",
        included,
        Utc::now().format("%d-%b-%Y_%H%M")
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

/// A human-recognizable label for a session's worksheet tab: its excel-batch
/// class+time when Excel-generated, else its GPS location name.
async fn sheet_label_for(state: &Arc<crate::AppState>, session: &Session) -> Result<String> {
    if let Some(excel_batch_id) = session.excel_batch_id {
        let row: Option<(String, String)> =
            sqlx::query_as("SELECT class_label, session_time_raw FROM excel_batches WHERE id = $1")
                .bind(excel_batch_id)
                .fetch_optional(&state.db)
                .await?;
        if let Some((class_label, session_time_raw)) = row {
            return Ok(format!("{} {}", class_label, session_time_raw));
        }
    }
    if let Some(location_id) = session.location_id {
        let name: Option<String> = sqlx::query_scalar("SELECT name FROM locations WHERE id = $1")
            .bind(location_id)
            .fetch_optional(&state.db)
            .await?;
        if let Some(name) = name {
            return Ok(name);
        }
    }
    Ok(format!("Session {}", &session.id.to_string()[..8]))
}

/// Sanitizes a candidate name into a valid, unique Excel worksheet name:
/// strips characters Excel forbids in sheet names (`: \ / ? * [ ]`),
/// truncates to the 31-character limit, and de-dupes against names already
/// used in this workbook by appending a numeric suffix.
fn unique_sheet_name(label: &str, used: &mut std::collections::HashSet<String>) -> String {
    let cleaned: String = label
        .chars()
        .map(|c| if "\\/?*[]:".contains(c) { '_' } else { c })
        .collect();
    let cleaned = cleaned.trim();
    let base = if cleaned.is_empty() { "Sheet" } else { cleaned };
    let base: String = base.chars().take(31).collect();

    if !used.contains(&base) {
        used.insert(base.clone());
        return base;
    }

    let mut suffix = 2;
    loop {
        let candidate_suffix = format!("~{}", suffix);
        let truncated_len = 31usize.saturating_sub(candidate_suffix.chars().count());
        let candidate: String = base
            .chars()
            .take(truncated_len)
            .chain(candidate_suffix.chars())
            .collect();
        if !used.contains(&candidate) {
            used.insert(candidate.clone());
            return candidate;
        }
        suffix += 1;
    }
}

// =================== Mentor add/remove (session_admins) ===================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSessionMentorsRequest {
    #[serde(default)]
    pub add: Vec<String>,
    #[serde(default)]
    pub remove: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSessionMentorsResponse {
    pub assigned_admin_ids: Vec<String>,
    pub assigned_admin_names: Vec<String>,
}

/// Adds/removes mentors from a session's `session_admins` assignment.
/// Deliberately scoped by a direct role-aware query rather than
/// `find_session_for_admin` — that helper also matches a session via the
/// caller's *own* mentor assignment, which would let a mentor edit their own
/// session's mentor list. Only a super-admin (any super-admin, not just the
/// one who created the session) may. Works identically for Excel-based and
/// manually-created exam sessions, since it only ever touches
/// `session_admins`.
pub async fn update_session_mentors(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateSessionMentorsRequest>,
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

    let mut add_ids = Vec::with_capacity(payload.add.len());
    for raw_id in &payload.add {
        let raw_id = raw_id.trim();
        if raw_id.is_empty() {
            continue;
        }
        let mentor_id = Uuid::parse_str(raw_id)
            .map_err(|e| AppError::BadRequest(format!("Invalid mentor ID: {}", e)))?;
        sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM admins WHERE id = $1 AND role = $2 AND is_active = true",
        )
        .bind(mentor_id)
        .bind(ROLE_ADMIN)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| {
            AppError::BadRequest(
                "Every assigned mentor must be an active admin/mentor account".to_string(),
            )
        })?;
        add_ids.push(mentor_id);
    }

    let mut remove_ids = Vec::with_capacity(payload.remove.len());
    for raw_id in &payload.remove {
        let raw_id = raw_id.trim();
        if raw_id.is_empty() {
            continue;
        }
        remove_ids.push(
            Uuid::parse_str(raw_id)
                .map_err(|e| AppError::BadRequest(format!("Invalid mentor ID: {}", e)))?,
        );
    }

    let mut tx = state.db.begin().await?;
    for mentor_id in &add_ids {
        sqlx::query(
            "INSERT INTO session_admins (session_id, admin_id) VALUES ($1, $2) \
             ON CONFLICT DO NOTHING",
        )
        .bind(session_id)
        .bind(mentor_id)
        .execute(&mut *tx)
        .await?;
    }
    if !remove_ids.is_empty() {
        sqlx::query("DELETE FROM session_admins WHERE session_id = $1 AND admin_id = ANY($2)")
            .bind(session_id)
            .bind(&remove_ids)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;

    let (assigned_admin_ids, assigned_admin_names) =
        fetch_assigned_mentors(&state.db, session_id).await?;

    Ok(Json(UpdateSessionMentorsResponse {
        assigned_admin_ids,
        assigned_admin_names,
    }))
}
