use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::IntoResponse,
    Extension,
};
use serde::Serialize;
use std::sync::Arc;

use crate::{
    error::{AppError, Result},
    middleware::{
        generate_token,
        validators::{validate_request, AdminLoginRequest, AdminRegisterRequest},
        AuthenticatedAdmin,
    },
    models::{Admin, AdminLogin, AdminRegistration},
};

#[derive(Debug, Serialize)]
pub struct AdminResponse {
    #[serde(rename = "_id")]
    pub id: String,
    pub username: String,
    pub email: String,
    pub role: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub expires_in: String,
    pub admin: AdminResponse,
}

pub async fn register(
    State(state): State<Arc<crate::AppState>>,
    Json(payload): Json<AdminRegistration>,
) -> Result<impl IntoResponse> {
    let validation_req = AdminRegisterRequest {
        username: payload.username.clone(),
        email: payload.email.clone(),
        password: payload.password.clone(),
        admin_secret: payload.admin_secret.clone(),
    };
    validate_request(&validation_req)?;

    // Constant-time: a byte-wise `!=` leaks the correct prefix through timing,
    // and this endpoint is public.
    use subtle::ConstantTimeEq;
    let secret_matches: bool = payload
        .admin_secret
        .as_bytes()
        .ct_eq(state.config.admin_secret.as_bytes())
        .into();
    if !secret_matches {
        return Err(AppError::Unauthorized("Invalid admin secret".to_string()));
    }

    let existing: Option<Admin> = sqlx::query_as("SELECT * FROM admins WHERE username = $1")
        .bind(&payload.username)
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
        "INSERT INTO admins (id, username, email, password, role, failed_login_attempts, created_at) \
         VALUES ($1, $2, $3, $4, $5, 0, now()) \
         RETURNING *",
    )
    .bind(uuid::Uuid::new_v4())
    .bind(&payload.username)
    .bind(&payload.email)
    .bind(&hashed_password)
    .bind("admin")
    .fetch_one(&state.db)
    .await?;

    let token = generate_token(
        &admin.id,
        &state.config.jwt_secret,
        &state.config.jwt_expire,
    )?;

    Ok((
        StatusCode::CREATED,
        Json(LoginResponse {
            token,
            expires_in: state.config.jwt_expire.clone(),
            admin: AdminResponse {
                id: admin.id.to_string(),
                username: admin.username,
                email: admin.email,
                role: admin.role,
            },
        }),
    ))
}

pub async fn login(
    State(state): State<Arc<crate::AppState>>,
    Json(payload): Json<AdminLogin>,
) -> Result<impl IntoResponse> {
    let validation_req = AdminLoginRequest {
        username: payload.username.clone(),
        password: payload.password.clone(),
    };
    validate_request(&validation_req)?;

    let admin: Admin = sqlx::query_as("SELECT * FROM admins WHERE username = $1")
        .bind(&payload.username)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::Unauthorized("Invalid credentials".to_string()))?;

    if admin.is_locked() {
        return Err(AppError::Unauthorized(
            "Account is locked. Try again later.".to_string(),
        ));
    }

    if !admin.verify_password(&payload.password)? {
        let sys_config = state.get_system_config().await;
        let max_attempts = sys_config.lockout_config.max_login_attempts as i32;
        let lock_duration = sys_config.lockout_config.lockout_duration_minutes as i64;
        let attempts = admin.failed_login_attempts + 1;
        let lock_until = if attempts >= max_attempts {
            Some(chrono::Utc::now() + chrono::Duration::minutes(lock_duration))
        } else {
            None
        };

        sqlx::query("UPDATE admins SET failed_login_attempts = $1, lock_until = $2 WHERE id = $3")
            .bind(attempts)
            .bind(lock_until)
            .bind(admin.id)
            .execute(&state.db)
            .await?;

        return Err(AppError::Unauthorized("Invalid credentials".to_string()));
    }

    if admin.failed_login_attempts > 0 {
        sqlx::query("UPDATE admins SET failed_login_attempts = 0, lock_until = NULL WHERE id = $1")
            .bind(admin.id)
            .execute(&state.db)
            .await?;
    }

    let admin_id = admin.id;

    if admin.should_rehash() {
        let new_hash = Admin::hash_password(&payload.password)?;
        sqlx::query("UPDATE admins SET password = $1 WHERE id = $2")
            .bind(&new_hash)
            .bind(admin_id)
            .execute(&state.db)
            .await?;
    }
    let token = generate_token(
        &admin_id,
        &state.config.jwt_secret,
        &state.config.jwt_expire,
    )?;

    Ok(Json(LoginResponse {
        token,
        expires_in: state.config.jwt_expire.clone(),
        admin: AdminResponse {
            id: admin_id.to_string(),
            username: admin.username,
            email: admin.email,
            role: admin.role,
        },
    }))
}

pub async fn get_profile(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
) -> Result<impl IntoResponse> {
    let admin: Admin = sqlx::query_as("SELECT * FROM admins WHERE id = $1")
        .bind(auth.id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound("Admin not found".to_string()))?;

    Ok(Json(AdminResponse {
        id: auth.id.to_string(),
        username: admin.username,
        email: admin.email,
        role: admin.role,
    }))
}
