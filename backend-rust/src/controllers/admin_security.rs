use axum::{
    extract::{Json, Path, State},
    response::IntoResponse,
    Extension,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    error::{AppError, Result},
    middleware::AuthenticatedAdmin,
    models::{
        Admin, Attendance, EmulatorFlag, GpsAnomaly, GpsConfidence, IntegrityCheck, SystemConfig,
    },
};

#[derive(Debug, Serialize)]
pub struct UnreviewedFlagsSummary {
    #[serde(rename = "gpsAnomalies")]
    pub gps_anomalies: i64,
    #[serde(rename = "emulatorDetected")]
    pub emulator_detected: i64,
    #[serde(rename = "integrityIssues")]
    pub integrity_issues: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecuritySummary {
    pub total_submissions: i64,
    pub flagged_submissions: i64,
    pub unreviewed_flags: UnreviewedFlagsSummary,
    pub flag_percentage: String,
}

/// Confirms the calling admin may access the session: any super-admin may
/// access any session; a mentor only one they created (which in practice is
/// never, since mentors don't create sessions — these security-review
/// endpoints are effectively super-admin only).
///
/// These endpoints previously took `Extension(_auth)` and looked rows up by
/// path parameter alone, so any admin could read any other admin's flagged
/// submissions and student PII, or approve their attendance records.
async fn assert_session_owned(
    pool: &sqlx::PgPool,
    session_id: Uuid,
    admin: &AuthenticatedAdmin,
) -> Result<()> {
    let owned: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM sessions WHERE id = $1 AND ($2 = 'super_admin' OR created_by = $3))",
    )
    .bind(session_id)
    .bind(&admin.role)
    .bind(admin.id)
    .fetch_one(pool)
    .await?;

    if owned {
        Ok(())
    } else {
        Err(AppError::NotFound("Session not found".to_string()))
    }
}

/// Confirms the calling admin may access the attendance row's session (see
/// `assert_session_owned`).
async fn assert_attendance_owned(
    pool: &sqlx::PgPool,
    attendance_id: Uuid,
    admin: &AuthenticatedAdmin,
) -> Result<()> {
    let owned: bool = sqlx::query_scalar(
        "SELECT EXISTS( \
             SELECT 1 FROM attendances a \
             JOIN sessions s ON s.id = a.session_id \
             WHERE a.id = $1 AND ($2 = 'super_admin' OR s.created_by = $3) \
         )",
    )
    .bind(attendance_id)
    .bind(&admin.role)
    .bind(admin.id)
    .fetch_one(pool)
    .await?;

    if owned {
        Ok(())
    } else {
        Err(AppError::NotFound(
            "Attendance record not found".to_string(),
        ))
    }
}

pub async fn get_security_summary(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Path(session_id): Path<String>,
) -> Result<impl IntoResponse> {
    let session_id = Uuid::parse_str(&session_id)
        .map_err(|e| AppError::BadRequest(format!("Invalid session ID: {}", e)))?;

    assert_session_owned(&state.db, session_id, &auth).await?;

    let total_submissions: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM attendances WHERE session_id = $1")
            .bind(session_id)
            .fetch_one(&state.db)
            .await?;

    let flagged_submissions: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM attendances WHERE session_id = $1 AND flagged = true",
    )
    .bind(session_id)
    .fetch_one(&state.db)
    .await?;

    let unreviewed_gps: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM attendances \
         WHERE session_id = $1 AND flagged = true AND flag_reviewed = false \
           AND (jsonb_array_length(gps_anomalies) > 0 OR gps_mock_location = true)",
    )
    .bind(session_id)
    .fetch_one(&state.db)
    .await?;

    let unreviewed_emulator: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM attendances \
         WHERE session_id = $1 AND flagged = true AND flag_reviewed = false AND emulator_detected = true",
    )
    .bind(session_id)
    .fetch_one(&state.db)
    .await?;

    let unreviewed_integrity: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM attendances \
         WHERE session_id = $1 AND flagged = true AND flag_reviewed = false \
           AND jsonb_array_length(integrity_checks) > 0",
    )
    .bind(session_id)
    .fetch_one(&state.db)
    .await?;

    let flag_percentage = if total_submissions > 0 {
        format!(
            "{:.1}%",
            (flagged_submissions as f64 / total_submissions as f64) * 100.0
        )
    } else {
        "0.0%".to_string()
    };

    Ok(Json(SecuritySummary {
        total_submissions,
        flagged_submissions,
        unreviewed_flags: UnreviewedFlagsSummary {
            gps_anomalies: unreviewed_gps,
            emulator_detected: unreviewed_emulator,
            integrity_issues: unreviewed_integrity,
        },
        flag_percentage,
    }))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewSubmissionRequest {
    pub action: String, // "approve" or "reject"
    pub notes: Option<String>,
}

