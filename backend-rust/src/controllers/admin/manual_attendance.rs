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
        mentor_edit_window_closed, resolve_roster_source, RosterStudentRow,
    },
    error::{AppError, Result},
    middleware::AuthenticatedAdmin,
    models::{
        record_audit_event, Attendance, AttendanceSource, AttendanceStatus, Location, Session,
    },
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RosterSessionInfo {
    #[serde(rename = "_id")]
    pub id: String,
    pub college_name: Option<String>,
    pub starts_at: Option<DateTime<Utc>>,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub batch_name: Option<String>,
    pub description: Option<String>,
    /// Super-admin configurable (Settings page) — how many minutes before
    /// `starts_at` the mobile app should let a mentor start marking. Sent
    /// per-response instead of baked into the client so it stays correct
    /// without an app release when a super-admin changes the setting.
    pub manual_mark_early_window_minutes: i64,
    /// Normal (non-exam) sessions only — how many hours after `created_at` a
    /// mentor may still mark/undo attendance or be added/removed. Sent
    /// per-response (super-admin configurable, Settings page) so the mentor
    /// apps can show an accurate countdown/lock state without a release.
    pub mentor_edit_window_hours: i64,
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

/// `pub(crate)`: also reused by `batch_analytics` for the per-student
/// session-detail table, so the present/absent/unmarked mapping never drifts
/// between the two features.
pub(crate) fn source_label(source: AttendanceSource) -> &'static str {
    match source {
        AttendanceSource::SelfSubmitted => "self_submitted",
        AttendanceSource::Manual => "manual",
    }
}

pub(crate) fn status_label(status: AttendanceStatus) -> &'static str {
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
    let sys_config = state.get_system_config().await;

    let source = resolve_roster_source(&session);
    let batch_name = fetch_roster_name(&state.db, &source).await?;

    let session_info = RosterSessionInfo {
        id: session.id.to_string(),
        college_name: session.college_name.clone(),
        starts_at: session.starts_at,
        expires_at: session.expires_at,
        created_at: session.created_at,
        batch_name,
        description: session.description.clone(),
        manual_mark_early_window_minutes: sys_config
            .session_config
            .manual_mark_early_window_minutes,
        mentor_edit_window_hours: sys_config.session_config.mentor_edit_window_hours,
    };

    let students = fetch_roster_students(&state.db, &source).await?;

    let attendances: Vec<Attendance> =
        sqlx::query_as("SELECT * FROM attendances WHERE session_id = $1 ORDER BY captured_at")
            .bind(session_id)
            .fetch_all(&state.db)
            .await?;

    // A session with no batch (or students who checked in without being on
    // its roster) still has attendance a mentor must be able to see, so the
    // list is roster students merged with any off-roster attendees rather
    // than being empty whenever the roster is.
    let (roster, summary) = build_roster_view(students, &attendances);

    Ok(Json(RosterResponse {
        session: session_info,
        students: roster,
        summary,
    }))
}

