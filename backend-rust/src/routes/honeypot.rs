use axum::{
    extract::{ConnectInfo, State},
    http::StatusCode,
    routing::{any, get},
    Router,
};
use std::sync::Arc;

use crate::AppState;

/// Decoy paths that mimic common CMS/admin-panel/secret-file targets. No
/// legitimate client of this system ever requests any of these — every
/// occupant of this list was picked because it's a top hit in real internet
/// scanner traffic (WordPress admin, dotenv files, phpMyAdmin, a plausible-
/// sounding backup-download endpoint). A hit is a near-zero-false-positive
/// signal of reconnaissance or an automated exploit scanner, so it's dealt
/// with far more aggressively than a normal failed request: the source IP
/// goes straight on the deny-list (see middleware::deny_list).
pub fn create_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/wp-admin", any(hit))
        .route("/wp-admin/{*rest}", any(hit))
        .route("/wp-login.php", any(hit))
        .route("/.env", any(hit))
        .route("/.env.bak", any(hit))
        .route("/phpmyadmin", any(hit))
        .route("/phpmyadmin/{*rest}", any(hit))
        .route("/api/admin/backup-download", any(hit))
        .route("/.git/config", any(hit))
        .route("/xmlrpc.php", get(hit))
}

async fn hit(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<std::net::SocketAddr>,
    uri: axum::http::Uri,
) -> StatusCode {
    let ip = addr.ip().to_string();
    state
        .deny_list
        .add(&ip, &format!("honeypot path: {}", uri.path()))
        .await;

    // A plain 404 — same as any other unrouted path — rather than anything
    // that would let an attacker distinguish "hit a tripwire" from "typo'd a
    // URL" before the deny-list takes effect on their next request.
    StatusCode::NOT_FOUND
}
