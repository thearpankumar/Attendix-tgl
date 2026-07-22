use axum::{
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use std::sync::Arc;

use crate::AppState;

/// Extract client IP from request headers (x-forwarded-for or x-real-ip)
fn get_client_ip<T>(request: &Request<T>) -> String {
    // Try x-forwarded-for header first (may contain multiple IPs, take the first one)
    if let Some(forwarded) = request.headers().get("x-forwarded-for") {
        if let Ok(forwarded_str) = forwarded.to_str() {
            if let Some(first_ip) = forwarded_str.split(',').next() {
                let ip = first_ip.trim();
                if !ip.is_empty() {
                    return ip.to_string();
                }
            }
        }
    }

    // Fall back to x-real-ip header
    if let Some(real_ip) = request.headers().get("x-real-ip") {
        if let Ok(ip_str) = real_ip.to_str() {
            let ip = ip_str.trim();
            if !ip.is_empty() {
                return ip.to_string();
            }
        }
    }

    // Default to unknown if no headers present
    "unknown".to_string()
}

/// Return a 429 Too Many Requests response
fn rate_limit_exceeded_response(limit_type: &str) -> Response {
    let message = match limit_type {
        "login" => "Too many login attempts. Please try again later.",
        "admin" => "Too many requests. Please slow down.",
        "student" => "Too many requests from this device. Please try again later.",
        _ => "Rate limit exceeded. Please try again later.",
    };

    (
        StatusCode::TOO_MANY_REQUESTS,
        Json(json!({
            "success": false,
            "error": message,
        })),
    )
        .into_response()
}

/// Rate limiting middleware for login/registration routes
pub async fn login_rate_limit_middleware(
    State(state): State<Arc<AppState>>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let ip = get_client_ip(&request);

    let allowed = state.rate_limiter.login_rate_limit(&ip).await;

    if !allowed {
        return rate_limit_exceeded_response("login");
    }

    next.run(request).await
}

/// Rate limiting middleware for admin routes
pub async fn admin_rate_limit_middleware(
    State(state): State<Arc<AppState>>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let ip = get_client_ip(&request);

    let allowed = state.rate_limiter.admin_rate_limit(&ip).await;

    if !allowed {
        return rate_limit_exceeded_response("admin");
    }

    next.run(request).await
}

/// Rate limiting middleware for student routes
pub async fn student_rate_limit_middleware(
    State(state): State<Arc<AppState>>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let ip = get_client_ip(&request);

    let allowed = state.rate_limiter.student_rate_limit(&ip).await;

    if !allowed {
        return rate_limit_exceeded_response("student");
    }

    next.run(request).await
}
