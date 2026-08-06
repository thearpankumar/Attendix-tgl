use axum::{
    extract::{ConnectInfo, Extension, Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use rand::RngExt;
use std::{sync::Arc, time::Duration};

const DENY_LIST_PREFIX: &str = "denylist:";

/// How long a flagged IP is denied outright after touching a honeypot path or
/// the canary admin account. Generous on purpose — these are near-zero-
/// false-positive signals (no legitimate client ever has a reason to hit
/// either), so there's little cost to a long block.
pub const DENY_LIST_TTL_SECS: u64 = 24 * 60 * 60;

#[derive(Clone)]
pub struct DenyList {
    redis: Arc<redis::Client>,
}

impl DenyList {
    pub fn new(redis: Arc<redis::Client>) -> Self {
        Self { redis }
    }

    pub async fn add(&self, ip: &str, reason: &str) {
        tracing::error!(
            target: "security",
            event = "ip_denylisted",
            ip = %ip,
            reason = %reason,
            "IP added to deny-list"
        );

        match self.redis.get_multiplexed_async_connection().await {
            Ok(mut conn) => {
                use redis::AsyncCommands;
                let key = format!("{}{}", DENY_LIST_PREFIX, ip);
                if let Err(e) = conn
                    .set_ex::<_, _, ()>(key, reason, DENY_LIST_TTL_SECS)
                    .await
                {
                    tracing::error!("Failed to write deny-list entry to Redis: {}", e);
                }
            }
            Err(e) => tracing::error!("Failed to connect to Redis for deny-list add: {}", e),
        }
    }

    /// Fails open (not-denied) on a Redis error: this is checked on every
    /// request via `deny_list_middleware`, so an outage here must not turn
    /// into a full-site outage — the deny-list is a defense-in-depth signal,
    /// not the primary access control.
    pub async fn contains(&self, ip: &str) -> bool {
        match self.redis.get_multiplexed_async_connection().await {
            Ok(mut conn) => {
                use redis::AsyncCommands;
                let key = format!("{}{}", DENY_LIST_PREFIX, ip);
                match conn.exists::<_, bool>(key).await {
                    Ok(denied) => denied,
                    Err(e) => {
                        tracing::error!("Deny-list Redis lookup failed: {}", e);
                        false
                    }
                }
            }
            Err(e) => {
                tracing::error!("Failed to connect to Redis for deny-list check: {}", e);
                false
            }
        }
    }
}

/// Applied as the outermost layer of the whole app router. A denied IP gets
/// tarpitted — a randomized multi-second delay — before the 403, rather than
/// an instant rejection: it costs us nothing extra to hold one already-
/// malicious connection open, and it's real friction against a scanner or
/// credential-stuffing tool running many attempts concurrently.
pub async fn deny_list_middleware(
    State(state): State<Arc<crate::AppState>>,
    // `Option<Extension<ConnectInfo<_>>>`, not a bare `ConnectInfo<SocketAddr>`:
    // the latter 500s outright when the extension isn't present (e.g. router
    // tests that call `.oneshot()` directly without
    // `into_make_service_with_connect_info`), which — since this middleware
    // wraps literally every route — took down every test in the suite, not
    // just ones that happen to touch IP-based logic. `ConnectInfo` itself
    // doesn't implement `OptionalFromRequestParts` in this axum version, so
    // `Option<ConnectInfo<_>>` doesn't compile; `Extension<T>` does, and
    // `ConnectInfo` is stored as an `Extension` internally, so this is
    // equivalent. Missing ConnectInfo degrades to "unknown-peer" instead,
    // the same fallback `rate_limit_middleware::get_client_ip` already uses.
    connect_info: Option<Extension<ConnectInfo<std::net::SocketAddr>>>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let ip = connect_info
        .map(|Extension(ConnectInfo(addr))| addr.ip().to_string())
        .unwrap_or_else(|| "unknown-peer".to_string());

    if state.deny_list.contains(&ip).await {
        let jitter_ms = rand::rng().random_range(2000..5000);
        tokio::time::sleep(Duration::from_millis(jitter_ms)).await;
        return (StatusCode::FORBIDDEN, "Forbidden").into_response();
    }

    next.run(request).await
}