pub async fn review_submission(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Path(attendance_id): Path<String>,
    Json(payload): Json<ReviewSubmissionRequest>,
) -> Result<impl IntoResponse> {
    let attendance_uuid = Uuid::parse_str(&attendance_id)
        .map_err(|e| AppError::BadRequest(format!("Invalid attendance ID: {}", e)))?;

    assert_attendance_owned(&state.db, attendance_uuid, &auth).await?;

    let notes = payload.notes.as_deref().unwrap_or("").trim();
    if notes.is_empty() {
        return Err(AppError::BadRequest(
            "Review notes are required — briefly note why this submission was approved or rejected."
                .to_string(),
        ));
    }

    let message = match payload.action.as_str() {
        "approve" => {
            // `review_notes` is a dedicated column — it used to be written
            // into `flag_details`, which already holds the anomaly summary
            // (what the system detected), overwriting it with what the
            // reviewer wrote (why they decided) instead of keeping both.
            sqlx::query(
                "UPDATE attendances SET flagged = false, flag_reviewed = true, flag_reviewed_by = $1, \
                 flag_reviewed_at = now(), review_notes = $2, verified = true WHERE id = $3",
            )
            .bind(auth.id)
            .bind(notes)
            .bind(attendance_uuid)
            .execute(&state.db)
            .await?;

            // TODO: Update device trust score (when device_trust is available)

            "Attendance submission approved and verified successfully"
        }
        "reject" => {
            sqlx::query(
                "UPDATE attendances SET flag_reviewed = true, flag_reviewed_by = $1, \
                 flag_reviewed_at = now(), review_notes = $2, verified = false WHERE id = $3",
            )
            .bind(auth.id)
            .bind(notes)
            .bind(attendance_uuid)
            .execute(&state.db)
            .await?;

            "Attendance submission rejected and marked as unverified"
        }
        _ => {
            return Err(AppError::BadRequest(
                "Invalid action. Use 'approve' or 'reject'".to_string(),
            ));
        }
    };

    Ok(Json(serde_json::json!({
        "success": true,
        "message": message,
        "attendance_id": attendance_id,
        "action": payload.action,
        "reviewed_by": auth.id.to_string(),
    })))
}

// =================== Flagged Submissions ===================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlaggedSubmissionResponse {
    #[serde(rename = "_id")]
    pub id: String,
    /// Needed on the global (cross-session) queue so an admin can tell which
    /// session a flagged row belongs to — the per-session endpoint doesn't
    /// need it (the caller already knows), but it's harmless there too.
    pub session_id: String,
    pub roll_number: String,
    pub student_name: String,
    pub captured_at: chrono::DateTime<chrono::Utc>,
    pub flagged: bool,
    pub flag_reason: Option<String>,
    pub flag_reviewed: bool,
    /// Rollup of the max severity across this row's anomalies — lets the
    /// unified review queue sort/filter without unpacking the JSONB arrays
    /// below. `None` for a row that was never flagged.
    pub flag_severity: Option<crate::models::Severity>,
    /// What the reviewing admin wrote when approving/rejecting this flag —
    /// `None` until reviewed.
    pub review_notes: Option<String>,
    pub gps_confidence: Option<GpsConfidence>,
    pub gps_anomalies: Vec<GpsAnomaly>,
    pub emulator_detected: bool,
    pub emulator_flags: Vec<EmulatorFlag>,
    pub integrity_checks: Vec<IntegrityCheck>,
}

