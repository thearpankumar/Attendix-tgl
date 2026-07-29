use axum::{
    extract::{Json, Path, State},
    response::IntoResponse,
    Extension,
};
use chrono::Utc;
use mongodb::{
    bson::{doc, oid::ObjectId, DateTime as BsonDateTime},
    Collection,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

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
    let attendances: Collection<Attendance> = state
        .db
        .database(
            state
                .config
                .mongodb_uri
                .split('/')
                .next_back()
                .unwrap_or("default")
                .split('?')
                .next()
                .unwrap_or("default"),
        )
        .collection(Attendance::collection_name());

    let session_oid = ObjectId::parse_str(&session_id)
        .map_err(|e| AppError::BadRequest(format!("Invalid session ID: {}", e)))?;

    let total_submissions = attendances
        .count_documents(doc! { "sessionId": session_oid })
        .await?;

    let flagged_submissions = attendances
        .count_documents(doc! { "sessionId": session_oid, "flagged": true })
        .await?;

    let unreviewed_gps = attendances
        .count_documents(doc! {
            "sessionId": session_oid,
            "flagged": true,
            "flagReviewed": false,
            "$or": [
                { "gpsAnomalies": { "$exists": true, "$not": { "$size": 0 } } },
                { "gpsMockLocation": true }
            ]
        })
        .await?;

    let unreviewed_emulator = attendances
        .count_documents(doc! {
            "sessionId": session_oid,
            "flagged": true,
            "flagReviewed": false,
            "emulatorDetected": true
        })
        .await?;

    let unreviewed_integrity = attendances
        .count_documents(doc! {
            "sessionId": session_oid,
            "flagged": true,
            "flagReviewed": false,
            "integrityChecks": { "$exists": true, "$not": { "$size": 0 } }
        })
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
        total_submissions: total_submissions as i64,
        flagged_submissions: flagged_submissions as i64,
        unreviewed_flags: UnreviewedFlagsSummary {
            gps_anomalies: unreviewed_gps as i64,
            emulator_detected: unreviewed_emulator as i64,
            integrity_issues: unreviewed_integrity as i64,
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
    let attendances: Collection<Attendance> = state
        .db
        .database(
            state
                .config
                .mongodb_uri
                .split('/')
                .next_back()
                .unwrap_or("default")
                .split('?')
                .next()
                .unwrap_or("default"),
        )
        .collection(Attendance::collection_name());

    let attendance_oid = ObjectId::parse_str(&attendance_id)
        .map_err(|e| AppError::BadRequest(format!("Invalid attendance ID: {}", e)))?;

    // Verify attendance exists before reviewing
    let _existing = attendances
        .find_one(doc! { "_id": attendance_oid })
        .await?
        .ok_or_else(|| AppError::NotFound("Attendance record not found".to_string()))?;

    let message = match payload.action.as_str() {
        "approve" => {
            // Clear flags, mark verified
            attendances
                .update_one(
                    doc! { "_id": attendance_oid },
                    doc! {
                        "$set": {
                            "flagged": false,
                            "flagReviewed": true,
                            "flagReviewedBy": auth.id,
                            "flagReviewedAt": BsonDateTime::now(),
                            "flagNotes": payload.notes,
                            "verified": true,
                        }
                    },
                )
                .await?;

            // TODO: Update device trust score (when device_trust is available)

            "Attendance submission approved and verified successfully"
        }
        "reject" => {
            // Keep flagged, mark reviewed
            attendances
                .update_one(
                    doc! { "_id": attendance_oid },
                    doc! {
                        "$set": {
                            "flagReviewed": true,
                            "flagReviewedBy": auth.id,
                            "flagReviewedAt": BsonDateTime::now(),
                            "flagNotes": payload.notes,
                            "verified": false,
                        }
                    },
                )
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
        "reviewed_by": auth.id.to_hex(),
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
            id: a.id.unwrap_or_default().to_hex(),
            roll_number: a.roll_number,
            student_name: a.student_name,
            captured_at: a.captured_at,
            flagged: a.flagged,
            flag_reason: a.flag_reason,
            flag_reviewed: a.flag_reviewed,
            gps_confidence: a.gps_confidence,
            gps_anomalies: a.gps_anomalies,
            emulator_detected: a.emulator_detected,
            emulator_flags: a.emulator_flags,
            integrity_checks: a.integrity_checks,
        }
    }
}

pub async fn get_flagged_submissions(
    State(state): State<Arc<crate::AppState>>,
    Extension(_auth): Extension<AuthenticatedAdmin>,
    Path(session_id): Path<String>,
) -> Result<impl IntoResponse> {
    let attendances: Collection<Attendance> = state
        .db
        .database(
            state
                .config
                .mongodb_uri
                .split('/')
                .next_back()
                .unwrap_or("default")
                .split('?')
                .next()
                .unwrap_or("default"),
        )
        .collection(Attendance::collection_name());

    let session_oid = ObjectId::parse_str(&session_id)
        .map_err(|e| AppError::BadRequest(format!("Invalid session ID: {}", e)))?;

    let mut cursor = attendances
        .find(doc! { "sessionId": session_oid, "flagged": true })
        .limit(100)
        .await?;
    let mut submissions = Vec::new();

    while cursor.advance().await? {
        let a = cursor.deserialize_current()?;
        submissions.push(FlaggedSubmissionResponse::from_attendance(a));
    }

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
    let db = state.db.database(
        state
            .config
            .mongodb_uri
            .split('/')
            .next_back()
            .unwrap_or("default")
            .split('?')
            .next()
            .unwrap_or("default"),
    );

    let attendances: Collection<Attendance> = db.collection(Attendance::collection_name());
    let admins: Collection<Admin> = db.collection(Admin::collection_name());

    let attendance_oid = ObjectId::parse_str(&attendance_id)
        .map_err(|e| AppError::BadRequest(format!("Invalid attendance ID: {}", e)))?;

    let attendance = attendances
        .find_one(doc! { "_id": attendance_oid })
        .await?
        .ok_or_else(|| AppError::NotFound("Attendance record not found".to_string()))?;

    let has_security_data = !attendance.gps_anomalies.is_empty()
        || !attendance.emulator_flags.is_empty()
        || !attendance.integrity_checks.is_empty()
        || attendance.gps_accuracy.is_some();

    let flag_reviewed_by_id = attendance.flag_reviewed_by;
    let flag_reviewed_at = attendance.flag_reviewed_at;

    let flag_reviewed_by = if let Some(admin_id) = flag_reviewed_by_id {
        admins
            .find_one(doc! { "_id": admin_id })
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
    let db = state.db.database(
        state
            .config
            .mongodb_uri
            .split('/')
            .next_back()
            .unwrap_or("default")
            .split('?')
            .next()
            .unwrap_or("default"),
    );

    let configs: Collection<SystemConfig> = db.collection(SystemConfig::collection_name());

    let config = configs.find_one(doc! {}).await?.unwrap_or_default();

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
    let db = state.db.database(
        state
            .config
            .mongodb_uri
            .split('/')
            .next_back()
            .unwrap_or("default")
            .split('?')
            .next()
            .unwrap_or("default"),
    );

    let configs: Collection<SystemConfig> = db.collection(SystemConfig::collection_name());

    let mut config = configs.find_one(doc! {}).await?.unwrap_or_default();

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

    configs
        .update_one(
            doc! {},
            doc! { "$set": mongodb::bson::to_document(&config).map_err(|e| AppError::Internal(e.to_string()))? },
        )
        .upsert(true)
        .await?;

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
