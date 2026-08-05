pub mod config;
pub mod constants;
pub mod controllers;
pub mod error;
pub mod middleware;
pub mod models;
pub mod routes;
pub mod services;
pub mod storage;
pub mod utils;

pub use config::AppConfig;
pub use constants::*;
pub use error::AppError;

use middleware::{DenyList, RateLimiter, SessionCache};
use models::SystemConfig;
use services::GpsHistoryService;
use std::sync::Arc;
use storage::Storage;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub db: sqlx::PgPool,
    pub redis: Option<redis::Client>,
    pub rate_limiter: Arc<RateLimiter>,
    /// Tripwire deny-list: IPs that touched a honeypot path or the canary
    /// admin account get tarpitted-then-blocked on every subsequent request
    /// (see middleware::deny_list_middleware) — a near-zero-false-positive
    /// signal, unlike the general rate limiters which bound legitimate
    /// traffic too.
    pub deny_list: Arc<DenyList>,
    pub session_cache: Arc<SessionCache>,
    pub gps_history: Arc<GpsHistoryService>,
    pub start_time: std::time::Instant,
    pub storage: Storage,
    pub http_client: reqwest::Client,
    /// Hot-reloadable system configuration (loaded from DB, updated on every save)
    pub system_config: Arc<RwLock<SystemConfig>>,
    /// WebAuthn relying-party instance. Owns challenge generation, signature
    /// verification, origin and rpId checks for every ceremony.
    pub webauthn: Arc<webauthn_rs::Webauthn>,
}

/// Namespace for deriving a stable WebAuthn user handle from a roll number.
/// Roll numbers are short and guessable, so the handle exposed to authenticators
/// is a UUIDv5 over this namespace rather than the roll number itself.
pub const WEBAUTHN_USER_HANDLE_NAMESPACE: uuid::Uuid =
    uuid::uuid!("6f1b0f8e-6c2a-4f5d-9b3e-2a7c1d4e8f90");

/// Derives the opaque, stable user handle for a roll number.
pub fn webauthn_user_handle(roll_number: &str) -> uuid::Uuid {
    uuid::Uuid::new_v5(
        &WEBAUTHN_USER_HANDLE_NAMESPACE,
        roll_number.to_uppercase().as_bytes(),
    )
}

/// Builds the relying-party instance from configuration.
pub fn build_webauthn(config: &AppConfig) -> anyhow::Result<webauthn_rs::Webauthn> {
    let origin = url::Url::parse(&config.webauthn.origin).map_err(|e| {
        anyhow::anyhow!(
            "WEBAUTHN_ORIGIN {:?} is not a valid URL: {e}",
            config.webauthn.origin
        )
    })?;

    webauthn_rs::WebauthnBuilder::new(&config.webauthn.rp_id, &origin)
        .map_err(|e| anyhow::anyhow!("Invalid WebAuthn relying party configuration: {e}"))?
        .rp_name(&config.webauthn.rp_name)
        .build()
        .map_err(|e| anyhow::anyhow!("Failed to build WebAuthn relying party: {e}"))
}

impl AppState {
    pub fn is_redis_enabled(&self) -> bool {
        self.redis.is_some() && self.rate_limiter.is_redis_enabled()
    }

    /// Read the current system config snapshot (fast, non-blocking read)
    pub async fn get_system_config(&self) -> SystemConfig {
        self.system_config.read().await.clone()
    }

    /// Update the in-memory system config cache
    pub async fn set_system_config(&self, config: SystemConfig) {
        *self.system_config.write().await = config;
    }
}