impl FlaggedSubmissionResponse {
    fn from_attendance(a: Attendance) -> Self {
        Self {
            id: a.id.to_string(),
            session_id: a.session_id.to_string(),
            roll_number: a.roll_number,
            student_name: a.student_name,
            captured_at: a.captured_at,
            flagged: a.flagged,
            flag_reason: a.flag_reason,
            flag_reviewed: a.flag_reviewed,
            flag_severity: a.flag_severity,
            review_notes: a.review_notes,
            gps_confidence: a.gps_confidence,
            gps_anomalies: a.gps_anomalies.0,
            emulator_detected: a.emulator_detected,
            emulator_flags: a.emulator_flags.0,
            integrity_checks: a.integrity_checks.0,
        }
    }
}

pub async fn get_flagged_submissions(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Path(session_id): Path<String>,
) -> Result<impl IntoResponse> {
    let session_id = Uuid::parse_str(&session_id)
        .map_err(|e| AppError::BadRequest(format!("Invalid session ID: {}", e)))?;

    assert_session_owned(&state.db, session_id, &auth).await?;

    let attendances: Vec<Attendance> = sqlx::query_as(
        "SELECT * FROM attendances WHERE session_id = $1 AND flagged = true LIMIT 100",
    )
    .bind(session_id)
    .fetch_all(&state.db)
    .await?;

    let submissions: Vec<FlaggedSubmissionResponse> = attendances
        .into_iter()
        .map(FlaggedSubmissionResponse::from_attendance)
        .collect();

    Ok(Json(serde_json::json!({ "submissions": submissions })))
}

// =================== Unified Flag Queue ===================
//
// Replaces the old `controllers::admin::flags` module ("System A"): that
// endpoint required no `session_id` (so it was at least global), but only
// ever matched the legacy `device_flag` column, missed any row flagged
// purely via `gps_anomalies`/`emulator_flags`/`integrity_checks`, had no
// pagination, and its `review_notes` request field was silently discarded.
// This endpoint is System B's richer model (severity, ownership-scoped,
// notes actually persisted — see `review_submission` above) made global by
// making `session_id` optional, with real pagination and severity/review
// filters added.

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlagQueueQuery {
    pub session_id: Option<String>,
    pub reviewed: Option<bool>,
    /// "high" | "medium" | "low", matching `Severity`'s serde representation.
    pub severity: Option<String>,
    #[serde(default = "default_page")]
    pub page: i64,
    #[serde(default = "default_page_size")]
    pub page_size: i64,
}

fn default_page() -> i64 {
    1
}

fn default_page_size() -> i64 {
    25
}

const MAX_PAGE_SIZE: i64 = 100;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlagQueuePage {
    pub items: Vec<FlaggedSubmissionResponse>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}

pub async fn get_flag_queue(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    axum::extract::Query(query): axum::extract::Query<FlagQueueQuery>,
) -> Result<impl IntoResponse> {
    let session_id = match &query.session_id {
        Some(id) => {
            let session_id = Uuid::parse_str(id)
                .map_err(|e| AppError::BadRequest(format!("Invalid session ID: {}", e)))?;
            assert_session_owned(&state.db, session_id, &auth).await?;
            Some(session_id)
        }
        // No session filter means a cross-tenant view — same rationale as
        // the old System A route (see controllers::admin::flags, now
        // removed): only a super-admin may see flags across every session,
        // a mentor only ever reaches this route with a session_id, scoped by
        // assert_session_owned above.
        None => {
            auth.require_role(crate::constants::ROLE_SUPER_ADMIN)?;
            None
        }
    };

    let severity = match query.severity.as_deref() {
        Some("high") => Some(crate::models::Severity::High),
        Some("medium") => Some(crate::models::Severity::Medium),
        Some("low") => Some(crate::models::Severity::Low),
        Some(other) => {
            return Err(AppError::BadRequest(format!(
                "Invalid severity '{other}'. Use 'high', 'medium', or 'low'."
            )));
        }
        None => None,
    };

    let page = query.page.max(1);
    let page_size = query.page_size.clamp(1, MAX_PAGE_SIZE);
    let offset = (page - 1) * page_size;

    let total: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM attendances \
         WHERE (flagged = true OR device_flag IS NOT NULL) \
           AND ($1::uuid IS NULL OR session_id = $1) \
           AND ($2::bool IS NULL OR flag_reviewed = $2) \
           AND ($3::text IS NULL OR flag_severity = $3)",
    )
    .bind(session_id)
    .bind(query.reviewed)
    .bind(severity)
    .fetch_one(&state.db)
    .await?;

    let attendances: Vec<Attendance> = sqlx::query_as(
        "SELECT * FROM attendances \
         WHERE (flagged = true OR device_flag IS NOT NULL) \
           AND ($1::uuid IS NULL OR session_id = $1) \
           AND ($2::bool IS NULL OR flag_reviewed = $2) \
           AND ($3::text IS NULL OR flag_severity = $3) \
         ORDER BY \
           flag_reviewed ASC, \
           CASE flag_severity WHEN 'high' THEN 0 WHEN 'medium' THEN 1 WHEN 'low' THEN 2 ELSE 3 END, \
           captured_at DESC \
         LIMIT $4 OFFSET $5",
    )
    .bind(session_id)
    .bind(query.reviewed)
    .bind(severity)
    .bind(page_size)
    .bind(offset)
    .fetch_all(&state.db)
    .await?;

    let items: Vec<FlaggedSubmissionResponse> = attendances
        .into_iter()
        .map(FlaggedSubmissionResponse::from_attendance)
        .collect();

    Ok(Json(FlagQueuePage {
        items,
        total,
        page,
        page_size,
    }))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkReviewFlagsRequest {
    pub ids: Vec<String>,
    pub action: String, // "approve" or "reject"
    pub notes: Option<String>,
}

