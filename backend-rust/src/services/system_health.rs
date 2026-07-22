use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemHealth {
    pub overall_score: f64,
    pub components: ComponentHealth,
    pub last_updated: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealth {
    pub database: HealthStatus,
    pub redis: HealthStatus,
    pub storage: HealthStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    pub status: String,
    pub score: f64,
    pub latency_ms: Option<f64>,
    pub last_check: DateTime<Utc>,
}

pub async fn get_system_health(
    db_client: &mongodb::Client,
    redis_client: Option<&redis::Client>,
    storage: &crate::storage::Storage,
) -> crate::error::Result<SystemHealth> {
    let db_health = check_database(db_client).await?;
    let redis_health = check_redis(redis_client).await?;
    let storage_health = check_storage(storage).await?;

    let components = ComponentHealth {
        database: db_health.clone(),
        redis: redis_health.clone(),
        storage: storage_health.clone(),
    };

    let overall_score = (db_health.score + redis_health.score + storage_health.score) / 3.0;

    Ok(SystemHealth {
        overall_score,
        components,
        last_updated: Utc::now(),
    })
}

pub async fn check_database(db_client: &mongodb::Client) -> crate::error::Result<HealthStatus> {
    let start = std::time::Instant::now();

    let db = db_client.database("admin");
    let result = db.run_command(mongodb::bson::doc! { "ping": 1 }).await;

    let latency = start.elapsed().as_millis() as f64;

    Ok(match result {
        Ok(_) => HealthStatus {
            status: "healthy".to_string(),
            score: 100.0,
            latency_ms: Some(latency),
            last_check: Utc::now(),
        },
        Err(_) => HealthStatus {
            status: "unhealthy".to_string(),
            score: 0.0,
            latency_ms: Some(latency),
            last_check: Utc::now(),
        },
    })
}

pub async fn check_redis(
    redis_client: Option<&redis::Client>,
) -> crate::error::Result<HealthStatus> {
    match redis_client {
        Some(client) => {
            let start = std::time::Instant::now();

            let mut conn = client
                .get_connection_manager()
                .await
                .map_err(crate::error::AppError::Redis)?;
            let result: redis::RedisResult<String> =
                redis::cmd("PING").query_async(&mut conn).await;

            let latency = start.elapsed().as_millis() as f64;

            Ok(match result {
                Ok(_) => HealthStatus {
                    status: "healthy".to_string(),
                    score: 100.0,
                    latency_ms: Some(latency),
                    last_check: Utc::now(),
                },
                Err(_) => HealthStatus {
                    status: "unhealthy".to_string(),
                    score: 0.0,
                    latency_ms: Some(latency),
                    last_check: Utc::now(),
                },
            })
        }
        None => Ok(HealthStatus {
            status: "disabled".to_string(),
            score: 100.0,
            latency_ms: None,
            last_check: Utc::now(),
        }),
    }
}

pub async fn check_storage(
    storage: &crate::storage::Storage,
) -> crate::error::Result<HealthStatus> {
    let start = std::time::Instant::now();

    // Try to list objects (just 1 to minimize cost)
    match storage.provider().list_objects(1).await {
        Ok(_) => {
            let latency_ms = start.elapsed().as_millis() as f64;
            let score = if latency_ms < 100.0 {
                100.0
            } else if latency_ms < 500.0 {
                90.0
            } else {
                70.0
            };
            Ok(HealthStatus {
                status: "healthy".to_string(),
                score,
                latency_ms: Some(latency_ms),
                last_check: Utc::now(),
            })
        }
        Err(e) => {
            tracing::error!("Storage health check failed: {}", e);
            Ok(HealthStatus {
                status: "unhealthy".to_string(),
                score: 0.0,
                latency_ms: None,
                last_check: Utc::now(),
            })
        }
    }
}
