use chrono::{DateTime, Duration, Utc};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

const SESSION_CACHE_PREFIX: &str = "session:";

#[derive(Clone, Serialize, Deserialize)]
pub struct CachedSession {
    pub id: Uuid,
    pub token_hash: String,
    pub location_id: Uuid,
    pub location_name: Option<String>,
    pub location_latitude: Option<f64>,
    pub location_longitude: Option<f64>,
    pub location_radius: Option<f64>,
    pub batch_id: Option<Uuid>,
    pub created_by: Uuid,
    pub is_active: bool,
    pub expires_at: DateTime<Utc>,
    pub totp_secret: Option<String>,
    pub description: Option<String>,
    pub cached_at: DateTime<Utc>,
}

pub struct SessionCache {
    redis: Arc<redis::Client>,
    ttl_secs: i64,
}

impl SessionCache {
    pub fn new(redis: Arc<redis::Client>, ttl_secs: i64) -> Self {
        Self { redis, ttl_secs }
    }

    pub async fn get(&self, token_hash: &str) -> Option<CachedSession> {
        let mut conn = match self.redis.get_multiplexed_async_connection().await {
            Ok(conn) => conn,
            Err(e) => {
                tracing::warn!("Session cache Redis connection failed: {}", e);
                return None;
            }
        };
        let key = format!("{}{}", SESSION_CACHE_PREFIX, token_hash);

        let result: Option<String> = conn.get(&key).await.ok()?;

        if let Some(json) = result {
            let session: CachedSession = serde_json::from_str(&json).ok()?;

            if session.cached_at + Duration::seconds(self.ttl_secs) > Utc::now() {
                return Some(session);
            }
        }

        None
    }

    pub async fn set(&self, token_hash: String, session: CachedSession) {
        let mut conn = match self.redis.get_multiplexed_async_connection().await {
            Ok(conn) => conn,
            Err(e) => {
                tracing::warn!("Session cache Redis connection failed: {}", e);
                return;
            }
        };
        let key = format!("{}{}", SESSION_CACHE_PREFIX, token_hash);

        if let Ok(json) = serde_json::to_string(&session) {
            let _: Result<(), _> = conn.set_ex(&key, json, self.ttl_secs as u64).await;
        }
    }

    pub async fn invalidate(&self, token_hash: &str) {
        let mut conn = match self.redis.get_multiplexed_async_connection().await {
            Ok(conn) => conn,
            Err(e) => {
                tracing::warn!("Session cache Redis connection failed: {}", e);
                return;
            }
        };
        let key = format!("{}{}", SESSION_CACHE_PREFIX, token_hash);
        let _: Result<(), _> = conn.del(&key).await;
    }

    pub async fn clear(&self) {
        let mut conn = match self.redis.get_multiplexed_async_connection().await {
            Ok(conn) => conn,
            Err(e) => {
                tracing::warn!("Session cache Redis connection failed: {}", e);
                return;
            }
        };
        let pattern = format!("{}*", SESSION_CACHE_PREFIX);
        if let Ok(keys) = redis::cmd("KEYS")
            .arg(&pattern)
            .query_async::<Vec<String>>(&mut conn)
            .await
        {
            if !keys.is_empty() {
                let _: Result<(), _> = redis::cmd("DEL").arg(&keys).query_async(&mut conn).await;
            }
        }
    }
}

pub async fn get_or_fetch_session(
    cache: &SessionCache,
    token_hash: &str,
    db: &sqlx::PgPool,
) -> crate::error::Result<Option<CachedSession>> {
    if let Some(cached) = cache.get(token_hash).await {
        tracing::debug!("Session cache hit for token_hash");
        return Ok(Some(cached));
    }

    tracing::debug!("Session cache miss, fetching from database");

    let session = sqlx::query_as::<_, crate::models::Session>(
        "SELECT * FROM sessions WHERE token_hash = $1 AND is_active = true AND expires_at > now()",
    )
    .bind(token_hash)
    .fetch_optional(db)
    .await?;

    if let Some(session) = session {
        let location =
            sqlx::query_as::<_, crate::models::Location>("SELECT * FROM locations WHERE id = $1")
                .bind(session.location_id)
                .fetch_optional(db)
                .await?;

        let cached = CachedSession {
            id: session.id,
            token_hash: session.token_hash.clone(),
            location_id: session.location_id,
            location_name: location.as_ref().map(|l| l.name.clone()),
            location_latitude: location.as_ref().map(|l| l.latitude),
            location_longitude: location.as_ref().map(|l| l.longitude),
            location_radius: location.as_ref().map(|l| l.radius_meters),
            batch_id: session.batch_id,
            created_by: session.created_by,
            is_active: session.is_active,
            expires_at: session.expires_at,
            totp_secret: session.totp_secret.clone(),
            description: session.description.clone(),
            cached_at: Utc::now(),
        };

        cache.set(token_hash.to_string(), cached.clone()).await;

        return Ok(Some(cached));
    }

    Ok(None)
}