/// Bulk approve/reject for the unified queue — mirrors
/// `admin::flags::bulk_verify_attendance`'s shape (cap at 100 ids, single
/// UPDATE) but for the review fields rather than `verified`.
pub async fn bulk_review_flags(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Json(payload): Json<BulkReviewFlagsRequest>,
) -> Result<impl IntoResponse> {
    if payload.ids.is_empty() {
        return Err(AppError::BadRequest(
            "ids must be a non-empty array".to_string(),
        ));
    }
    if payload.ids.len() > 100 {
        return Err(AppError::BadRequest(
            "Cannot bulk-review more than 100 records at once".to_string(),
        ));
    }
    let notes = payload.notes.as_deref().unwrap_or("").trim();
    if notes.is_empty() {
        return Err(AppError::BadRequest(
            "Review notes are required for a bulk review.".to_string(),
        ));
    }

    let ids: Result<Vec<Uuid>> = payload
        .ids
        .iter()
        .map(|id| {
            Uuid::parse_str(id)
                .map_err(|e| AppError::BadRequest(format!("Invalid attendance ID: {}", e)))
        })
        .collect();
    let ids = ids?;

    // Ownership check per row (not just per session) since a bulk request
    // can span multiple sessions in one call.
    let owned_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM attendances a JOIN sessions s ON s.id = a.session_id \
         WHERE a.id = ANY($1) AND ($2 = 'super_admin' OR s.created_by = $3)",
    )
    .bind(&ids)
    .bind(&auth.role)
    .bind(auth.id)
    .fetch_one(&state.db)
    .await?;
    if owned_count != ids.len() as i64 {
        return Err(AppError::Forbidden(
            "One or more attendance records are not accessible to this admin".to_string(),
        ));
    }

    // Same per-row semantics as the single-item `review_submission` above:
    // approving clears `flagged` (the row is settled); rejecting leaves
    // `flagged` set (still surfaced as a known-bad row) but marks it
    // reviewed so it drops out of the *unreviewed* queue.
    let result = match payload.action.as_str() {
        "approve" => {
            sqlx::query(
                "UPDATE attendances SET flagged = false, flag_reviewed = true, flag_reviewed_by = $1, \
                 flag_reviewed_at = now(), review_notes = $2, verified = true WHERE id = ANY($3)",
            )
            .bind(auth.id)
            .bind(notes)
            .bind(&ids)
            .execute(&state.db)
            .await?
        }
        "reject" => {
            sqlx::query(
                "UPDATE attendances SET flag_reviewed = true, flag_reviewed_by = $1, \
                 flag_reviewed_at = now(), review_notes = $2, verified = false WHERE id = ANY($3)",
            )
            .bind(auth.id)
            .bind(notes)
            .bind(&ids)
            .execute(&state.db)
            .await?
        }
        _ => {
            return Err(AppError::BadRequest(
                "Invalid action. Use 'approve' or 'reject'".to_string(),
            ));
        }
    };

    Ok(Json(
        serde_json::json!({ "updated": result.rows_affected() }),
    ))
}

