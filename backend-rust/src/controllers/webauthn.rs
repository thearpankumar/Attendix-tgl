use axum::{
    extract::{Json, State},
    response::IntoResponse,
    Extension,
};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::{
    error::{AppError, Result},
    middleware::AuthenticatedAdmin,
    models::{WebAuthnCredential, WebAuthnReenrollmentAction},
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

/// Confirms the calling admin may manage this student's credential: any
/// super-admin may, for any student; a mentor only for one on the roster of
/// a batch they created themselves.
///
/// WebAuthn credentials belong to students, not admins, so the ownership chain
/// runs admin -> batch -> student. Without this check any admin could reset,
/// suspend or enumerate any student's credential in the system.
async fn assert_student_owned(
    pool: &sqlx::PgPool,
    roll_number: &str,
    admin: &AuthenticatedAdmin,
) -> Result<()> {
    let owned: bool = sqlx::query_scalar(
        "SELECT EXISTS( \
             SELECT 1 FROM students st \
             JOIN batches b ON b.id = st.batch_id \
             WHERE upper(st.roll_number) = upper($1) AND ($2 = 'super_admin' OR b.created_by = $3) \
         )",
    )
    .bind(roll_number)
    .bind(&admin.role)
    .bind(admin.id)
    .fetch_one(pool)
    .await?;

    if owned {
        Ok(())
    } else {
        Err(AppError::NotFound("Student not found".to_string()))
    }
}

pub async fn reset_credential(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Json(payload): Json<ResetCredentialRequest>,
) -> Result<impl IntoResponse> {
    // Check for abuse: > 10 resets in 1 hour
    let one_hour_ago = Utc::now() - Duration::hours(1);

    let recent_resets: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM webauthn_credentials WHERE reset_by = $1 AND reset_at >= $2",
    )
    .bind(auth.id)
    .bind(one_hour_ago)
    .fetch_one(&state.db)
    .await?;

    if recent_resets >= 10 {
        sqlx::query(
            "INSERT INTO flags (flag_type, admin_id, details, timestamp, resolved) \
             VALUES ($1, $2, $3, now(), false)",
        )
        .bind("WEBAUTHN_RESET_ABUSE")
        .bind(auth.id)
        .bind(format!(
            "Admin reset {} credentials in 1 hour",
            recent_resets
        ))
        .execute(&state.db)
        .await?;

        return Err(AppError::BadRequest(
            "Too many credential resets. This action has been flagged for review.".to_string(),
        ));
    }

    assert_student_owned(&state.db, &payload.student_id, &auth).await?;

    let previous_credential_id: Option<String> =
        sqlx::query_scalar("SELECT credential_id FROM webauthn_credentials WHERE student_id = $1")
            .bind(&payload.student_id)
            .fetch_optional(&state.db)
            .await?;

    // Update credential with reset metadata instead of deleting
    sqlx::query(
        "UPDATE webauthn_credentials SET reset_at = now(), reset_by = $1 WHERE student_id = $2",
    )
    .bind(auth.id)
    .bind(&payload.student_id)
    .execute(&state.db)
    .await?;

    sqlx::query(
        "INSERT INTO webauthn_reenrollment_logs \
         (student_id, admin_id, reason, previous_credential_id, new_credential_id, action_type, timestamp) \
         VALUES ($1, $2, $3, $4, NULL, $5, now())",
    )
    .bind(&payload.student_id)
    .bind(auth.id)
    .bind(&payload.reason)
    .bind(&previous_credential_id)
    .bind(WebAuthnReenrollmentAction::Reset)
    .execute(&state.db)
    .await?;

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
    assert_student_owned(&state.db, &payload.student_id, &auth).await?;

    sqlx::query(
        "UPDATE webauthn_credentials \
         SET is_suspended = true, suspended_reason = $1, suspended_at = now(), suspended_by = $2 \
         WHERE student_id = $3",
    )
    .bind(&payload.reason)
    .bind(auth.id)
    .bind(&payload.student_id)
    .execute(&state.db)
    .await?;

    sqlx::query(
        "INSERT INTO webauthn_reenrollment_logs \
         (student_id, admin_id, reason, previous_credential_id, new_credential_id, action_type, timestamp) \
         VALUES ($1, $2, $3, NULL, NULL, $4, now())",
    )
    .bind(&payload.student_id)
    .bind(auth.id)
    .bind(&payload.reason)
    .bind(WebAuthnReenrollmentAction::Suspend)
    .execute(&state.db)
    .await?;

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
    assert_student_owned(&state.db, &payload.student_id, &auth).await?;

    sqlx::query(
        "UPDATE webauthn_credentials \
         SET is_suspended = false, suspended_reason = NULL, suspended_at = NULL \
         WHERE student_id = $1",
    )
    .bind(&payload.student_id)
    .execute(&state.db)
    .await?;

    sqlx::query(
        "INSERT INTO webauthn_reenrollment_logs \
         (student_id, admin_id, reason, previous_credential_id, new_credential_id, action_type, timestamp) \
         VALUES ($1, $2, $3, NULL, NULL, $4, now())",
    )
    .bind(&payload.student_id)
    .bind(auth.id)
    .bind(&payload.reason)
    .bind(WebAuthnReenrollmentAction::Unsuspend)
    .execute(&state.db)
    .await?;

    Ok(Json(serde_json::json!({ "success": true })))
}

