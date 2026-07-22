use axum::{
    extract::{Request, State},
    http::{header, Method},
    middleware::Next,
    response::Response,
    Json,
};
use rand::RngExt;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::warn;

use crate::{error::AppError, AppState};

const CSRF_HEADER: &str = "x-csrf-token";
const CSRF_COOKIE: &str = "csrf_token";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsrfTokenResponse {
    pub csrf_token: String,
    pub expires_in_seconds: u64,
}

pub fn generate_csrf_token() -> String {
    let mut rng = rand::rng();
    let token_bytes: [u8; 32] = rng.random();
    hex::encode(token_bytes)
}

pub fn validate_csrf_token(token: &str) -> bool {
    !token.is_empty() && token.len() == 64 && token.chars().all(|c| c.is_ascii_hexdigit())
}

pub async fn csrf_middleware(
    State(_state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Result<Response, AppError> {
    let method = request.method().clone();
    let path = request.uri().path().to_string();

    if should_skip_csrf(&method, &path) {
        return Ok(next.run(request).await);
    }

    let csrf_header = request
        .headers()
        .get(CSRF_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let csrf_cookie = request
        .headers()
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|cookies| {
            cookies.split(';').map(|s| s.trim()).find_map(|cookie| {
                let parts: Vec<&str> = cookie.splitn(2, '=').collect();
                if parts.len() == 2 && parts[0] == CSRF_COOKIE {
                    Some(parts[1].to_string())
                } else {
                    None
                }
            })
        });

    match (csrf_header, csrf_cookie) {
        (Some(header_token), Some(cookie_token)) => {
            if !validate_csrf_token(&header_token) || !validate_csrf_token(&cookie_token) {
                warn!(
                    "Invalid CSRF token format: header={}, cookie={}",
                    header_token.len(),
                    cookie_token.len()
                );
                return Err(AppError::Unauthorized("Invalid CSRF token".to_string()));
            }

            if header_token != cookie_token {
                warn!("CSRF token mismatch: header != cookie");
                return Err(AppError::Unauthorized("CSRF token mismatch".to_string()));
            }

            Ok(next.run(request).await)
        }
        _ => {
            warn!("Missing CSRF token: method={:?}, path={:?}", method, path);
            Err(AppError::Unauthorized("CSRF token required".to_string()))
        }
    }
}

fn should_skip_csrf(method: &Method, path: &str) -> bool {
    if method == Method::GET || method == Method::HEAD || method == Method::OPTIONS {
        return true;
    }

    let skip_paths = [
        "/api/admin/login",
        "/api/admin/register",
        "/api/logs/client",
        "/public/",
        "/_health",
        "/health",
        "/health/",
        "/metrics",
        "/api/attend",
        "/s/",
    ];

    skip_paths.iter().any(|skip| path.starts_with(skip))
        || path.starts_with("/api/admin/webauthn/")
        || path.starts_with("/s/")
        || path.contains("/webauthn/")
}

pub async fn get_csrf_token() -> Result<Json<CsrfTokenResponse>, AppError> {
    let csrf_token = generate_csrf_token();

    Ok(Json(CsrfTokenResponse {
        csrf_token,
        expires_in_seconds: 3600,
    }))
}
