use redis::AsyncCommands;
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::RwLock;

const RATE_LIMIT_PREFIX: &str = "rl:";

#[derive(Clone)]
pub struct RateLimitConfig {
    pub window_secs: u64,
    pub max_requests: u32,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            window_secs: 60,
            max_requests: 1000,
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
        Self {
            use_redis: redis.is_some(),
            redis,
            memory_store: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn check_rate_limit(&self, key: &str, config: &RateLimitConfig) -> bool {
        // Skip rate limiting in test environment
        if std::env::var("NODE_ENV").unwrap_or_default() == "test" {
            return true;
        }

        if self.use_redis {
            match self.check_with_redis(key, config).await {
                Ok(result) => result,
                Err(e) => {
                    tracing::warn!(
                        "Redis rate limit check failed: {}, falling back to memory",
                        e
                    );
                    self.check_with_config(key, config).await
                }
            }
        } else {
            self.check_with_config(key, config).await
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

    pub async fn student_rate_limit(&self, ip: &str, max_requests: u32, window_secs: u64) -> bool {
        let key = format!("student:{}", ip);
        self.check_rate_limit(&key, &RateLimitConfig::new(window_secs, max_requests))
            .await
    }

    pub async fn admin_rate_limit(&self, ip: &str, max_requests: u32, window_secs: u64) -> bool {
        let key = format!("admin:{}", ip);
        self.check_rate_limit(&key, &RateLimitConfig::new(window_secs, max_requests))
            .await
    }

    pub async fn login_rate_limit(&self, ip: &str, max_requests: u32, window_secs: u64) -> bool {
        let key = format!("login:{}", ip);
        self.check_rate_limit(&key, &RateLimitConfig::new(window_secs, max_requests))
            .await
    }

    pub async fn registration_rate_limit(
        &self,
        ip: &str,
        max_requests: u32,
        window_secs: u64,
    ) -> bool {
        let key = format!("registration:{}", ip);
        self.check_rate_limit(&key, &RateLimitConfig::new(window_secs, max_requests))
            .await
    }

    /// Per-identity submission caps, layered on top of the per-IP
    /// `student_rate_limit`. IP-only limiting shares one bucket across an
    /// entire NAT'd classroom (false positives) and gives an attacker
    /// rotating through many IPs effectively no limit at all against a
    /// single roll number or device (false negatives).
    pub async fn roll_number_rate_limit(
        &self,
        roll_number: &str,
        max_requests: u32,
        window_secs: u64,
    ) -> bool {
        let key = format!("rollnum:{}", roll_number.to_uppercase());
        self.check_rate_limit(&key, &RateLimitConfig::new(window_secs, max_requests))
            .await
    }

    pub async fn device_fingerprint_rate_limit(
        &self,
        device_fingerprint: &str,
        max_requests: u32,
        window_secs: u64,
    ) -> bool {
        let key = format!("devicefp:{}", device_fingerprint);
        self.check_rate_limit(&key, &RateLimitConfig::new(window_secs, max_requests))
            .await
    }

    pub async fn short_link_guess_rate_limit(
        &self,
        ip: &str,
        max_requests: u32,
        window_secs: u64,
    ) -> bool {
        let key = format!("shortlinkguess:{}", ip);
        self.check_rate_limit(&key, &RateLimitConfig::new(window_secs, max_requests))
            .await
    }

    pub async fn client_log_rate_limit(
        &self,
        ip: &str,
        max_requests: u32,
        window_secs: u64,
    ) -> bool {
        let key = format!("clientlog:{}", ip);
        self.check_rate_limit(&key, &RateLimitConfig::new(window_secs, max_requests))
            .await
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
    } else if key.starts_with("shortlinkguess:") {
        "shortlinkguess"
    } else if key.starts_with("rollnum:") {
        "rollnum"
    } else if key.starts_with("devicefp:") {
        "devicefp"
    } else {
        "admin"
    }
}
