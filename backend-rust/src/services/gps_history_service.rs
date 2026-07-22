use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;

use crate::constants::*;
use crate::models::Severity;

const GPS_HISTORY_PREFIX: &str = "gps:positions:";
const GPS_HISTORY_TTL: u64 = 300;
const MAX_HISTORY_SIZE: usize = 20;

/// Represents a detected GPS anomaly (e.g., impossible travel, position jump)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpsAnomaly {
    pub anomaly_type: String,
    pub severity: Severity,
    pub details: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpsPositionEntry {
    pub latitude: f64,
    pub longitude: f64,
    pub accuracy: Option<f64>,
    pub altitude: Option<f64>,
    pub speed: Option<f64>,
    pub heading: Option<f64>,
    pub timestamp: i64,
    pub provider: Option<String>,
    pub mock_location: Option<bool>,
}

pub struct GpsHistoryService {
    redis: Option<Arc<redis::Client>>,
    max_history: usize,
    ttl_secs: u64,
}

impl GpsHistoryService {
    pub fn new(redis: Option<Arc<redis::Client>>) -> Self {
        Self {
            redis,
            max_history: MAX_HISTORY_SIZE,
            ttl_secs: GPS_HISTORY_TTL,
        }
    }

    pub async fn add_position(
        &self,
        device_id: &str,
        position: GpsPositionEntry,
    ) -> crate::error::Result<()> {
        if let Some(ref redis_client) = self.redis {
            self.add_to_redis(redis_client, device_id, position).await?;
        }
        Ok(())
    }

    async fn add_to_redis(
        &self,
        redis_client: &Arc<redis::Client>,
        device_id: &str,
        position: GpsPositionEntry,
    ) -> crate::error::Result<()> {
        let mut conn = redis_client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| {
                crate::error::AppError::Internal(format!("Redis connection failed: {}", e))
            })?;

        let key = format!("{}{}", GPS_HISTORY_PREFIX, device_id);

        let history = self.get_history(device_id).await?;

        let mut history: VecDeque<GpsPositionEntry> = history.into_iter().collect();
        history.push_back(position);

        if history.len() > self.max_history {
            history.pop_front();
        }

        let history: Vec<GpsPositionEntry> = history.into_iter().collect();

        let json = serde_json::to_string(&history).map_err(|e| {
            crate::error::AppError::Internal(format!("JSON serialization failed: {}", e))
        })?;

        let _: () = conn
            .set_ex(&key, json, self.ttl_secs)
            .await
            .map_err(|e| crate::error::AppError::Internal(format!("Redis SET failed: {}", e)))?;

        Ok(())
    }

    pub async fn get_history(
        &self,
        device_id: &str,
    ) -> crate::error::Result<Vec<GpsPositionEntry>> {
        if let Some(ref redis_client) = self.redis {
            self.get_from_redis(redis_client, device_id).await
        } else {
            Ok(Vec::new())
        }
    }

    async fn get_from_redis(
        &self,
        redis_client: &Arc<redis::Client>,
        device_id: &str,
    ) -> crate::error::Result<Vec<GpsPositionEntry>> {
        let mut conn = redis_client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| {
                crate::error::AppError::Internal(format!("Redis connection failed: {}", e))
            })?;

        let key = format!("{}{}", GPS_HISTORY_PREFIX, device_id);

        let result: Option<String> = conn
            .get(&key)
            .await
            .map_err(|e| crate::error::AppError::Internal(format!("Redis GET failed: {}", e)))?;

        match result {
            Some(json) => {
                let history: Vec<GpsPositionEntry> = serde_json::from_str(&json).map_err(|e| {
                    crate::error::AppError::Internal(format!("JSON parse failed: {}", e))
                })?;
                Ok(history)
            }
            None => Ok(Vec::new()),
        }
    }

    pub async fn clear_history(&self, device_id: &str) -> crate::error::Result<()> {
        if let Some(ref redis_client) = self.redis {
            let mut conn = redis_client
                .get_multiplexed_async_connection()
                .await
                .map_err(|e| {
                    crate::error::AppError::Internal(format!("Redis connection failed: {}", e))
                })?;

            let key = format!("{}{}", GPS_HISTORY_PREFIX, device_id);
            let _: () = conn.del(&key).await.map_err(|e| {
                crate::error::AppError::Internal(format!("Redis DEL failed: {}", e))
            })?;
        }
        Ok(())
    }

    pub async fn detect_position_jump(
        &self,
        device_id: &str,
        new_position: &GpsPositionEntry,
        threshold_meters: f64,
    ) -> crate::error::Result<bool> {
        let history = self.get_history(device_id).await?;

        for prev_position in history.iter().rev().take(5) {
            let distance = crate::utils::calculate_distance(
                prev_position.latitude,
                prev_position.longitude,
                new_position.latitude,
                new_position.longitude,
            );

            if distance > threshold_meters {
                let time_diff = (new_position.timestamp - prev_position.timestamp).abs();
                if time_diff > 0 {
                    let speed_mps = distance / (time_diff as f64 / 1000.0);
                    let speed_kmh = speed_mps * 3.6;

                    if speed_kmh > MAX_REASONABLE_SPEED_KMH {
                        return Ok(true);
                    }
                }
            }
        }

        Ok(false)
    }

    pub async fn detect_impossible_travel(
        &self,
        device_id: &str,
        new_position: &GpsPositionEntry,
    ) -> crate::error::Result<Vec<GpsAnomaly>> {
        let history = self.get_history(device_id).await?;
        let mut anomalies = Vec::new();

        for prev_position in history.iter().rev().take(10) {
            let distance = crate::utils::calculate_distance(
                prev_position.latitude,
                prev_position.longitude,
                new_position.latitude,
                new_position.longitude,
            );

            let time_diff_ms = (new_position.timestamp - prev_position.timestamp).abs();

            if time_diff_ms > 0 && time_diff_ms < 1000 && distance > GEOGENCE_MAX_DISTANCE_M {
                anomalies.push(GpsAnomaly {
                    anomaly_type: "RAPID_POSITION_CHANGE".to_string(),
                    severity: Severity::High,
                    details: format!("Position changed {}m in {}ms", distance, time_diff_ms),
                });
            }

            if time_diff_ms > 0 && distance > 0.0 {
                let speed_kmh = (distance / (time_diff_ms as f64 / 1000.0)) * 3.6;
                if speed_kmh > IMPOSSIBLE_SPEED_KMH {
                    anomalies.push(GpsAnomaly {
                        anomaly_type: "IMPOSSIBLE_SPEED".to_string(),
                        severity: Severity::High,
                        details: format!("Speed {} km/h detected", speed_kmh),
                    });
                }
            }
        }

        Ok(anomalies)
    }

    pub fn is_enabled(&self) -> bool {
        self.redis.is_some()
    }
}

impl Default for GpsHistoryService {
    fn default() -> Self {
        Self::new(None)
    }
}
