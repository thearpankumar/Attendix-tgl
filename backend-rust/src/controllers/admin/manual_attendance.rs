// Mentor-facing roster + manual present/absent marking for exam sessions.
//
// Attendance has historically been exclusively self-submitted by students
// (photo + geofence + anti-fraud checks, see controllers/attendance.rs). This
// module adds the admin-initiated counterpart: a mentor viewing their
// assigned session's roster and marking students present/absent in person.
// Manual rows are tagged `source = 'manual'` and never overwrite a
// self-submitted row's forensic data (see mark_attendance_manual's conflict
// check).

use axum::{
    extract::{Json, Path, State},
    response::IntoResponse,
    Extension,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use crate::{
    controllers::{
        fetch_roster_name, fetch_roster_students, find_roster_student, find_session_for_admin,
        resolve_roster_source,
    },
    error::{AppError, Result},
    middleware::AuthenticatedAdmin,
    models::{record_audit_event, Attendance, AttendanceSource, AttendanceStatus, Location},
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RosterSessionInfo {
    #[serde(rename = "_id")]
    pub id: String,
    pub college_name: Option<String>,
    pub starts_at: Option<DateTime<Utc>>,
    pub expires_at: DateTime<Utc>,
    pub batch_name: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RosterStudent {
    pub student_id: String,
    pub roll_number: String,
    pub name: String,
    pub college_name: Option<String>,
    pub email: Option<String>,
    /// "unmarked" | "present" | "absent"
    pub status: &'static str,
    /// null | "self_submitted" | "manual"
    pub source: Option<&'static str>,
    pub marked_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RosterSummary {
    pub total: i64,
    pub marked: i64,
    pub present: i64,
    pub absent: i64,
    pub unmarked: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RosterResponse {
    pub session: RosterSessionInfo,
    pub students: Vec<RosterStudent>,
    pub summary: RosterSummary,
}

fn source_label(source: AttendanceSource) -> &'static str {
    match source {
        AttendanceSource::SelfSubmitted => "self_submitted",
        AttendanceSource::Manual => "manual",
    }
}

fn status_label(status: AttendanceStatus) -> &'static str {
    match status {
        AttendanceStatus::Present => "present",
        AttendanceStatus::Absent => "absent",
    }
}

/// The whole roster + progress screen in a single call: joins the session's
/// batch roster with whatever attendance rows already exist for it.
pub async fn get_session_roster(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse> {
    let session_id = uuid::Uuid::parse_str(&id)
        .map_err(|e| AppError::BadRequest(format!("Invalid session ID: {}", e)))?;

    let session = find_session_for_admin(&state.db, session_id, &auth).await?;

    let source = resolve_roster_source(&session);
    let batch_name = fetch_roster_name(&state.db, &source).await?;

    let session_info = RosterSessionInfo {
        id: session.id.to_string(),
        college_name: session.college_name.clone(),
        starts_at: session.starts_at,
        expires_at: session.expires_at,
        batch_name,
        description: session.description.clone(),
    };

    // Legacy sessions created before batches were mandatory (or a normal
    // session with no batch attached) have no roster to show — mirrors
    // get_session_absent's existing `None => vec![]` fallback.
    let students = fetch_roster_students(&state.db, &source).await?;
    if students.is_empty() {
        return Ok(Json(RosterResponse {
            session: session_info,
            students: vec![],
            summary: RosterSummary {
                total: 0,
                marked: 0,
                present: 0,
                absent: 0,
                unmarked: 0,
            },
        }));
    }

    let attendances: Vec<Attendance> =
        sqlx::query_as("SELECT * FROM attendances WHERE session_id = $1")
            .bind(session_id)
            .fetch_all(&state.db)
            .await?;

    let by_roll: HashMap<String, &Attendance> = attendances
        .iter()
        .map(|a| (a.roll_number.to_uppercase(), a))
        .collect();

    let mut present = 0i64;
    let mut absent = 0i64;

    let roster: Vec<RosterStudent> = students
        .into_iter()
        .map(|s| {
            let att = by_roll.get(&s.roll_number.to_uppercase());
            let (status, source, marked_at) = match att {
                Some(a) => {
                    match a.status {
                        AttendanceStatus::Present => present += 1,
                        AttendanceStatus::Absent => absent += 1,
                    }
                    (
                        status_label(a.status),
                        Some(source_label(a.source)),
                        Some(a.captured_at),
                    )
                }
                None => ("unmarked", None, None),
            };
            RosterStudent {
                student_id: s.id.to_string(),
                roll_number: s.roll_number,
                name: s.name,
                college_name: s.college_name,
                email: s.email,
                status,
                source,
                marked_at,
            }
        })
        .collect();

    let total = roster.len() as i64;
    let marked = present + absent;

    Ok(Json(RosterResponse {
        session: session_info,
        students: roster,
        summary: RosterSummary {
            total,
            marked,
            present,
            absent,
            unmarked: total - marked,
        },
    }))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualMarkRequest {
    pub roll_number: String,
    /// "present" | "absent"
    pub status: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualMarkResponse {
    pub success: bool,
    pub attendance_id: String,
    pub status: String,
    pub roll_number: String,
}

/// Marks (or re-marks) a single roster student present/absent. Idempotent: a
/// second call for the same student updates the existing manual row rather
/// than creating a duplicate — this is how a mentor corrects a mis-swipe.
/// Never touches a self-submitted row (409 instead — that student's forensic
/// data belongs to the review/verify tooling, not this endpoint).
pub async fn mark_attendance_manual(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Path(id): Path<String>,
    Json(payload): Json<ManualMarkRequest>,
) -> Result<impl IntoResponse> {
    let session_id = uuid::Uuid::parse_str(&id)
        .map_err(|e| AppError::BadRequest(format!("Invalid session ID: {}", e)))?;

    let status = match payload.status.as_str() {
        "present" => AttendanceStatus::Present,
        "absent" => AttendanceStatus::Absent,
        _ => {
            return Err(AppError::BadRequest(
                "status must be 'present' or 'absent'".to_string(),
            ))
        }
    };

    let session = find_session_for_admin(&state.db, session_id, &auth).await?;

    // Mirrors the mentor frontend's own "starts soon" gate — enforced here
    // too since that's only a UI convenience, not something a direct API
    // call is bound by. A session with no `starts_at` (normal self-check-in)
    // has always been markable immediately.
    if let Some(starts_at) = session.starts_at {
        if starts_at > Utc::now() {
            return Err(AppError::BadRequest(
                "This session hasn't started yet".to_string(),
            ));
        }
    }

    let source = resolve_roster_source(&session);
    if matches!(source, crate::controllers::RosterSource::None) {
        return Err(AppError::BadRequest(
            "This session has no batch attached".to_string(),
        ));
    }

    let roll_upper = payload.roll_number.trim().to_uppercase();
    if roll_upper.is_empty() {
        return Err(AppError::BadRequest("rollNumber is required".to_string()));
    }

    let student = find_roster_student(&state.db, &source, &roll_upper)
        .await?
        .ok_or_else(|| {
            AppError::NotFound("Student not found in this session's roster".to_string())
        })?;

    let mut tx = state.db.begin().await?;

    let existing: Option<Attendance> = sqlx::query_as(
        "SELECT * FROM attendances WHERE session_id = $1 AND roll_number = $2 FOR UPDATE",
    )
    .bind(session_id)
    .bind(&roll_upper)
    .fetch_optional(&mut *tx)
    .await?;

    let attendance: Attendance =
        match existing {
            Some(att) if att.source == AttendanceSource::SelfSubmitted => {
                return Err(AppError::Conflict(
                    "This student already self-submitted attendance for this session".to_string(),
                ));
            }
            Some(att) => sqlx::query_as(
                "UPDATE attendances SET status = $1, marked_by_admin_id = $2, captured_at = now() \
                 WHERE id = $3 RETURNING *",
            )
            .bind(status)
            .bind(auth.id)
            .bind(att.id)
            .fetch_one(&mut *tx)
            .await?,
            None => {
                let location: Option<Location> =
                    sqlx::query_as("SELECT * FROM locations WHERE id = $1")
                        .bind(session.location_id)
                        .fetch_optional(&mut *tx)
                        .await?;
                let (lat, lng) = location
                    .map(|l| (l.latitude, l.longitude))
                    .unwrap_or((0.0, 0.0));

                sqlx::query_as(
                    "INSERT INTO attendances ( \
                    id, session_id, student_name, roll_number, photo_url, photo_public_id, \
                    student_latitude, student_longitude, distance_from_location, verified, \
                    source, status, marked_by_admin_id, face_detected, captured_at \
                 ) VALUES ( \
                    $1, $2, $3, $4, '', '', $5, $6, 0.0, $7, $8, $9, $10, false, now() \
                 ) RETURNING *",
                )
                .bind(uuid::Uuid::new_v4())
                .bind(session_id)
                .bind(&student.name)
                .bind(&roll_upper)
                .bind(lat)
                .bind(lng)
                .bind(status == AttendanceStatus::Present)
                .bind(AttendanceSource::Manual)
                .bind(status)
                .bind(auth.id)
                .fetch_one(&mut *tx)
                .await?
            }
        };

    tx.commit().await?;

    if let Err(e) = record_audit_event(
        &state.db,
        Some(auth.id),
        "manual_attendance_marked",
        serde_json::json!({ "session_id": session_id, "roll_number": roll_upper, "status": payload.status }),
        None,
    )
    .await
    {
        tracing::error!(error = %e, "Failed to record audit log entry for manual_attendance_marked");
    }

    Ok(Json(ManualMarkResponse {
        success: true,
        attendance_id: attendance.id.to_string(),
        status: status_label(attendance.status).to_string(),
        roll_number: attendance.roll_number,
    }))
}

/// Reverts a manual mark all the way back to "unmarked" (deletes the row).
/// Never touches a self-submitted row.
pub async fn undo_manual_mark(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Path((id, roll_number)): Path<(String, String)>,
) -> Result<impl IntoResponse> {
    let session_id = uuid::Uuid::parse_str(&id)
        .map_err(|e| AppError::BadRequest(format!("Invalid session ID: {}", e)))?;

    let _session = find_session_for_admin(&state.db, session_id, &auth).await?;

    let roll_upper = roll_number.trim().to_uppercase();

    let result = sqlx::query(
        "DELETE FROM attendances WHERE session_id = $1 AND roll_number = $2 AND source = $3",
    )
    .bind(session_id)
    .bind(&roll_upper)
    .bind(AttendanceSource::Manual)
    .execute(&state.db)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(
            "No manual attendance mark found for this student".to_string(),
        ));
    }

    if let Err(e) = record_audit_event(
        &state.db,
        Some(auth.id),
        "manual_attendance_undone",
        serde_json::json!({ "session_id": session_id, "roll_number": roll_upper }),
        None,
    )
    .await
    {
        tracing::error!(error = %e, "Failed to record audit log entry for manual_attendance_undone");
    }

    Ok(Json(serde_json::json!({ "success": true })))
}
