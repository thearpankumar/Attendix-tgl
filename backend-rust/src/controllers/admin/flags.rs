use axum::{
    extract::{Json, State},
    response::IntoResponse,
    Extension,
};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    error::{AppError, Result},
    middleware::AuthenticatedAdmin,
    models::{Attendance, Session},
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyRequest {
    pub verified: bool,
}

pub async fn verify_attendance(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    axum::extract::Path(id): axum::extract::Path<String>,
    Json(payload): Json<VerifyRequest>,
) -> Result<impl IntoResponse> {
    auth.require_role(crate::constants::ROLE_SUPER_ADMIN)?;

    let attendance_id = Uuid::parse_str(&id)
        .map_err(|e| AppError::BadRequest(format!("Invalid attendance ID: {}", e)))?;

    // Get attendance and verify ownership
    let attendance = sqlx::query_as::<_, Attendance>("SELECT * FROM attendances WHERE id = $1")
        .bind(attendance_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound("Attendance not found".to_string()))?;

    // Verify the session exists (route already requires super-admin above,
    // and any super-admin may verify attendance on any session).
    sqlx::query_as::<_, Session>(
        "SELECT * FROM sessions WHERE id = $1 AND ($2 = 'super_admin' OR created_by = $3)",
    )
    .bind(attendance.session_id)
    .bind(&auth.role)
    .bind(auth.id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("Session not found".to_string()))?;

    sqlx::query("UPDATE attendances SET verified = $1 WHERE id = $2")
        .bind(payload.verified)
        .bind(attendance_id)
        .execute(&state.db)
        .await?;

    Ok(Json(serde_json::json!({
        "message": if payload.verified { "Marked verified" } else { "Marked unverified" },
        "verified": payload.verified
    })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkVerifyRequest {
    pub ids: Vec<String>,
    pub verified: bool,
}

pub async fn bulk_verify_attendance(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    axum::extract::Path(session_id): axum::extract::Path<String>,
    Json(payload): Json<BulkVerifyRequest>,
) -> Result<impl IntoResponse> {
    if payload.ids.is_empty() {
        return Err(AppError::BadRequest(
            "ids must be a non-empty array".to_string(),
        ));
    }
    if payload.ids.len() > 100 {
        return Err(AppError::BadRequest(
            "Cannot bulk-update more than 100 records at once".to_string(),
        ));
    }

    let session_uuid = Uuid::parse_str(&session_id)
        .map_err(|e| AppError::BadRequest(format!("Invalid session ID: {}", e)))?;

    // Verify session access — any super-admin may bulk-verify any session;
    // a mentor (who never has a matching `created_by`) is denied.
    sqlx::query_as::<_, Session>(
        "SELECT * FROM sessions WHERE id = $1 AND ($2 = 'super_admin' OR created_by = $3)",
    )
    .bind(session_uuid)
    .bind(&auth.role)
    .bind(auth.id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("Session not found".to_string()))?;

    let ids: Result<Vec<Uuid>> = payload
        .ids
        .iter()
        .map(|id| {
            Uuid::parse_str(id)
                .map_err(|e| AppError::BadRequest(format!("Invalid attendance ID: {}", e)))
        })
        .collect();
    let ids = ids?;

    let result =
        sqlx::query("UPDATE attendances SET verified = $1 WHERE id = ANY($2) AND session_id = $3")
            .bind(payload.verified)
            .bind(&ids)
            .bind(session_uuid)
            .execute(&state.db)
            .await?;

    Ok(Json(
        serde_json::json!({ "updated": result.rows_affected() }),
    ))
}
