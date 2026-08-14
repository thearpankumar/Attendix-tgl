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
    constants::{ROLE_ADMIN, ROLE_SUPER_ADMIN},
    error::{AppError, Result},
    middleware::AuthenticatedAdmin,
    models::{record_audit_event, Admin},
};

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
    if username.len() < 3 || username.len() > 30 {
        return Err(AppError::BadRequest(
            "Username must be 3-30 characters".to_string(),
        ));
    }
    if username.eq_ignore_ascii_case(crate::constants::CANARY_ADMIN_USERNAME) {
        return Err(AppError::BadRequest("Username is reserved".to_string()));
    }
    if !crate::middleware::validators::is_valid_email(&payload.email) {
        return Err(AppError::BadRequest("Valid email required".to_string()));
    }
    if payload.password.len() < 6 {
        return Err(AppError::BadRequest(
            "Password must be at least 6 characters".to_string(),
        ));
    }
    let role = payload.role.as_deref().unwrap_or(ROLE_ADMIN);
    if !is_known_role(role) {
        return Err(AppError::BadRequest("Invalid role".to_string()));
    }

    let existing: Option<Admin> = sqlx::query_as("SELECT * FROM admins WHERE username = $1")
        .bind(username)
        .fetch_optional(&state.db)
        .await?;
    if existing.is_some() {
        return Err(AppError::BadRequest("Username already exists".to_string()));
    }
    let existing_email: Option<Admin> = sqlx::query_as("SELECT * FROM admins WHERE email = $1")
        .bind(&payload.email)
        .fetch_optional(&state.db)
        .await?;
    if existing_email.is_some() {
        return Err(AppError::BadRequest("Email already exists".to_string()));
    }

    let hashed_password = Admin::hash_password(&payload.password)?;

    let admin: Admin = sqlx::query_as(
        "INSERT INTO admins (id, username, email, password, role, full_name, college_name, is_active, failed_login_attempts, created_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, true, 0, now()) \
         RETURNING *",
    )
    .bind(Uuid::new_v4())
    .bind(username)
    .bind(&payload.email)
    .bind(&hashed_password)
    .bind(role)
    .bind(&payload.full_name)
    .bind(&payload.college_name)
    .fetch_one(&state.db)
    .await?;

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
    pub full_name: Option<String>,
    pub email: Option<String>,
    pub college_name: Option<String>,
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
    if let Some(email) = &payload.email {
        if !crate::middleware::validators::is_valid_email(email) {
            return Err(AppError::BadRequest("Valid email required".to_string()));
        }
    }
    if let Some(new_password) = &payload.new_password {
        if new_password.len() < 6 {
            return Err(AppError::BadRequest(
                "New password must be at least 6 characters".to_string(),
            ));
        }
    }

    let role_changed = payload.role.as_ref().is_some_and(|r| r != &admin.role);

    let full_name = payload.full_name.or(admin.full_name.take());
    let email = payload.email.unwrap_or(admin.email.clone());
    let college_name = payload.college_name.or(admin.college_name.take());
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
    .await?;

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

    let result = sqlx::query("DELETE FROM admins WHERE id = $1 AND username <> $2")
        .bind(target_id)
        .bind(crate::constants::CANARY_ADMIN_USERNAME)
        .execute(&state.db)
        .await
        .map_err(|e| {
            // Foreign-key violation: this admin still owns locations/batches/
            // sessions/short_links (created_by has no ON DELETE behavior).
            // Surface a clear, actionable message instead of a raw DB error.
            if let sqlx::Error::Database(db_err) = &e {
                if db_err.is_foreign_key_violation() {
                    return AppError::BadRequest(
                        "This admin still owns locations, batches, or sessions and cannot be deleted. Reassign or remove those first.".to_string(),
                    );
                }
            }
            AppError::from(e)
        })?;

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

    Ok(Json(serde_json::json!({ "success": true })))
}
