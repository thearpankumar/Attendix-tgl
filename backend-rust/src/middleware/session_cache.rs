use chrono::{DateTime, Duration, Utc};
use mongodb::bson::oid::ObjectId;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

const SESSION_CACHE_PREFIX: &str = "session:";
const SESSION_CACHE_TTL: i64 = 300;

#[derive(Clone, Serialize, Deserialize)]
pub struct CachedSession {
    pub id: ObjectId,
    pub token_hash: String,
    pub location_id: ObjectId,
    pub location_name: Option<String>,
    pub location_latitude: Option<f64>,
    pub location_longitude: Option<f64>,
    pub location_radius: Option<f64>,
    pub batch_id: Option<ObjectId>,
    pub created_by: ObjectId,
    pub is_active: bool,
    pub expires_at: DateTime<Utc>,
    pub totp_secret: Option<String>,
    pub description: Option<String>,
    pub cached_at: DateTime<Utc>,
}

pub struct SessionCache {
    redis: Option<Arc<redis::Client>>,
    memory_cache: Arc<RwLock<HashMap<String, CachedSession>>>,
    ttl_secs: i64,
    use_redis: bool,
}

impl SessionCache {
    pub fn new(redis: Option<Arc<redis::Client>>, ttl_secs: i64) -> Self {
        Self {
            use_redis: redis.is_some(),
            redis,
            memory_cache: Arc::new(RwLock::new(HashMap::new())),
            ttl_secs,
        }
    }

    pub fn new_memory_only(ttl_secs: i64) -> Self {
        Self {
            use_redis: false,
            redis: None,
            memory_cache: Arc::new(RwLock::new(HashMap::new())),
            ttl_secs,
        }
    }

    pub async fn get(&self, token_hash: &str) -> Option<CachedSession> {
        if self.use_redis {
            self.get_from_redis(token_hash).await
        } else {
            self.get_from_memory(token_hash).await
        }
    }

    async fn get_from_redis(&self, token_hash: &str) -> Option<CachedSession> {
        if let Some(ref redis_client) = self.redis {
            let mut conn = redis_client.get_multiplexed_async_connection().await.ok()?;
            let key = format!("{}{}", SESSION_CACHE_PREFIX, token_hash);

            let result: Option<String> = conn.get(&key).await.ok()?;

            if let Some(json) = result {
                let session: CachedSession = serde_json::from_str(&json).ok()?;

                if session.cached_at + Duration::seconds(self.ttl_secs) > Utc::now() {
                    return Some(session);
                }
            }
        }

        None
    }

    async fn get_from_memory(&self, token_hash: &str) -> Option<CachedSession> {
        let cache = self.memory_cache.read().await;

        if let Some(session) = cache.get(token_hash) {
            if session.cached_at + Duration::seconds(self.ttl_secs) > Utc::now() {
                return Some(session.clone());
            }
        }

        None
    }

    pub async fn set(&self, token_hash: String, session: CachedSession) {
        if self.use_redis {
            self.set_to_redis(&token_hash, &session).await;
        } else {
            self.set_to_memory(token_hash, session).await;
        }
    }

    async fn set_to_redis(&self, token_hash: &str, session: &CachedSession) {
        if let Some(ref redis_client) = self.redis {
            if let Ok(mut conn) = redis_client.get_multiplexed_async_connection().await {
                let key = format!("{}{}", SESSION_CACHE_PREFIX, token_hash);

                if let Ok(json) = serde_json::to_string(session) {
                    let _: Result<(), _> = conn.set_ex(&key, json, self.ttl_secs as u64).await;
                }
            }
        }
    }

    async fn set_to_memory(&self, token_hash: String, session: CachedSession) {
        let mut cache = self.memory_cache.write().await;

        if cache.len() > 1000 {
            cache.retain(|_, v| v.cached_at + Duration::seconds(self.ttl_secs) > Utc::now());
        }

        cache.insert(token_hash, session);
    }

    pub async fn invalidate(&self, token_hash: &str) {
        if self.use_redis {
            self.invalidate_from_redis(token_hash).await;
        }

        let mut cache = self.memory_cache.write().await;
        cache.remove(token_hash);
    }

    async fn invalidate_from_redis(&self, token_hash: &str) {
        if let Some(ref redis_client) = self.redis {
            if let Ok(mut conn) = redis_client.get_multiplexed_async_connection().await {
                let key = format!("{}{}", SESSION_CACHE_PREFIX, token_hash);
                let _: Result<(), _> = conn.del(&key).await;
            }
        }
    }

    pub async fn clear(&self) {
        if self.use_redis {
            self.clear_redis().await;
        }

        let mut cache = self.memory_cache.write().await;
        cache.clear();
    }

    async fn clear_redis(&self) {
        if let Some(ref redis_client) = self.redis {
            if let Ok(mut conn) = redis_client.get_multiplexed_async_connection().await {
                let pattern = format!("{}*", SESSION_CACHE_PREFIX);
                if let Ok(keys) = redis::cmd("KEYS")
                    .arg(&pattern)
                    .query_async::<Vec<String>>(&mut conn)
                    .await
                {
                    if !keys.is_empty() {
                        let _: Result<(), _> =
                            redis::cmd("DEL").arg(&keys).query_async(&mut conn).await;
                    }
                }
            }
        }
    }

    pub fn is_redis_enabled(&self) -> bool {
        self.use_redis
    }
}

impl Default for SessionCache {
    fn default() -> Self {
        Self::new_memory_only(SESSION_CACHE_TTL)
    }
}

pub async fn get_or_fetch_session(
    cache: &SessionCache,
    token_hash: &str,
    db: &mongodb::Database,
) -> crate::error::Result<Option<CachedSession>> {
    if let Some(cached) = cache.get(token_hash).await {
        tracing::debug!("Session cache hit for token_hash");
        return Ok(Some(cached));
    }

    tracing::debug!("Session cache miss, fetching from database");

    use mongodb::bson::doc;

    let collection = db.collection::<crate::models::Session>("sessions");
    let filter = doc! {
        "tokenHash": token_hash,
        "isActive": true,
        "expiresAt": { "$gt": mongodb::bson::DateTime::now() }
    };

    let session = collection.find_one(filter).await?;

    if let Some(session) = session {
        let location_collection = db.collection::<crate::models::Location>("locations");
        let location = location_collection
            .find_one(doc! { "_id": session.location_id })
            .await?;

        let cached = CachedSession {
            id: session
                .id
                .ok_or_else(|| crate::error::AppError::Internal("Session missing ID".into()))?,
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
