use axum::{
    extract::{ConnectInfo, State},
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use std::sync::Arc;

use crate::AppState;

/// Determines the client IP to key rate limits on.
///
/// The socket peer address is the source of truth. `X-Forwarded-For` is only
/// consulted when the peer is itself a configured trusted proxy — otherwise
/// any client can mint a fresh rate-limit bucket per request just by varying
/// the header, which defeated the login brute-force limiter along with every
/// other limit in the system. The previous implementation trusted the header
/// unconditionally and fell back to the literal string `"unknown"`, putting
/// every header-less client into one shared bucket.
fn get_client_ip<T>(request: &Request<T>, trusted_proxies: &[ipnet::IpNet]) -> String {
    let peer = request
        .extensions()
        .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
        .map(|ConnectInfo(addr)| addr.ip());

    let peer_is_trusted = match peer {
        Some(ip) => trusted_proxies.iter().any(|net| net.contains(&ip)),
        // No ConnectInfo (e.g. in tests): treat as untrusted.
        None => false,
    };

    if peer_is_trusted {
        // Right-most entry that is not itself a trusted proxy is the closest
        // address the trusted hop actually observed. Walking from the right
        // prevents a client from prepending forged entries.
        if let Some(forwarded) = request.headers().get("x-forwarded-for") {
            if let Ok(forwarded_str) = forwarded.to_str() {
                for candidate in forwarded_str.rsplit(',') {
                    let candidate = candidate.trim();
                    if candidate.is_empty() {
                        continue;
                    }
                    match candidate.parse::<std::net::IpAddr>() {
                        Ok(ip) if !trusted_proxies.iter().any(|net| net.contains(&ip)) => {
                            return ip.to_string();
                        }
                        Ok(_) => continue,
                        Err(_) => break,
                    }
                }
            }
        }
    }

    match peer {
        Some(ip) => ip.to_string(),
        // Falling back to a constant would merge all such callers into one
        // bucket, so be explicit that this is a degraded path.
        None => "unknown-peer".to_string(),
    }
}

/// Return a 429 Too Many Requests response
fn rate_limit_exceeded_response(limit_type: &str) -> Response {
    let message = match limit_type {
        "login" => "Too many login attempts. Please try again later.",
        "admin" => "Too many requests. Please slow down.",
        "student" => "Too many requests from this device. Please try again later.",
        "registration" => "Too many registration attempts. Please try again later.",
        "shortlinkguess" => "Too many attempts. Please try again later.",
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
    let ip = get_client_ip(&request, &state.config.trusted_proxies);

    let config = state.get_system_config().await;
    let allowed = state
        .rate_limiter
        .login_rate_limit(
            &ip,
            config.rate_limits.login_max_requests,
            config.rate_limits.login_window_secs,
        )
        .await;

    if !allowed {
        return rate_limit_exceeded_response("login");
    }

    next.run(request).await
}

/// Rate limiting middleware specifically for `/register`.
///
/// A leaked/guessed ADMIN_SECRET is the only gate on self-registration
/// (see controllers/admin/auth.rs::register). It previously shared the
/// generic `login` bucket (20 req/min by default) with `/login`, which is
/// far too permissive for an action that should happen a handful of times
/// ever in a normal deployment. This is layered in addition to, not instead
/// of, `login_rate_limit_middleware`.
pub async fn registration_rate_limit_middleware(
    State(state): State<Arc<AppState>>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let ip = get_client_ip(&request, &state.config.trusted_proxies);

    let config = state.get_system_config().await;
    let allowed = state
        .rate_limiter
        .registration_rate_limit(
            &ip,
            config.rate_limits.registration_max_requests,
            config.rate_limits.registration_window_secs,
        )
        .await;

    if !allowed {
        return rate_limit_exceeded_response("registration");
    }

    next.run(request).await
}

/// Dedicated, tighter cap for short-link code resolution
/// (`GET /api/s/{shortCode}` and `.../session`), on top of the general
/// `student_rate_limit_middleware` already layered on the whole router. A
/// 10-char short code is much lower-entropy than the session token it
/// stands in for; this bounds how many distinct codes one IP can try.
pub async fn short_link_guess_rate_limit_middleware(
    State(state): State<Arc<AppState>>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let ip = get_client_ip(&request, &state.config.trusted_proxies);

    let allowed = state
        .rate_limiter
        .short_link_guess_rate_limit(
            &ip,
            crate::constants::SHORT_LINK_GUESS_MAX_ATTEMPTS,
            crate::constants::SHORT_LINK_GUESS_WINDOW_SECS,
        )
        .await;

    if !allowed {
        return rate_limit_exceeded_response("shortlinkguess");
    }

    next.run(request).await
}

/// Rate limiting middleware for admin routes
pub async fn admin_rate_limit_middleware(
    State(state): State<Arc<AppState>>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let ip = get_client_ip(&request, &state.config.trusted_proxies);

    let config = state.get_system_config().await;
    let allowed = state
        .rate_limiter
        .admin_rate_limit(
            &ip,
            config.rate_limits.admin_max_requests,
            config.rate_limits.admin_window_secs,
        )
        .await;

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
    let ip = get_client_ip(&request, &state.config.trusted_proxies);

    let config = state.get_system_config().await;
    let allowed = state
        .rate_limiter
        .student_rate_limit(
            &ip,
            config.rate_limits.student_max_requests,
            config.rate_limits.student_window_secs,
        )
        .await;

    if !allowed {
        return rate_limit_exceeded_response("student");
    }

    next.run(request).await
}

/// Rate limiting middleware for client log routes
pub async fn client_log_rate_limit_middleware(
    State(state): State<Arc<AppState>>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let ip = get_client_ip(&request, &state.config.trusted_proxies);

    let config = state.get_system_config().await;
    let allowed = state
        .rate_limiter
        .client_log_rate_limit(
            &ip,
            config.rate_limits.client_log_max_requests,
            config.rate_limits.client_log_window_secs,
        )
        .await;

    if !allowed {
        return rate_limit_exceeded_response("client_log");
    }

    next.run(request).await
}