/// Merges the batch roster with a session's attendance rows into the single
/// combined present/absent/unmarked list the mentor apps render. Roster
/// students come first (in roster order), followed by any attendee whose roll
/// number is not on the roster (e.g. every attendee of a no-batch session).
/// Off-roster rows are read-only for the mentor (`source` is never "manual"
/// unless the mentor created it, and `mark_attendance_manual` only accepts
/// roster students).
fn build_roster_view(
    students: Vec<RosterStudentRow>,
    attendances: &[Attendance],
) -> (Vec<RosterStudent>, RosterSummary) {
    let by_roll: HashMap<String, &Attendance> = attendances
        .iter()
        .map(|a| (a.roll_number.to_uppercase(), a))
        .collect();
    let roster_rolls: std::collections::HashSet<String> = students
        .iter()
        .map(|s| s.roll_number.to_uppercase())
        .collect();

    let mut present = 0i64;
    let mut absent = 0i64;
    let mut count = |a: &Attendance| match a.status {
        AttendanceStatus::Present => present += 1,
        AttendanceStatus::Absent => absent += 1,
    };

    let mut roster: Vec<RosterStudent> = Vec::with_capacity(students.len());
    for s in students {
        let (status, source, marked_at) = match by_roll.get(&s.roll_number.to_uppercase()) {
            Some(a) => {
                count(a);
                (
                    status_label(a.status),
                    Some(source_label(a.source)),
                    Some(a.captured_at),
                )
            }
            None => ("unmarked", None, None),
        };
        roster.push(RosterStudent {
            student_id: s.id.to_string(),
            roll_number: s.roll_number.to_uppercase(),
            name: s.name,
            college_name: s.college_name,
            email: s.email,
            status,
            source,
            marked_at,
        });
    }

    let mut seen_extra = std::collections::HashSet::new();
    for a in attendances {
        let roll_upper = a.roll_number.to_uppercase();
        if roster_rolls.contains(&roll_upper) || !seen_extra.insert(roll_upper) {
            continue;
        }
        count(a);
        roster.push(RosterStudent {
            student_id: a.id.to_string(),
            roll_number: a.roll_number.clone(),
            name: a.student_name.clone(),
            college_name: None,
            email: None,
            status: status_label(a.status),
            source: Some(source_label(a.source)),
            marked_at: Some(a.captured_at),
        });
    }

    let total = roster.len() as i64;
    let marked = present + absent;
    (
        roster,
        RosterSummary {
            total,
            marked,
            present,
            absent,
            unmarked: total - marked,
        },
    )
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

/// The single gate for every manual attendance edit (mark *and* undo), so the
/// two can never disagree about whether a session is still editable. Applies
/// to mentors and super-admins alike.
///
/// * A deactivated session (explicitly ended by its creator / a super-admin)
///   accepts no further manual edits.
/// * Exam session (has `starts_at`): mirrors the mentor frontend's "starts
///   soon" gate — a mentor may start marking
///   `manual_mark_early_window_minutes` (Settings page) before the scheduled
///   start, but never after the scheduled end (`expires_at`).
/// * Normal session: its own short self-check-in window (`is_expired()`) is
///   deliberately *not* checked — a mentor must still be able to fix a
///   student's attendance after it closes, up to the later, configurable
///   `mentor_edit_window_hours` cutoff counted from when it was created.
fn ensure_manual_edit_open(
    session: &Session,
    early_window_minutes: i64,
    edit_window_hours: i64,
) -> Result<()> {
    if !session.is_active {
        return Err(AppError::BadRequest(
            "This session has been deactivated".to_string(),
        ));
    }
    if let Some(starts_at) = session.starts_at {
        let marking_opens_at = starts_at - chrono::Duration::minutes(early_window_minutes);
        if marking_opens_at > Utc::now() {
            return Err(AppError::BadRequest(
                "This session hasn't started yet".to_string(),
            ));
        }
        if session.is_expired() {
            return Err(AppError::BadRequest(
                "This session's attendance window has closed".to_string(),
            ));
        }
    } else if mentor_edit_window_closed(session, edit_window_hours) {
        return Err(AppError::BadRequest(format!(
            "The {edit_window_hours}-hour window to update this session's attendance has closed"
        )));
    }
    Ok(())
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
    let sys_config = state.get_system_config().await;

    ensure_manual_edit_open(
        &session,
        sys_config.session_config.manual_mark_early_window_minutes,
        sys_config.session_config.mentor_edit_window_hours,
    )?;

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

    let attendance: Attendance = match existing {
        Some(att) if att.source == AttendanceSource::SelfSubmitted => {
            return Err(AppError::Conflict(
                "This student already self-submitted attendance for this session".to_string(),
            ));
        }
        Some(att) => {
            sqlx::query_as(
                // `verified` tracks the new status, same as the INSERT below:
                // a mentor-confirmed present is "verified", and flipping back
                // to absent must clear it. Leaving it at its first-mark value
                // made an absent -> present re-mark show as "Unverified" in
                // the admin session table, stats and verified export.
                "UPDATE attendances SET status = $1, verified = $2, marked_by_admin_id = $3, \
                 captured_at = now() WHERE id = $4 RETURNING *",
            )
            .bind(status)
            .bind(status == AttendanceStatus::Present)
            .bind(auth.id)
            .bind(att.id)
            .fetch_one(&mut *tx)
            .await?
        }
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

    let session = find_session_for_admin(&state.db, session_id, &auth).await?;

    // An undo is just another edit: same gate as marking.
    let sys_config = state.get_system_config().await;
    ensure_manual_edit_open(
        &session,
        sys_config.session_config.manual_mark_early_window_minutes,
        sys_config.session_config.mentor_edit_window_hours,
    )?;

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