// =================== Submission Details ===================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewedByInfo {
    pub username: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmissionDetailsResponse {
    #[serde(flatten)]
    pub submission: FlaggedSubmissionResponse,
    pub flag_reviewed_by: Option<ReviewedByInfo>,
    pub flag_reviewed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub has_security_data: bool,
}

pub async fn get_submission_details(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Path(attendance_id): Path<String>,
) -> Result<impl IntoResponse> {
    let attendance_uuid = Uuid::parse_str(&attendance_id)
        .map_err(|e| AppError::BadRequest(format!("Invalid attendance ID: {}", e)))?;

    assert_attendance_owned(&state.db, attendance_uuid, &auth).await?;

    let attendance: Attendance = sqlx::query_as("SELECT * FROM attendances WHERE id = $1")
        .bind(attendance_uuid)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound("Attendance record not found".to_string()))?;

    let has_security_data = !attendance.gps_anomalies.0.is_empty()
        || !attendance.emulator_flags.0.is_empty()
        || !attendance.integrity_checks.0.is_empty()
        || attendance.gps_accuracy.is_some();

    let flag_reviewed_by_id = attendance.flag_reviewed_by;
    let flag_reviewed_at = attendance.flag_reviewed_at;

    let flag_reviewed_by = if let Some(admin_id) = flag_reviewed_by_id {
        sqlx::query_as::<_, Admin>("SELECT * FROM admins WHERE id = $1")
            .bind(admin_id)
            .fetch_optional(&state.db)
            .await?
            .map(|admin| ReviewedByInfo {
                username: admin.username,
            })
    } else {
        None
    };

    let details = SubmissionDetailsResponse {
        submission: FlaggedSubmissionResponse::from_attendance(attendance),
        flag_reviewed_by,
        flag_reviewed_at,
        has_security_data,
    };

    Ok(Json(details))
}

// =================== Security Settings ===================

#[derive(Debug, Serialize)]
pub struct SecuritySettingsResponse {
    pub gps_validation: crate::models::GpsValidationConfig,
    pub emulator_detection: crate::models::EmulatorDetectionConfig,
    pub trust_score: crate::models::TrustScoreConfig,
}

pub async fn get_security_settings(
    State(state): State<Arc<crate::AppState>>,
    Extension(_auth): Extension<AuthenticatedAdmin>,
) -> Result<impl IntoResponse> {
    let config = SystemConfig::load(&state.db).await?.unwrap_or_default();

    Ok(Json(SecuritySettingsResponse {
        gps_validation: config.gps_validation,
        emulator_detection: config.emulator_detection,
        trust_score: config.trust_score,
    }))
}

#[derive(Debug, Deserialize)]
pub struct UpdateSecuritySettingsRequest {
    pub gps_validation: Option<crate::models::GpsValidationConfig>,
    pub emulator_detection: Option<crate::models::EmulatorDetectionConfig>,
    pub trust_score: Option<crate::models::TrustScoreConfig>,
}

pub async fn update_security_settings(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Json(payload): Json<UpdateSecuritySettingsRequest>,
) -> Result<impl IntoResponse> {
    auth.require_role(crate::constants::ROLE_SUPER_ADMIN)?;

    let mut config = SystemConfig::load(&state.db).await?.unwrap_or_default();

    if let Some(gps) = payload.gps_validation {
        config.gps_validation = gps;
    }
    if let Some(emu) = payload.emulator_detection {
        config.emulator_detection = emu;
    }
    if let Some(trust) = payload.trust_score {
        config.trust_score = trust;
    }

    config.updated_by = Some(auth.id);
    config.updated_at = Utc::now();

    let config = config.save(&state.db).await?;

    if let Err(e) = crate::models::record_audit_event(
        &state.db,
        Some(auth.id),
        "security_settings_updated",
        serde_json::json!({}),
        None,
    )
    .await
    {
        tracing::error!(error = %e, "Failed to record audit log entry for security_settings_updated");
    }

    Ok(Json(serde_json::json!({
        "message": "Security settings updated",
        "config": {
            "gpsValidation": config.gps_validation,
            "emulatorDetection": config.emulator_detection,
            "trustScore": config.trust_score,
        }
    })))
}

