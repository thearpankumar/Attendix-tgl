// User Management: super-admin-only CRUD over admin/mentor accounts.
//
// This is the *only* place new admin/mentor accounts can be created once the
// one-time bootstrap registration route (`POST /api/admin/register`) has
// closed itself after the first super_admin exists — see
// `controllers/admin/auth.rs::register`.

use axum::{
    extract::{Json, Path, State},
    response::IntoResponse,
    Extension,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    constants::{password_meets_policy, MIN_PASSWORD_LEN, ROLE_ADMIN, ROLE_SUPER_ADMIN},
    error::{AppError, Result},
    middleware::{validators::normalize_email, AuthenticatedAdmin},
    models::{record_audit_event, Admin},
    utils::input::{apply_patch, patch_text},
};

const MAX_TEXT_LEN: usize = 200;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminUserResponse {
    #[serde(rename = "_id")]
    pub id: String,
    pub username: String,
    pub email: String,
    pub full_name: Option<String>,
    pub college_name: Option<String>,
    pub role: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

impl From<Admin> for AdminUserResponse {
    fn from(a: Admin) -> Self {
        Self {
            id: a.id.to_string(),
            username: a.username,
            email: a.email,
            full_name: a.full_name,
            college_name: a.college_name,
            role: a.role,
            is_active: a.is_active,
            created_at: a.created_at,
        }
    }
}

fn is_known_role(role: &str) -> bool {
    role == ROLE_ADMIN || role == ROLE_SUPER_ADMIN
}

/// Trims an optional text field; blank counts as absent.
fn clean_text(value: Option<&str>, label: &str) -> Result<Option<String>> {
    let cleaned = value.map(str::trim).filter(|s| !s.is_empty());
    if cleaned.is_some_and(|v| v.chars().count() > MAX_TEXT_LEN) {
        return Err(AppError::BadRequest(format!(
            "{label} is longer than {MAX_TEXT_LEN} characters"
        )));
    }
    Ok(cleaned.map(str::to_string))
}

fn check_password(password: &str, label: &str) -> Result<()> {
    if !password_meets_policy(password) {
        return Err(AppError::BadRequest(format!(
            "{label} must be at least {MIN_PASSWORD_LEN} characters"
        )));
    }
    Ok(())
}

/// Username and email are unique case-insensitively (`lower(...)` unique
/// indexes, migration 0016), so `ZZ_M1` and `zz_m1` are the same account name.
/// `except` is the account being edited, which may keep its own values.
async fn ensure_identity_free(
    db: &sqlx::PgPool,
    username: Option<&str>,
    email: Option<&str>,
    except: Option<Uuid>,
) -> Result<()> {
    if let Some(username) = username {
        let taken: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM admins WHERE lower(username) = lower($1) \
             AND ($2::uuid IS NULL OR id <> $2))",
        )
        .bind(username)
        .bind(except)
        .fetch_one(db)
        .await?;
        if taken {
            return Err(AppError::Conflict("Username already exists".to_string()));
        }
    }
    if let Some(email) = email {
        let taken: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM admins WHERE lower(email) = lower($1) \
             AND ($2::uuid IS NULL OR id <> $2))",
        )
        .bind(email)
        .bind(except)
        .fetch_one(db)
        .await?;
        if taken {
            return Err(AppError::Conflict("Email already exists".to_string()));
        }
    }
    Ok(())
}

/// Safety net for the race between the pre-check above and the write: a
/// unique-index violation is still a 409 with a clear message, never a 500.
fn map_identity_conflict(e: sqlx::Error) -> AppError {
    if let sqlx::Error::Database(db_err) = &e {
        if db_err.is_unique_violation() {
            let constraint = db_err.constraint().unwrap_or_default();
            return AppError::Conflict(if constraint.contains("email") {
                "Email already exists".to_string()
            } else {
                "Username already exists".to_string()
            });
        }
    }
    AppError::from(e)
}

