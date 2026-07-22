use axum::{
    extract::{Json, State},
    response::IntoResponse,
    Extension,
};
use chrono::{Duration, Utc};
use mongodb::{
    bson::{doc, DateTime as BsonDateTime},
    Collection,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::{
    error::{AppError, Result},
    middleware::AuthenticatedAdmin,
    models::{
        Attendance, Flag, WebAuthnCredential, WebAuthnReenrollmentAction, WebAuthnReenrollmentLog,
    },
};

#[derive(Debug, Serialize)]
pub struct WebAuthnCredentialResponse {
    pub id: String,
    pub student_id: String,
    pub credential_id: String,
    pub device_label: String,
    pub is_suspended: bool,
    pub enrolled_at: String,
}

pub async fn reset_credential(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Json(payload): Json<ResetCredentialRequest>,
) -> Result<impl IntoResponse> {
    let db_name = state
        .config
        .mongodb_uri
        .split('/')
        .next_back()
        .unwrap_or("default");
    let db = state.db.database(db_name);

    let credentials: Collection<WebAuthnCredential> =
        db.collection(WebAuthnCredential::collection_name());
    let logs: Collection<WebAuthnReenrollmentLog> =
        db.collection(WebAuthnReenrollmentLog::collection_name());
    let flags: Collection<Flag> = db.collection(Flag::collection_name());

    // Check for abuse: > 10 resets in 1 hour
    let one_hour_ago = Utc::now() - Duration::hours(1);

    let recent_resets = credentials
        .count_documents(doc! {
            "resetBy": &auth.id,
            "resetAt": { "$gte": BsonDateTime::from_millis(one_hour_ago.timestamp_millis()) }
        })
        .await?;

    if recent_resets >= 10 {
        // Create abuse flag
        let abuse_flag = Flag {
            id: None,
            flag_type: "WEBAUTHN_RESET_ABUSE".to_string(),
            admin_id: Some(auth.id),
            student_id: None,
            details: Some(format!(
                "Admin reset {} credentials in 1 hour",
                recent_resets
            )),
            session_id: None,
            timestamp: Utc::now(),
            resolved: false,
            resolved_by: None,
            resolved_at: None,
        };

        flags.insert_one(&abuse_flag).await?;

        return Err(AppError::BadRequest(
            "Too many credential resets. This action has been flagged for review.".to_string(),
        ));
    }

    let credential = credentials
        .find_one(doc! { "studentId": &payload.student_id })
        .await?;

    let previous_credential_id = credential.as_ref().map(|c| c.credential_id.clone());

    // Update credential with reset metadata instead of deleting
    credentials
        .update_one(
            doc! { "studentId": &payload.student_id },
            doc! {
                "$set": {
                    "resetAt": BsonDateTime::now(),
                    "resetBy": &auth.id,
                }
            },
        )
        .await?;

    let log = WebAuthnReenrollmentLog {
        id: None,
        student_id: payload.student_id.clone(),
        admin_id: auth.id,
        reason: payload.reason,
        previous_credential_id,
        new_credential_id: None,
        action_type: WebAuthnReenrollmentAction::Reset,
        timestamp: Utc::now(),
    };

    logs.insert_one(&log).await?;

    Ok(Json(
        serde_json::json!({ "success": true, "message": "Credential reset successfully" }),
    ))
}

#[derive(Debug, Deserialize)]
pub struct ResetCredentialRequest {
    pub student_id: String,
    pub reason: Option<String>,
}

pub async fn suspend_credential(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Json(payload): Json<SuspendCredentialRequest>,
) -> Result<impl IntoResponse> {
    let credentials: Collection<WebAuthnCredential> = state
        .db
        .database(
            state
                .config
                .mongodb_uri
                .split('/')
                .next_back()
                .unwrap_or("default"),
        )
        .collection(WebAuthnCredential::collection_name());
    let logs: Collection<WebAuthnReenrollmentLog> = state
        .db
        .database(
            state
                .config
                .mongodb_uri
                .split('/')
                .next_back()
                .unwrap_or("default"),
        )
        .collection(WebAuthnReenrollmentLog::collection_name());

    credentials.update_one(
        doc! { "studentId": &payload.student_id },
        doc! { "$set": { "isSuspended": true, "suspendedReason": &payload.reason, "suspendedAt": BsonDateTime::now(), "suspendedBy": auth.id } },
    ).await?;

    let log = WebAuthnReenrollmentLog {
        id: None,
        student_id: payload.student_id.clone(),
        admin_id: auth.id,
        reason: payload.reason.clone(),
        previous_credential_id: None,
        new_credential_id: None,
        action_type: WebAuthnReenrollmentAction::Suspend,
        timestamp: Utc::now(),
    };

    logs.insert_one(&log).await?;

    Ok(Json(serde_json::json!({ "success": true })))
}

#[derive(Debug, Deserialize)]
pub struct SuspendCredentialRequest {
    pub student_id: String,
    pub reason: Option<String>,
}

pub async fn unsuspend_credential(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Json(payload): Json<UnsuspendCredentialRequest>,
) -> Result<impl IntoResponse> {
    let credentials: Collection<WebAuthnCredential> = state
        .db
        .database(
            state
                .config
                .mongodb_uri
                .split('/')
                .next_back()
                .unwrap_or("default"),
        )
        .collection(WebAuthnCredential::collection_name());
    let logs: Collection<WebAuthnReenrollmentLog> = state
        .db
        .database(
            state
                .config
                .mongodb_uri
                .split('/')
                .next_back()
                .unwrap_or("default"),
        )
        .collection(WebAuthnReenrollmentLog::collection_name());

    credentials
        .update_one(
            doc! { "studentId": &payload.student_id },
            doc! { "$set": { "isSuspended": false, "suspendedReason": null, "suspendedAt": null } },
        )
        .await?;

    let log = WebAuthnReenrollmentLog {
        id: None,
        student_id: payload.student_id.clone(),
        admin_id: auth.id,
        reason: payload.reason,
        previous_credential_id: None,
        new_credential_id: None,
        action_type: WebAuthnReenrollmentAction::Unsuspend,
        timestamp: Utc::now(),
    };

    logs.insert_one(&log).await?;

    Ok(Json(serde_json::json!({ "success": true })))
}

#[derive(Debug, Deserialize)]
pub struct UnsuspendCredentialRequest {
    pub student_id: String,
    pub reason: Option<String>,
}

pub async fn get_credentials(
    State(state): State<Arc<crate::AppState>>,
    Extension(_auth): Extension<AuthenticatedAdmin>,
) -> Result<impl IntoResponse> {
    let credentials: Collection<WebAuthnCredential> = state
        .db
        .database(
            state
                .config
                .mongodb_uri
                .split('/')
                .next_back()
                .unwrap_or("default"),
        )
        .collection(WebAuthnCredential::collection_name());

    let mut cursor = credentials
        .find(doc! {})
        .sort(doc! { "enrolledAt": -1 })
        .limit(100)
        .await?;
    let mut creds = Vec::new();

    while cursor.advance().await? {
        let c = cursor.deserialize_current()?;
        creds.push(WebAuthnCredentialResponse {
            id: c.id.unwrap().to_hex(),
            student_id: c.student_id,
            credential_id: c.credential_id,
            device_label: c.device_label,
            is_suspended: c.is_suspended,
            enrolled_at: c.enrolled_at.to_rfc3339(),
        });
    }

    Ok(Json(creds))
}

pub async fn get_webauthn_stats(
    State(state): State<Arc<crate::AppState>>,
    Extension(_auth): Extension<AuthenticatedAdmin>,
) -> Result<impl IntoResponse> {
    let db = state.db.database(
        state
            .config
            .mongodb_uri
            .split('/')
            .next_back()
            .unwrap_or("default"),
    );

    let credentials: Collection<WebAuthnCredential> =
        db.collection(WebAuthnCredential::collection_name());
    let attendances: Collection<Attendance> = db.collection(Attendance::collection_name());

    let total = credentials.count_documents(doc! {}).await?;
    let suspended = credentials
        .count_documents(doc! { "isSuspended": true })
        .await?;

    // Count unique students with credentials (enrolled)
    let credentials_count = total as i64;

    // Count unique students from attendance records (approximate total student count)
    let pipeline = vec![doc! { "$group": { "_id": "$rollNumber" } }];
    let mut cursor = attendances.aggregate(pipeline).await?;
    let mut unique_students = 0i64;
    while cursor.advance().await? {
        unique_students += 1;
    }

    let enrollment_rate = if unique_students > 0 {
        (credentials_count as f64 / unique_students as f64) * 100.0
    } else {
        0.0
    };

    Ok(Json(serde_json::json!({
        "total": total,
        "active": total - suspended,
        "suspended": suspended,
        "enrollmentRate": enrollment_rate,
    })))
}
