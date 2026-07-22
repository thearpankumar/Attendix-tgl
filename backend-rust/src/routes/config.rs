use axum::{
    extract::State,
    routing::{get, post},
    Extension, Json, Router,
};
use mongodb::{bson::doc, Collection};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::error::{AppError, Result};
use crate::middleware::AuthenticatedAdmin;
use crate::models::SystemConfig;
use crate::AppState;

pub fn create_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(get_config).post(update_config))
        .route("/dev-bypass", post(toggle_dev_bypass))
}

async fn get_config(
    State(state): State<Arc<AppState>>,
    Extension(_admin): Extension<AuthenticatedAdmin>,
) -> Result<impl axum::response::IntoResponse> {
    let db_name = state
        .config
        .mongodb_uri
        .split('/')
        .next_back()
        .unwrap_or("default");

    let collection: Collection<SystemConfig> = state
        .db
        .database(db_name)
        .collection(SystemConfig::collection_name());

    let config = collection
        .find_one(doc! {})
        .await?
        .unwrap_or_else(SystemConfig::default);

    Ok(Json(config))
}

async fn update_config(
    State(state): State<Arc<AppState>>,
    Extension(_admin): Extension<AuthenticatedAdmin>,
    Json(payload): Json<SystemConfig>,
) -> Result<impl axum::response::IntoResponse> {
    let db_name = state
        .config
        .mongodb_uri
        .split('/')
        .next_back()
        .unwrap_or("default");

    let configs: Collection<SystemConfig> = state
        .db
        .database(db_name)
        .collection(SystemConfig::collection_name());

    configs
        .update_one(
            doc! {},
            doc! { "$set": mongodb::bson::to_document(&payload).map_err(|e| AppError::Internal(e.to_string()))? },
        )
        .upsert(true)
        .await?;

    Ok(Json(payload))
}

#[derive(Debug, Deserialize)]
struct DevBypassRequest {
    enabled: bool,
    password: String,
}

#[derive(Debug, Serialize)]
struct DevBypassResponse {
    message: String,
    config: SystemConfig,
}

async fn toggle_dev_bypass(
    State(state): State<Arc<AppState>>,
    Extension(auth_admin): Extension<AuthenticatedAdmin>,
    Json(payload): Json<DevBypassRequest>,
) -> Result<impl axum::response::IntoResponse> {
    let db_name = state
        .config
        .mongodb_uri
        .split('/')
        .next_back()
        .unwrap_or("default");

    let admins: Collection<crate::models::Admin> = state
        .db
        .database(db_name)
        .collection(crate::models::Admin::collection_name());

    let admin = admins
        .find_one(doc! { "_id": auth_admin.id })
        .await?
        .ok_or_else(|| AppError::Unauthorized("Admin not found".to_string()))?;

    if !admin.verify_password(&payload.password)? {
        return Err(AppError::Unauthorized("Invalid password".to_string()));
    }

    let configs: Collection<SystemConfig> = state
        .db
        .database(db_name)
        .collection(SystemConfig::collection_name());

    let config = configs
        .find_one(doc! {})
        .await?
        .unwrap_or_else(SystemConfig::default);

    configs
        .update_one(
            doc! {},
            doc! {
                "$set": {
                    "devBypassEnabled": payload.enabled,
                    "updatedBy": auth_admin.id,
                }
            },
        )
        .upsert(true)
        .await?;

    let admin_id = admin
        .id
        .ok_or_else(|| AppError::Internal("Admin has no ID".to_string()))?;
    let updated_config = SystemConfig {
        dev_bypass_enabled: payload.enabled,
        updated_by: Some(admin_id),
        ..config
    };

    Ok(Json(DevBypassResponse {
        message: "Developer Bypass Mode updated successfully".to_string(),
        config: updated_config,
    }))
}