pub async fn list_admin_users(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
) -> Result<impl IntoResponse> {
    auth.require_role(ROLE_SUPER_ADMIN)?;

    let admins: Vec<Admin> =
        sqlx::query_as("SELECT * FROM admins WHERE username <> $1 ORDER BY created_at DESC")
            .bind(crate::constants::CANARY_ADMIN_USERNAME)
            .fetch_all(&state.db)
            .await?;

    let response: Vec<AdminUserResponse> =
        admins.into_iter().map(AdminUserResponse::from).collect();
    Ok(Json(response))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAdminUserRequest {
    pub username: String,
    pub email: String,
    pub full_name: Option<String>,
    pub college_name: Option<String>,
    pub password: String,
    pub role: Option<String>,
}

pub async fn create_admin_user(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Json(payload): Json<CreateAdminUserRequest>,
) -> Result<impl IntoResponse> {
    auth.require_role(ROLE_SUPER_ADMIN)?;

    let username = payload.username.trim();
    if username.chars().count() < 3 || username.chars().count() > 30 {
        return Err(AppError::BadRequest(
            "Username must be 3-30 characters".to_string(),
        ));
    }
    if username.eq_ignore_ascii_case(crate::constants::CANARY_ADMIN_USERNAME) {
        return Err(AppError::BadRequest("Username is reserved".to_string()));
    }
    let email = normalize_email(&payload.email);
    if !crate::middleware::validators::is_valid_email(&email) {
        return Err(AppError::BadRequest("Valid email required".to_string()));
    }
    check_password(&payload.password, "Password")?;
    let full_name = clean_text(payload.full_name.as_deref(), "Full name")?;
    let college_name = clean_text(payload.college_name.as_deref(), "College name")?;
    let role = payload.role.as_deref().unwrap_or(ROLE_ADMIN);
    if !is_known_role(role) {
        return Err(AppError::BadRequest("Invalid role".to_string()));
    }

    ensure_identity_free(&state.db, Some(username), Some(&email), None).await?;

    let hashed_password = Admin::hash_password(&payload.password)?;

    let admin: Admin = sqlx::query_as(
        "INSERT INTO admins (id, username, email, password, role, full_name, college_name, is_active, failed_login_attempts, created_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, true, 0, now()) \
         RETURNING *",
    )
    .bind(Uuid::new_v4())
    .bind(username)
    .bind(&email)
    .bind(&hashed_password)
    .bind(role)
    .bind(&full_name)
    .bind(&college_name)
    .fetch_one(&state.db)
    .await
    .map_err(map_identity_conflict)?;

    if let Err(e) = record_audit_event(
        &state.db,
        Some(auth.id),
        "admin_user_created",
        serde_json::json!({ "created_admin_id": admin.id, "username": admin.username, "role": admin.role }),
        None,
    )
    .await
    {
        tracing::error!(error = %e, "Failed to record audit log entry for admin_user_created");
    }

    Ok((
        axum::http::StatusCode::CREATED,
        Json(AdminUserResponse::from(admin)),
    ))
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAdminUserRequest {
    /// Omitted keeps the stored value; `null` or an empty string clears it.
    #[serde(default, deserialize_with = "patch_text")]
    pub full_name: Option<Option<String>>,
    pub email: Option<String>,
    /// Same semantics as `full_name`.
    #[serde(default, deserialize_with = "patch_text")]
    pub college_name: Option<Option<String>>,
    pub role: Option<String>,
    pub is_active: Option<bool>,
    pub new_password: Option<String>,
}

pub async fn update_admin_user(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateAdminUserRequest>,
) -> Result<impl IntoResponse> {
    auth.require_role(ROLE_SUPER_ADMIN)?;

    let target_id = Uuid::parse_str(&id)
        .map_err(|e| AppError::BadRequest(format!("Invalid admin ID: {}", e)))?;

    // A super-admin must never be able to lock themselves out — there is no
    // email-based password reset to undo it.
    if target_id == auth.id && (payload.role.is_some() || payload.is_active.is_some()) {
        return Err(AppError::BadRequest(
            "You cannot change your own role or active status".to_string(),
        ));
    }

    let mut admin: Admin = sqlx::query_as("SELECT * FROM admins WHERE id = $1 AND username <> $2")
        .bind(target_id)
        .bind(crate::constants::CANARY_ADMIN_USERNAME)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound("Admin not found".to_string()))?;

    if let Some(role) = &payload.role {
        if !is_known_role(role) {
            return Err(AppError::BadRequest("Invalid role".to_string()));
        }
    }
    let new_email = match &payload.email {
        Some(raw) => {
            let email = normalize_email(raw);
            if !crate::middleware::validators::is_valid_email(&email) {
                return Err(AppError::BadRequest("Valid email required".to_string()));
            }
            Some(email)
        }
        None => None,
    };
    if let Some(new_password) = &payload.new_password {
        check_password(new_password, "New password")?;
    }
    for (value, label) in [
        (payload.full_name.as_ref(), "Full name"),
        (payload.college_name.as_ref(), "College name"),
    ] {
        clean_text(value.and_then(|v| v.as_deref()), label)?;
    }
    if let Some(email) = &new_email {
        if !email.eq_ignore_ascii_case(&admin.email) {
            ensure_identity_free(&state.db, None, Some(email), Some(target_id)).await?;
        }
    }

    let role_changed = payload.role.as_ref().is_some_and(|r| r != &admin.role);

    let full_name = apply_patch(payload.full_name, admin.full_name.take());
    let email = new_email.unwrap_or(admin.email.clone());
    let college_name = apply_patch(payload.college_name, admin.college_name.take());
    let role = payload.role.unwrap_or(admin.role.clone());
    let is_active = payload.is_active.unwrap_or(admin.is_active);
    let password_hash = match &payload.new_password {
        Some(p) => Some(Admin::hash_password(p)?),
        None => None,
    };

    let updated: Admin = sqlx::query_as(
        "UPDATE admins SET full_name = $1, email = $2, college_name = $3, role = $4, is_active = $5, \
             password = COALESCE($6, password) \
         WHERE id = $7 \
         RETURNING *",
    )
    .bind(&full_name)
    .bind(&email)
    .bind(&college_name)
    .bind(&role)
    .bind(is_active)
    .bind(&password_hash)
    .bind(target_id)
    .fetch_one(&state.db)
    .await
    .map_err(map_identity_conflict)?;

    if let Err(e) = record_audit_event(
        &state.db,
        Some(auth.id),
        if role_changed {
            "admin_role_changed"
        } else {
            "admin_user_updated"
        },
        serde_json::json!({ "target_admin_id": updated.id, "role": updated.role, "isActive": updated.is_active }),
        None,
    )
    .await
    {
        tracing::error!(error = %e, "Failed to record audit log entry for admin_user_updated");
    }

    Ok(Json(AdminUserResponse::from(updated)))
}

pub async fn delete_admin_user(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse> {
    auth.require_role(ROLE_SUPER_ADMIN)?;

    let target_id = Uuid::parse_str(&id)
        .map_err(|e| AppError::BadRequest(format!("Invalid admin ID: {}", e)))?;

    if target_id == auth.id {
        return Err(AppError::BadRequest(
            "You cannot delete your own account".to_string(),
        ));
    }

    let delete_result = sqlx::query("DELETE FROM admins WHERE id = $1 AND username <> $2")
        .bind(target_id)
        .bind(crate::constants::CANARY_ADMIN_USERNAME)
        .execute(&state.db)
        .await;

    let blocked_by_owned_data = matches!(
        &delete_result,
        Err(sqlx::Error::Database(db_err)) if db_err.is_foreign_key_violation()
    );

    if blocked_by_owned_data {
        // Every FK into admins(id) (sessions/batches/locations/short_links/
        // audit log/etc.) is intentionally left without an ON DELETE action,
        // so history keeps pointing at a real, named account instead of
        // going blank. Deactivating gets the same practical outcome as a
        // delete - is_active is checked on every authenticated request (see
        // middleware::auth), so the account is locked out immediately -
        // without destroying that attribution.
        let deactivated = sqlx::query("UPDATE admins SET is_active = false WHERE id = $1")
            .bind(target_id)
            .execute(&state.db)
            .await?;

        if deactivated.rows_affected() == 0 {
            return Err(AppError::NotFound("Admin not found".to_string()));
        }

        if let Err(e) = record_audit_event(
            &state.db,
            Some(auth.id),
            "admin_user_deactivated",
            serde_json::json!({ "target_admin_id": target_id, "reason": "owned_data_prevents_deletion" }),
            None,
        )
        .await
        {
            tracing::error!(error = %e, "Failed to record audit log entry for admin_user_deactivated");
        }

        return Ok(Json(serde_json::json!({
            "success": true,
            "deleted": false,
            "deactivated": true,
            "message": "This admin has owned records or activity history (sessions, batches, locations, or audit log) and can't be permanently deleted, so the account was deactivated instead. It can no longer log in or take any action."
        })));
    }

    let result = delete_result?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Admin not found".to_string()));
    }

    if let Err(e) = record_audit_event(
        &state.db,
        Some(auth.id),
        "admin_user_deleted",
        serde_json::json!({ "target_admin_id": target_id }),
        None,
    )
    .await
    {
        tracing::error!(error = %e, "Failed to record audit log entry for admin_user_deleted");
    }

    Ok(Json(serde_json::json!({
        "success": true,
        "deleted": true,
        "deactivated": false
    })))
}
