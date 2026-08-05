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

    #[error("Too many requests: {0}")]
    TooManyRequests(String),
}

impl From<ValidationError> for AppError {
    fn from(err: ValidationError) -> Self {
        AppError::Validation(err.message)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        // Internal failures are logged in full but reported opaquely. These
        // variants previously returned `e.to_string()` straight to the client,
        // leaking table, column and constraint names from sqlx errors, Redis
        // and AWS diagnostics, and filesystem paths.
        const OPAQUE: &str = "An internal error occurred";

        let (status, message) = match &self {
            AppError::Database(e) => {
                tracing::error!(error = %e, "database error");
                (StatusCode::INTERNAL_SERVER_ERROR, OPAQUE.to_string())
            }
            AppError::Redis(e) => {
                tracing::error!(error = %e, "redis error");
                (StatusCode::INTERNAL_SERVER_ERROR, OPAQUE.to_string())
            }
            AppError::Io(e) => {
                tracing::error!(error = %e, "io error");
                (StatusCode::INTERNAL_SERVER_ERROR, OPAQUE.to_string())
            }
            AppError::Internal(msg) => {
                tracing::error!(error = %msg, "internal error");
                (StatusCode::INTERNAL_SERVER_ERROR, OPAQUE.to_string())
            }
            AppError::Aws(msg) => {
                tracing::error!(error = %msg, "aws error");
                (StatusCode::INTERNAL_SERVER_ERROR, OPAQUE.to_string())
            }
            AppError::Image(msg) => {
                tracing::error!(error = %msg, "image error");
                (StatusCode::INTERNAL_SERVER_ERROR, OPAQUE.to_string())
            }
            AppError::Storage(msg) => {
                tracing::error!(error = %msg, "storage error");
                (StatusCode::INTERNAL_SERVER_ERROR, OPAQUE.to_string())
            }
            // Client-facing variants carry messages written for the client.
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            AppError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg.clone()),
            AppError::Forbidden(msg) => (StatusCode::FORBIDDEN, msg.clone()),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            AppError::Validation(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            AppError::TooManyRequests(msg) => (StatusCode::TOO_MANY_REQUESTS, msg.clone()),
            AppError::Jwt(_) => (
                StatusCode::UNAUTHORIZED,
                "Invalid or expired token".to_string(),
            ),
        };

        let body = Json(json!({
            "message": message,
        }));

        (status, body).into_response()
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
