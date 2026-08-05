//! Access control for the Prometheus scrape endpoint.
//!
//! `/metrics` was mounted on the root router with no authentication, so anyone
//! who could reach the service could read per-route request volumes, latency
//! histograms and error counts for the whole instance. Caddy proxies the
//! backend, so "not published to the host" was not sufficient protection.

use axum::{extract::State, http::Request, middleware::Next, response::Response};
use std::sync::Arc;

use crate::error::{AppError, Result};
use crate::AppState;

/// Allows a scrape when the caller presents `METRICS_TOKEN` as a bearer token,
/// or connects from a configured trusted proxy CIDR (the monitoring network).
///
/// If `METRICS_TOKEN` is unset, only the trusted-proxy path applies; if
/// `TRUSTED_PROXIES` is also empty, the endpoint is closed entirely. Both
/// failing closed is deliberate — an unreachable dashboard is a visible
/// problem, an open metrics endpoint is not.
pub async fn metrics_auth_middleware(
    State(state): State<Arc<AppState>>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Result<Response> {
    if is_trusted_peer(&request, &state) || has_valid_token(&request) {
        return Ok(next.run(request).await);
    }

    Err(AppError::Unauthorized(
        "Metrics access requires an authorised source".to_string(),
    ))
}

fn is_trusted_peer(request: &Request<axum::body::Body>, state: &AppState) -> bool {
    request
        .extensions()
        .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
        .map(|axum::extract::ConnectInfo(addr)| addr.ip())
        .is_some_and(|ip| {
            state
                .config
                .trusted_proxies
                .iter()
                .any(|net| net.contains(&ip))
        })
}

fn has_valid_token(request: &Request<axum::body::Body>) -> bool {
    let Ok(expected) = std::env::var("METRICS_TOKEN") else {
        return false;
    };
    if expected.is_empty() {
        return false;
    }

    let Some(presented) = request
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
    else {
        return false;
    };

    use subtle::ConstantTimeEq;
    presented.as_bytes().ct_eq(expected.as_bytes()).into()
}
