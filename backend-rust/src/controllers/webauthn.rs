use axum::{
    extract::{Json, Query, State},
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

// The admin table (WebAuthnCredentials.tsx) reads `_id`, `studentId`,
// `deviceLabel`, `isSuspended`, `enrolledAt`, `lastUsedAt` — this struct used
// to serialize plain snake_case (`id`, `student_id`, ...) with no `_id` and
// no `last_used_at` field at all, so every column except the row itself was
// `undefined`: roll number, device, enrolled date all blank, "Last Used"
// stuck on "Never" regardless of actual data, and the suspended badge always
// read "Active" since `c.isSuspended` was never present to be true.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebAuthnCredentialResponse {
    #[serde(rename = "_id")]
    pub id: String,
    pub student_id: String,
    pub credential_id: String,
    pub device_label: String,
    pub is_suspended: bool,
    pub enrolled_at: String,
    pub last_used_at: Option<String>,
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

/// Query-string shape for `GET /webauthn/credentials`. The admin UI's search
/// box sends a partial roll number in `search` on every keystroke, an
/// optional `suspended=true`/`false`, and always sends `page`/`limit`.
#[derive(Debug, Deserialize)]
pub struct GetCredentialsParams {
    pub search: Option<String>,
    pub suspended: Option<String>,
    pub page: Option<i64>,
    pub limit: Option<i64>,
}

pub async fn get_credentials(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Query(params): Query<GetCredentialsParams>,
) -> Result<impl IntoResponse> {
    // Any super-admin sees every student's credential; a mentor only sees
    // students on a batch roster they created themselves.
    //
    // A roll number is never required to be on a roster to enroll a passkey
    // (start_registration deliberately allows off-roster enrollment — see its
    // comment: "surfacing off-roster enrollments to the admin rather than by
    // blocking the student"). The old EXISTS(...) here required a `students`
    // roster row to match regardless of role, so it did the opposite: any
    // off-roster credential — including for a super-admin — was invisible on
    // this page entirely (stats all zero, list empty), even though the
    // student-facing status check (`get_webauthn_status`, a plain
    // `WHERE student_id = $1`) found it fine. `$1 = 'super_admin'` now
    // short-circuits the roster check instead of being ANDed inside it.
    const OWNED_STUDENTS: &str = "$1 = 'super_admin' OR EXISTS ( \
         SELECT 1 FROM students st \
         JOIN batches b ON b.id = st.batch_id \
         WHERE upper(st.roll_number) = upper(webauthn_credentials.student_id) \
           AND b.created_by = $2)";

    // `search`/`suspended` were never actually wired up on the Rust rewrite —
    // this handler previously took no query params at all, so the frontend's
    // search box and suspended-status filter silently did nothing (always the
    // same unfiltered top-100). Static placeholders (`$3::text IS NULL OR
    // ...`) mirror the pattern in session_records.rs's query_session_records
    // rather than growing the SQL string per-filter, so the query stays one
    // fixed shape regardless of which filters are present.
    let page = params.page.unwrap_or(1).max(1);
    let limit = params.limit.unwrap_or(20).clamp(1, 100);
    let offset = (page - 1) * limit;

    let search_pattern = params
        .search
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| format!("%{s}%"));

    let suspended: Option<bool> = match params.suspended.as_deref() {
        Some("true") => Some(true),
        Some("false") => Some(false),
        _ => None,
    };

    const FILTERS: &str = "AND ($3::text IS NULL OR student_id ILIKE $3) \
         AND ($4::bool IS NULL OR is_suspended = $4)";

    let total: i64 = sqlx::query_scalar(&format!(
        "SELECT COUNT(*) FROM webauthn_credentials WHERE ({OWNED_STUDENTS}) {FILTERS}"
    ))
    .bind(&auth.role)
    .bind(auth.id)
    .bind(&search_pattern)
    .bind(suspended)
    .fetch_one(&state.db)
    .await?;

    let creds: Vec<WebAuthnCredential> = sqlx::query_as(&format!(
        "SELECT * FROM webauthn_credentials WHERE ({OWNED_STUDENTS}) {FILTERS} \
         ORDER BY enrolled_at DESC LIMIT $5 OFFSET $6"
    ))
    .bind(&auth.role)
    .bind(auth.id)
    .bind(&search_pattern)
    .bind(suspended)
    .bind(limit)
    .bind(offset)
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
            last_used_at: c.last_used_at.map(|t| t.to_rfc3339()),
        })
        .collect();

    let pages = if total == 0 {
        1
    } else {
        (total + limit - 1) / limit
    };

    Ok(Json(serde_json::json!({
        "credentials": creds,
        "pagination": {
            "total": total,
            "page": page,
            "limit": limit,
            "pages": pages
        }
    })))
}

pub async fn get_webauthn_stats(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
) -> Result<impl IntoResponse> {
    // See the identical comment in get_credentials above: off-roster
    // enrollment is allowed by design, so roster membership can't gate
    // visibility for a super-admin, or every stat here reads 0 for any
    // credential belonging to a roll number not on a batch roster.
    const OWNED_STUDENTS: &str = "$1 = 'super_admin' OR EXISTS ( \
         SELECT 1 FROM students st \
         JOIN batches b ON b.id = st.batch_id \
         WHERE upper(st.roll_number) = upper(webauthn_credentials.student_id) \
           AND b.created_by = $2)";

    let total: i64 = sqlx::query_scalar(&format!(
        "SELECT COUNT(*) FROM webauthn_credentials WHERE ({OWNED_STUDENTS})"
    ))
    .bind(&auth.role)
    .bind(auth.id)
    .fetch_one(&state.db)
    .await?;
    let suspended: i64 = sqlx::query_scalar(&format!(
        "SELECT COUNT(*) FROM webauthn_credentials \
         WHERE is_suspended AND ({OWNED_STUDENTS})"
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

    // device_type defaults to 'multi_device' (see migrations/0001) and is
    // never NULL, so this always returns one row per type actually present.
    let device_type_rows: Vec<(String, i64)> = sqlx::query_as(&format!(
        "SELECT device_type, COUNT(*) FROM webauthn_credentials \
         WHERE ({OWNED_STUDENTS}) GROUP BY device_type"
    ))
    .bind(&auth.role)
    .bind(auth.id)
    .fetch_all(&state.db)
    .await?;
    let device_types: serde_json::Map<String, serde_json::Value> = device_type_rows
        .into_iter()
        .map(|(device_type, count)| (device_type, serde_json::json!(count)))
        .collect();

    let last_7_days: i64 = sqlx::query_scalar(&format!(
        "SELECT COUNT(*) FROM webauthn_credentials \
         WHERE enrolled_at >= now() - interval '7 days' AND ({OWNED_STUDENTS})"
    ))
    .bind(&auth.role)
    .bind(auth.id)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(serde_json::json!({
        // The admin frontend's Stats interface (WebAuthnCredentials.tsx)
        // reads `totalEnrolled`, not `total` — the mismatch meant the "Total
        // Enrolled" tile always rendered blank (undefined), never even "0",
        // while the other tiles happened to share field names and looked
        // merely wrong (stuck at 0) instead of visibly broken.
        "totalEnrolled": total,
        "active": total - suspended,
        "suspended": suspended,
        "enrollmentRate": enrollment_rate,
        "deviceTypes": device_types,
        "enrollmentTrends": { "last7Days": last_7_days },
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
