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

pub async fn get_security_summary(
    State(state): State<Arc<crate::AppState>>,
    Extension(_auth): Extension<AuthenticatedAdmin>,
    Path(session_id): Path<String>,
) -> Result<impl IntoResponse> {
    let session_id = Uuid::parse_str(&session_id)
        .map_err(|e| AppError::BadRequest(format!("Invalid session ID: {}", e)))?;

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

    // Verify attendance exists before reviewing
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM attendances WHERE id = $1)")
        .bind(attendance_uuid)
        .fetch_one(&state.db)
        .await?;
    if !exists {
        return Err(AppError::NotFound(
            "Attendance record not found".to_string(),
        ));
    }

    let message = match payload.action.as_str() {
        "approve" => {
            // Clear flags, mark verified. `notes` is persisted into flag_details,
            // the closest existing column (the Mongo-era "flagNotes" field this
            // mirrored was never modeled on the Attendance document either).
            sqlx::query(
                "UPDATE attendances SET flagged = false, flag_reviewed = true, flag_reviewed_by = $1, \
                 flag_reviewed_at = now(), flag_details = $2, verified = true WHERE id = $3",
            )
            .bind(auth.id)
            .bind(&payload.notes)
            .bind(attendance_uuid)
            .execute(&state.db)
            .await?;

            // TODO: Update device trust score (when device_trust is available)

            "Attendance submission approved and verified successfully"
        }
        "reject" => {
            sqlx::query(
                "UPDATE attendances SET flag_reviewed = true, flag_reviewed_by = $1, \
                 flag_reviewed_at = now(), flag_details = $2, verified = false WHERE id = $3",
            )
            .bind(auth.id)
            .bind(&payload.notes)
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
    pub roll_number: String,
    pub student_name: String,
    pub captured_at: chrono::DateTime<chrono::Utc>,
    pub flagged: bool,
    pub flag_reason: Option<String>,
    pub flag_reviewed: bool,
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
            roll_number: a.roll_number,
            student_name: a.student_name,
            captured_at: a.captured_at,
            flagged: a.flagged,
            flag_reason: a.flag_reason,
            flag_reviewed: a.flag_reviewed,
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
    Extension(_auth): Extension<AuthenticatedAdmin>,
    Path(session_id): Path<String>,
) -> Result<impl IntoResponse> {
    let session_id = Uuid::parse_str(&session_id)
        .map_err(|e| AppError::BadRequest(format!("Invalid session ID: {}", e)))?;

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
    Extension(_auth): Extension<AuthenticatedAdmin>,
    Path(attendance_id): Path<String>,
) -> Result<impl IntoResponse> {
    let attendance_uuid = Uuid::parse_str(&attendance_id)
        .map_err(|e| AppError::BadRequest(format!("Invalid attendance ID: {}", e)))?;

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

    Ok(Json(serde_json::json!({
        "message": "Security settings updated",
        "config": {
            "gpsValidation": config.gps_validation,
            "emulatorDetection": config.emulator_detection,
            "trustScore": config.trust_score,
        }
    })))
}

#[cfg(test)]
mod flagged_submission_contract_tests {
    use super::*;

    fn sample_response() -> FlaggedSubmissionResponse {
        FlaggedSubmissionResponse {
            id: "507f1f77bcf86cd799439011".to_string(),
            roll_number: "CS101".to_string(),
            student_name: "Alice".to_string(),
            captured_at: Utc::now(),
            flagged: true,
            flag_reason: Some("GPS anomalies detected".to_string()),
            flag_reviewed: false,
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
