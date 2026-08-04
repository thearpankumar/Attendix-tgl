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

use middleware::{RateLimiter, SessionCache};
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
    pub session_cache: Arc<SessionCache>,
    pub gps_history: Arc<GpsHistoryService>,
    pub start_time: std::time::Instant,
    pub storage: Storage,
    pub http_client: reqwest::Client,
    /// Hot-reloadable system configuration (loaded from DB, updated on every save)
    pub system_config: Arc<RwLock<SystemConfig>>,
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
