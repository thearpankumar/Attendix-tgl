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
use services::GpsHistoryService;
use std::sync::Arc;
use storage::Storage;

#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub db: mongodb::Client,
    pub db_name: String,
    pub redis: Option<redis::Client>,
    pub rate_limiter: Arc<RateLimiter>,
    pub session_cache: Arc<SessionCache>,
    pub gps_history: Arc<GpsHistoryService>,
    pub start_time: std::time::Instant,
    pub storage: Storage,
    pub http_client: reqwest::Client,
}

impl AppState {
    pub fn database(&self) -> mongodb::Database {
        self.db.database(&self.db_name)
    }

    pub fn is_redis_enabled(&self) -> bool {
        self.redis.is_some() && self.rate_limiter.is_redis_enabled()
    }
}