/// Recomputes the admin audit log's hash chain from the first row and
/// reports whether it's intact. A break means a row was altered or deleted
/// directly in the database, bypassing the application entirely — see
/// models::audit_log for how the chain itself is constructed.
pub async fn verify_audit_log(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
) -> Result<impl IntoResponse> {
    auth.require_role(crate::constants::ROLE_SUPER_ADMIN)?;

    let broken_at_seq = crate::models::verify_chain(&state.db).await?;

    Ok(Json(serde_json::json!({
        "intact": broken_at_seq.is_none(),
        "brokenAtSeq": broken_at_seq,
    })))
}

#[cfg(test)]
mod flagged_submission_contract_tests {
    use super::*;

    fn sample_response() -> FlaggedSubmissionResponse {
        FlaggedSubmissionResponse {
            id: "507f1f77bcf86cd799439011".to_string(),
            session_id: "6f1f77bc-f86c-4d79-9439-011111111111".to_string(),
            roll_number: "CS101".to_string(),
            student_name: "Alice".to_string(),
            captured_at: Utc::now(),
            flagged: true,
            flag_reason: Some("GPS anomalies detected".to_string()),
            flag_reviewed: false,
            flag_severity: Some(crate::models::Severity::High),
            review_notes: None,
            gps_confidence: Some(GpsConfidence::Suspicious),
            gps_anomalies: vec![],
            emulator_detected: false,
            emulator_flags: vec![],
            integrity_checks: vec![],
        }
    }

    /// Regression test: SecurityReview.tsx keys each flagged submission by
    /// `_id` (not `id`) and expects camelCase throughout. Without the
    /// `#[serde(rename = "_id")]` + `rename_all = "camelCase")]` pair here,
    /// "View Details" would request `.../attendance/undefined/details`.
    #[test]
    fn flagged_submission_response_serializes_underscore_id_and_camel_case() {
        let json = serde_json::to_value(sample_response()).unwrap();

        assert_eq!(json["_id"], "507f1f77bcf86cd799439011");
        assert!(json.get("id").is_none());
        assert_eq!(json["rollNumber"], "CS101");
    }

    #[test]
    fn flagged_submission_response_field_names() {
        let json = serde_json::to_value(sample_response()).unwrap();

        for key in [
            "_id",
            "rollNumber",
            "studentName",
            "capturedAt",
            "flagged",
            "flagReason",
            "flagReviewed",
            "gpsConfidence",
            "gpsAnomalies",
            "emulatorDetected",
            "emulatorFlags",
            "integrityChecks",
        ] {
            assert!(json.get(key).is_some(), "missing expected key `{key}`");
        }
        assert_eq!(json["gpsConfidence"], "suspicious");
    }

    /// Regression test: get_flagged_submissions must wrap the list as
    /// `{"submissions": [...]}` — SecurityReview.tsx reads `data.submissions`
    /// and previously got `undefined` back from a bare array response.
    #[test]
    fn flagged_submissions_list_is_wrapped_in_submissions_key() {
        let wrapped = serde_json::json!({ "submissions": [sample_response()] });
        assert!(wrapped["submissions"].is_array());
        assert_eq!(wrapped["submissions"].as_array().unwrap().len(), 1);
    }

    /// Regression test: SubmissionDetailsResponse flattens the submission
    /// fields (via #[serde(flatten)]) alongside flagReviewedBy/flagReviewedAt
    /// so the frontend's `{...submission, ...data}` spread actually adds new
    /// information instead of nested objects the UI never reads.
    #[test]
    fn submission_details_response_flattens_and_adds_review_info() {
        let details = SubmissionDetailsResponse {
            submission: sample_response(),
            flag_reviewed_by: Some(ReviewedByInfo {
                username: "admin1".to_string(),
            }),
            flag_reviewed_at: Some(Utc::now()),
            has_security_data: true,
        };

        let json = serde_json::to_value(&details).unwrap();
        // Flattened submission fields appear at the top level, not nested
        // under e.g. `attendance`/`gps`/`emulator` as the old shape did.
        assert_eq!(json["_id"], "507f1f77bcf86cd799439011");
        assert_eq!(json["rollNumber"], "CS101");
        assert_eq!(json["flagReviewedBy"]["username"], "admin1");
        assert!(json.get("flagReviewedAt").is_some());
        assert_eq!(json["hasSecurityData"], true);
    }
}