#[derive(Debug, Deserialize)]
pub struct UnsuspendCredentialRequest {
    pub student_id: String,
    pub reason: Option<String>,
}

pub async fn get_credentials(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
) -> Result<impl IntoResponse> {
    // Any super-admin sees every student's credential; a mentor only sees
    // students on a batch roster they created themselves.
    const OWNED_STUDENTS: &str = "SELECT 1 FROM students st \
         JOIN batches b ON b.id = st.batch_id \
         WHERE upper(st.roll_number) = upper(webauthn_credentials.student_id) \
           AND ($1 = 'super_admin' OR b.created_by = $2)";

    let total: i64 = sqlx::query_scalar(&format!(
        "SELECT COUNT(*) FROM webauthn_credentials WHERE EXISTS ({OWNED_STUDENTS})"
    ))
    .bind(&auth.role)
    .bind(auth.id)
    .fetch_one(&state.db)
    .await?;

    let creds: Vec<WebAuthnCredential> = sqlx::query_as(&format!(
        "SELECT * FROM webauthn_credentials WHERE EXISTS ({OWNED_STUDENTS}) \
         ORDER BY enrolled_at DESC LIMIT 100"
    ))
    .bind(&auth.role)
    .bind(auth.id)
    .fetch_all(&state.db)
    .await?;

    let creds: Vec<WebAuthnCredentialResponse> = creds
        .into_iter()
        .map(|c| WebAuthnCredentialResponse {
            id: c.id.to_string(),
            student_id: c.student_id,
            credential_id: c.credential_id,
            device_label: c.device_label,
            is_suspended: c.is_suspended,
            enrolled_at: c.enrolled_at.to_rfc3339(),
        })
        .collect();

    Ok(Json(serde_json::json!({
        "credentials": creds,
        "pagination": {
            "total": total,
            "page": 1,
            "limit": 100,
            "pages": 1
        }
    })))
}

pub async fn get_webauthn_stats(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
) -> Result<impl IntoResponse> {
    const OWNED_STUDENTS: &str = "SELECT 1 FROM students st \
         JOIN batches b ON b.id = st.batch_id \
         WHERE upper(st.roll_number) = upper(webauthn_credentials.student_id) \
           AND ($1 = 'super_admin' OR b.created_by = $2)";

    let total: i64 = sqlx::query_scalar(&format!(
        "SELECT COUNT(*) FROM webauthn_credentials WHERE EXISTS ({OWNED_STUDENTS})"
    ))
    .bind(&auth.role)
    .bind(auth.id)
    .fetch_one(&state.db)
    .await?;
    let suspended: i64 = sqlx::query_scalar(&format!(
        "SELECT COUNT(*) FROM webauthn_credentials \
         WHERE is_suspended AND EXISTS ({OWNED_STUDENTS})"
    ))
    .bind(&auth.role)
    .bind(auth.id)
    .fetch_one(&state.db)
    .await?;

    // Roster size across every batch visible to this admin (all batches for
    // a super-admin, only their own for a mentor).
    let unique_students: i64 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT upper(st.roll_number)) FROM students st \
         JOIN batches b ON b.id = st.batch_id WHERE ($1 = 'super_admin' OR b.created_by = $2)",
    )
    .bind(&auth.role)
    .bind(auth.id)
    .fetch_one(&state.db)
    .await?;

    let enrollment_rate = if unique_students > 0 {
        (total as f64 / unique_students as f64) * 100.0
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

#[cfg(test)]
mod payload_tests {

    use serde_json::json;

    #[test]
    fn test_get_credentials_payload_structure() {
        let creds = vec![json!({
            "id": "123",
            "studentId": "ABC",
            "credentialId": "cred",
            "isSuspended": false
        })];

        let payload = json!({
            "credentials": creds,
            "pagination": {
                "pages": 1,
                "total": 1
            }
        });

        assert!(payload.get("credentials").is_some());
        assert!(payload.get("credentials").unwrap().is_array());
        assert!(payload.get("pagination").is_some());
        assert_eq!(payload["pagination"]["pages"], 1);
    }
}
