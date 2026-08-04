use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

use crate::middleware::validators::ValidationError;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Redis error: {0}")]
    Redis(#[from] redis::RedisError),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("BadRequest: {0}")]
    BadRequest(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("JWT error: {0}")]
    Jwt(#[from] jsonwebtoken::errors::Error),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("AWS SDK error: {0}")]
    Aws(String),

    #[error("Image processing error: {0}")]
    Image(String),

    #[error("Storage error: {0}")]
    Storage(String),
}

impl From<ValidationError> for AppError {
    fn from(err: ValidationError) -> Self {
        AppError::Validation(err.message)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::Database(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
            AppError::Redis(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            AppError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg.clone()),
            AppError::Forbidden(msg) => (StatusCode::FORBIDDEN, msg.clone()),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            AppError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
            AppError::Jwt(e) => (StatusCode::UNAUTHORIZED, e.to_string()),
            AppError::Validation(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            AppError::Io(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
            AppError::Aws(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
            AppError::Image(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
            AppError::Storage(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
        };

        let body = Json(json!({
            "message": message,
        }));

        (status, body).into_response()
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
