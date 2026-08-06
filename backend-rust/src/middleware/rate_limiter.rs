use redis::AsyncCommands;
use std::sync::Arc;

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
    redis: Arc<redis::Client>,
}

impl RateLimiter {
    pub fn new(redis: Arc<redis::Client>) -> Self {
        Self { redis }
    }

    pub async fn check_rate_limit(&self, key: &str, config: &RateLimitConfig) -> bool {
        // Skip rate limiting in test environment
        if std::env::var("NODE_ENV").unwrap_or_default() == "test" {
            return true;
        }

        match self.check_with_redis(key, config).await {
            Ok(result) => result,
            Err(e) => {
                tracing::error!("Redis rate limit check failed: {}", e);
                // Fail closed: an unreachable Redis must not silently grant
                // unlimited requests on the endpoints this protects (login,
                // registration, attendance submission, etc).
                false
            }
        }
    }

    async fn check_with_redis(&self, key: &str, config: &RateLimitConfig) -> Result<bool, String> {
        let mut conn = self
            .redis
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
