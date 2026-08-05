use axum::{
    extract::{ConnectInfo, Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use rand::RngExt;
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::RwLock;

const DENY_LIST_PREFIX: &str = "denylist:";

/// How long a flagged IP is denied outright after touching a honeypot path or
/// the canary admin account. Generous on purpose — these are near-zero-
/// false-positive signals (no legitimate client ever has a reason to hit
/// either), so there's little cost to a long block.
pub const DENY_LIST_TTL_SECS: u64 = 24 * 60 * 60;

/// Same dual-backend shape as `RateLimiter`: Redis when configured (shared
/// across replicas/restarts), an in-memory map otherwise.
#[derive(Clone)]
pub struct DenyList {
    redis: Option<Arc<redis::Client>>,
    memory: Arc<RwLock<HashMap<String, Instant>>>,
}

impl DenyList {
    pub fn with_redis(redis: Option<Arc<redis::Client>>) -> Self {
        Self {
            redis,
            memory: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn add(&self, ip: &str, reason: &str) {
        tracing::error!(
            target: "security",
            event = "ip_denylisted",
            ip = %ip,
            reason = %reason,
            "IP added to deny-list"
        );

        if let Some(ref redis) = self.redis {
            if let Ok(mut conn) = redis.get_multiplexed_async_connection().await {
                use redis::AsyncCommands;
                let key = format!("{}{}", DENY_LIST_PREFIX, ip);
                let _: Result<(), _> = conn.set_ex(key, reason, DENY_LIST_TTL_SECS).await;
                return;
            }
        }

        let mut store = self.memory.write().await;
        store.insert(ip.to_string(), Instant::now());
    }

    pub async fn contains(&self, ip: &str) -> bool {
        if let Some(ref redis) = self.redis {
            if let Ok(mut conn) = redis.get_multiplexed_async_connection().await {
                use redis::AsyncCommands;
                let key = format!("{}{}", DENY_LIST_PREFIX, ip);
                return conn.exists::<_, bool>(key).await.unwrap_or(false);
            }
        }

        let store = self.memory.read().await;
        store
            .get(ip)
            .map(|inserted| inserted.elapsed() < Duration::from_secs(DENY_LIST_TTL_SECS))
            .unwrap_or(false)
    }
}

impl Default for DenyList {
    fn default() -> Self {
        Self::with_redis(None)
    }
}

/// Applied as the outermost layer of the whole app router. A denied IP gets
/// tarpitted — a randomized multi-second delay — before the 403, rather than
/// an instant rejection: it costs us nothing extra to hold one already-
/// malicious connection open, and it's real friction against a scanner or
/// credential-stuffing tool running many attempts concurrently.
pub async fn deny_list_middleware(
    State(state): State<Arc<crate::AppState>>,
    ConnectInfo(addr): ConnectInfo<std::net::SocketAddr>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let ip = addr.ip().to_string();

    if state.deny_list.contains(&ip).await {
        let jitter_ms = rand::rng().random_range(2000..5000);
        tokio::time::sleep(Duration::from_millis(jitter_ms)).await;
        return (StatusCode::FORBIDDEN, "Forbidden").into_response();
    }

    next.run(request).await
}
