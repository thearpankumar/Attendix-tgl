use redis::AsyncCommands;
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::RwLock;

const RATE_LIMIT_PREFIX: &str = "rl:";

// Rate limit constants for admin endpoints
const RATE_LIMIT_ADMIN_WINDOW_SECS: u64 = 900;
const RATE_LIMIT_ADMIN_MAX: u32 = 100;

// Rate limit constants for student endpoints
const RATE_LIMIT_STUDENT_WINDOW_SECS: u64 = 60;
const RATE_LIMIT_STUDENT_MAX: u32 = 20;

// Rate limit constants for login endpoint
const RATE_LIMIT_LOGIN_WINDOW_SECS: u64 = 900;
const RATE_LIMIT_LOGIN_MAX: u32 = 5;

// Rate limit constants for registration endpoint
const RATE_LIMIT_REGISTRATION_WINDOW_SECS: u64 = 900;
const RATE_LIMIT_REGISTRATION_MAX: u32 = 5;

// Rate limit constants for client log endpoint
const RATE_LIMIT_CLIENTLOG_WINDOW_SECS: u64 = 60;
const RATE_LIMIT_CLIENTLOG_MAX: u32 = 10;

#[derive(Clone)]
pub struct RateLimitConfig {
    pub window_secs: u64,
    pub max_requests: u32,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            window_secs: RATE_LIMIT_ADMIN_WINDOW_SECS,
            max_requests: RATE_LIMIT_ADMIN_MAX,
        }
    }
}

impl RateLimitConfig {
    pub fn new(window_secs: u64, max_requests: u32) -> Self {
        Self {
            window_secs,
            max_requests,
        }
    }
}

#[derive(Clone)]
pub struct RateLimiter {
    redis: Option<Arc<redis::Client>>,
    memory_store: Arc<RwLock<HashMap<String, RateEntry>>>,
    configs: HashMap<String, RateLimitConfig>,
    use_redis: bool,
}

#[derive(Clone)]
struct RateEntry {
    count: u32,
    window_start: Instant,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self::with_redis(None)
    }

    pub fn with_redis(redis: Option<Arc<redis::Client>>) -> Self {
        let mut configs = HashMap::new();
        configs.insert(
            "admin".to_string(),
            RateLimitConfig::new(RATE_LIMIT_ADMIN_WINDOW_SECS, RATE_LIMIT_ADMIN_MAX),
        );
        configs.insert(
            "student".to_string(),
            RateLimitConfig::new(RATE_LIMIT_STUDENT_WINDOW_SECS, RATE_LIMIT_STUDENT_MAX),
        );
        configs.insert(
            "login".to_string(),
            RateLimitConfig::new(RATE_LIMIT_LOGIN_WINDOW_SECS, RATE_LIMIT_LOGIN_MAX),
        );
        configs.insert(
            "registration".to_string(),
            RateLimitConfig::new(
                RATE_LIMIT_REGISTRATION_WINDOW_SECS,
                RATE_LIMIT_REGISTRATION_MAX,
            ),
        );
        configs.insert(
            "clientlog".to_string(),
            RateLimitConfig::new(RATE_LIMIT_CLIENTLOG_WINDOW_SECS, RATE_LIMIT_CLIENTLOG_MAX),
        );

        Self {
            use_redis: redis.is_some(),
            redis,
            memory_store: Arc::new(RwLock::new(HashMap::new())),
            configs,
        }
    }

    pub async fn check_rate_limit(&self, key: &str, limit_type: &str) -> bool {
        // Skip rate limiting in test environment
        if std::env::var("NODE_ENV").unwrap_or_default() == "test" {
            return true;
        }

        let config = self.configs.get(limit_type).cloned().unwrap_or_default();

        if self.use_redis {
            match self.check_with_redis(key, &config).await {
                Ok(result) => result,
                Err(e) => {
                    tracing::warn!(
                        "Redis rate limit check failed: {}, falling back to memory",
                        e
                    );
                    self.check_with_config(key, &config).await
                }
            }
        } else {
            self.check_with_config(key, &config).await
        }
    }

    async fn check_with_redis(&self, key: &str, config: &RateLimitConfig) -> Result<bool, String> {
        if let Some(ref redis_client) = self.redis {
            let mut conn = redis_client
                .get_multiplexed_async_connection()
                .await
                .map_err(|e| format!("Connection failed: {}", e))?;

            let redis_key = format!("{}{}:{}", RATE_LIMIT_PREFIX, limit_type_from_key(key), key);

            let count: i64 = conn
                .incr(&redis_key, 1)
                .await
                .map_err(|e| format!("INCR failed: {}", e))?;

            if count == 1 {
                let _: () = conn
                    .expire(&redis_key, config.window_secs as i64)
                    .await
                    .map_err(|e| format!("EXPIRE failed: {}", e))?;
            }

            Ok(count <= config.max_requests as i64)
        } else {
            Ok(self.check_with_config(key, config).await)
        }
    }

    async fn check_with_config(&self, key: &str, config: &RateLimitConfig) -> bool {
        let mut store = self.memory_store.write().await;

        if store.len() > 10000 {
            let now = Instant::now();
            let max_age = Duration::from_secs(config.window_secs * 2);
            store.retain(|_, entry| now.duration_since(entry.window_start) < max_age);
        }

        let entry = store.entry(key.to_string()).or_insert(RateEntry {
            count: 0,
            window_start: Instant::now(),
        });

        if entry.window_start.elapsed() > Duration::from_secs(config.window_secs) {
            entry.count = 0;
            entry.window_start = Instant::now();
        }

        entry.count += 1;
        entry.count <= config.max_requests
    }

    pub async fn student_rate_limit(&self, ip: &str) -> bool {
        let key = format!("student:{}", ip);
        self.check_rate_limit(&key, "student").await
    }

    pub async fn admin_rate_limit(&self, ip: &str) -> bool {
        let key = format!("admin:{}", ip);
        self.check_rate_limit(&key, "admin").await
    }

    pub async fn login_rate_limit(&self, ip: &str) -> bool {
        let key = format!("login:{}", ip);
        self.check_rate_limit(&key, "login").await
    }

    pub async fn registration_rate_limit(&self, ip: &str) -> bool {
        let key = format!("registration:{}", ip);
        self.check_rate_limit(&key, "registration").await
    }

    pub async fn client_log_rate_limit(&self, ip: &str) -> bool {
        let key = format!("clientlog:{}", ip);
        self.check_rate_limit(&key, "clientlog").await
    }

    pub fn is_redis_enabled(&self) -> bool {
        self.use_redis
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

fn limit_type_from_key(key: &str) -> &str {
    if key.starts_with("student:") {
        "student"
    } else if key.starts_with("admin:") {
        "admin"
    } else if key.starts_with("login:") {
        "login"
    } else if key.starts_with("registration:") {
        "registration"
    } else if key.starts_with("clientlog:") {
        "clientlog"
    } else {
        "admin"
    }
}
