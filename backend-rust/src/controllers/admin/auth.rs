use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::IntoResponse,
    Extension,
};
use mongodb::Collection;
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

    if payload.admin_secret != state.config.admin_secret {
        return Err(AppError::Unauthorized("Invalid admin secret".to_string()));
    }

    let collection: Collection<Admin> = state
        .db
        .database(
            state
                .config
                .mongodb_uri
                .split('/')
                .next_back()
                .unwrap_or("default"),
        )
        .collection(Admin::collection_name());

    let existing = collection
        .find_one(mongodb::bson::doc! { "username": &payload.username })
        .await?;
    if existing.is_some() {
        return Err(AppError::BadRequest("Username already exists".to_string()));
    }

    let existing_email = collection
        .find_one(mongodb::bson::doc! { "email": &payload.email })
        .await?;
    if existing_email.is_some() {
        return Err(AppError::BadRequest("Email already exists".to_string()));
    }

    let hashed_password = Admin::hash_password(&payload.password)?;

    let admin = Admin {
        id: None,
        username: payload.username,
        email: payload.email,
        password: hashed_password,
        role: "admin".to_string(),
        failed_login_attempts: 0,
        lock_until: None,
        created_at: chrono::Utc::now(),
    };

    let result = collection.insert_one(&admin).await?;
    let admin_id = result
        .inserted_id
        .as_object_id()
        .ok_or_else(|| AppError::Internal("Failed to get inserted ID".to_string()))?;

    let token = generate_token(
        &admin_id,
        &state.config.jwt_secret,
        &state.config.jwt_expire,
    )?;

    Ok((
        StatusCode::CREATED,
        Json(LoginResponse {
            token,
            expires_in: state.config.jwt_expire.clone(),
            admin: AdminResponse {
                id: admin_id.to_hex(),
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

    let collection: Collection<Admin> = state
        .db
        .database(
            state
                .config
                .mongodb_uri
                .split('/')
                .next_back()
                .unwrap_or("default"),
        )
        .collection(Admin::collection_name());

    let admin = collection
        .find_one(mongodb::bson::doc! { "username": &payload.username })
        .await?
        .ok_or_else(|| AppError::Unauthorized("Invalid credentials".to_string()))?;

    if admin.is_locked() {
        return Err(AppError::Unauthorized(
            "Account is locked. Try again later.".to_string(),
        ));
    }

    if !admin.verify_password(&payload.password)? {
        let attempts = admin.failed_login_attempts + 1;
        let lock_until = if attempts >= Admin::MAX_LOGIN_ATTEMPTS {
            Some(chrono::Utc::now() + chrono::Duration::minutes(Admin::LOCK_TIME_MINUTES))
        } else {
            None
        };

        collection
            .update_one(
                mongodb::bson::doc! { "_id": admin.id },
                mongodb::bson::doc! { "$set": { "failedLoginAttempts": attempts, "lockUntil": lock_until.map(|dt| mongodb::bson::DateTime::from_millis(dt.timestamp_millis())) } },
            )
            .await?;

        return Err(AppError::Unauthorized("Invalid credentials".to_string()));
    }

    if admin.failed_login_attempts > 0 {
        collection
            .update_one(
                mongodb::bson::doc! { "_id": admin.id },
                mongodb::bson::doc! { "$set": { "failedLoginAttempts": 0, "lockUntil": null } },
            )
            .await?;
    }

    let admin_id = admin
        .id
        .ok_or_else(|| AppError::Internal("No admin ID".to_string()))?;

    if admin.should_rehash() {
        let new_hash = Admin::hash_password(&payload.password)?;
        collection
            .update_one(
                mongodb::bson::doc! { "_id": admin_id },
                mongodb::bson::doc! { "$set": { "password": new_hash } },
            )
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
            id: admin_id.to_hex(),
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
    let collection: Collection<Admin> = state
        .db
        .database(
            state
                .config
                .mongodb_uri
                .split('/')
                .next_back()
                .unwrap_or("default"),
        )
        .collection(Admin::collection_name());

    let admin = collection
        .find_one(mongodb::bson::doc! { "_id": auth.id })
        .await?
        .ok_or_else(|| AppError::NotFound("Admin not found".to_string()))?;

    Ok(Json(AdminResponse {
        id: auth.id.to_hex(),
        username: admin.username,
        email: admin.email,
        role: admin.role,
    }))
}
