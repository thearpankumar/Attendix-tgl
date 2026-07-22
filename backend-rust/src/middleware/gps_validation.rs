use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::constants::*;
use crate::models::Severity;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpsAnomalyResult {
    pub anomaly_type: String,
    pub severity: Severity,
    pub details: String,
    pub detected_at: DateTime<Utc>,
}

pub fn check_position_jump(
    prev_lat: f64,
    prev_lon: f64,
    curr_lat: f64,
    curr_lon: f64,
    threshold_meters: f64,
    prev_time: DateTime<Utc>,
    curr_time: DateTime<Utc>,
) -> Option<GpsAnomalyResult> {
    let distance = crate::utils::calculate_distance(prev_lat, prev_lon, curr_lat, curr_lon);
    let time_diff = (curr_time - prev_time).num_milliseconds() as f64 / 1000.0;

    if time_diff > 0.0 {
        let speed = distance / time_diff * 3.6;
        if speed > MAX_REASONABLE_SPEED_KMH && distance > threshold_meters {
            return Some(GpsAnomalyResult {
                anomaly_type: ANOMALY_POSITION_JUMP.to_string(),
                severity: Severity::High,
                details: format!(
                    "Position jumped {}m in {}s",
                    distance.round(),
                    time_diff.round()
                ),
                detected_at: Utc::now(),
            });
        }
    }
    None
}

pub fn check_gps_accuracy(accuracy: Option<f64>) -> Option<GpsAnomalyResult> {
    match accuracy {
        Some(acc) if acc > GPS_ACCURACY_SUSPICIOUS_THRESHOLD => Some(GpsAnomalyResult {
            anomaly_type: ANOMALY_LOW_ACCURACY.to_string(),
            severity: Severity::Medium,
            details: format!(
                "GPS accuracy is {}m (>{}m)",
                acc, GPS_ACCURACY_SUSPICIOUS_THRESHOLD
            ),
            detected_at: Utc::now(),
        }),
        _ => None,
    }
}

pub fn check_gps_speed(speed: Option<f64>, max_speed_kmh: f64) -> Option<GpsAnomalyResult> {
    match speed {
        Some(s) if s > max_speed_kmh => Some(GpsAnomalyResult {
            anomaly_type: ANOMALY_IMPOSSIBLE_SPEED.to_string(),
            severity: Severity::High,
            details: format!("Reported speed {} km/h exceeds maximum", s),
            detected_at: Utc::now(),
        }),
        _ => None,
    }
}

/// Check for client-reported mock location (HIGH severity)
pub fn check_mock_location(is_mock: Option<bool>) -> Option<GpsAnomalyResult> {
    if is_mock == Some(true) {
        Some(GpsAnomalyResult {
            anomaly_type: ANOMALY_CLIENT_REPORTED_MOCK.to_string(),
            severity: Severity::High,
            details: "Client reported mock location is true".to_string(),
            detected_at: Utc::now(),
        })
    } else {
        None
    }
}

/// Check for suspiciously high GPS accuracy (HIGH/MEDIUM severity)
pub fn check_suspicious_accuracy(accuracy: Option<f64>) -> Option<GpsAnomalyResult> {
    match accuracy {
        Some(acc) if acc < GPS_ACCURACY_GOOD_THRESHOLD => Some(GpsAnomalyResult {
            anomaly_type: ANOMALY_ACCURACY_VERY_SUSPICIOUS.to_string(),
            severity: Severity::High,
            details: format!("GPS accuracy of {}m is unrealistically precise", acc),
            detected_at: Utc::now(),
        }),
        Some(acc)
            if (GPS_ACCURACY_GOOD_THRESHOLD..GPS_ACCURACY_MEDIUM_THRESHOLD).contains(&acc) =>
        {
            Some(GpsAnomalyResult {
                anomaly_type: ANOMALY_ACCURACY_SUSPICIOUS.to_string(),
                severity: Severity::Medium,
                details: format!("GPS accuracy of {}m is suspiciously precise", acc),
                detected_at: Utc::now(),
            })
        }
        _ => None,
    }
}

/// Check for missing or zero altitude (LOW severity)
pub fn check_altitude_issue(altitude: Option<f64>) -> Option<GpsAnomalyResult> {
    match altitude {
        None => Some(GpsAnomalyResult {
            anomaly_type: ANOMALY_ALTITUDE_ZERO_OR_NULL.to_string(),
            severity: Severity::Low,
            details: "GPS altitude is missing".to_string(),
            detected_at: Utc::now(),
        }),
        Some(0.0) => Some(GpsAnomalyResult {
            anomaly_type: ANOMALY_ALTITUDE_ZERO_OR_NULL.to_string(),
            severity: Severity::Low,
            details: "GPS altitude is exactly zero".to_string(),
            detected_at: Utc::now(),
        }),
        _ => None,
    }
}

/// Check for timestamp drift (MEDIUM severity)
pub fn check_timestamp_drift(
    gps_timestamp: Option<i64>,
    max_drift_ms: i64,
) -> Option<GpsAnomalyResult> {
    gps_timestamp.and_then(|ts| {
        let now = Utc::now().timestamp_millis();
        let drift = (ts - now).abs();
        if drift > max_drift_ms {
            Some(GpsAnomalyResult {
                anomaly_type: ANOMALY_TIMESTAMP_DRIFT.to_string(),
                severity: Severity::Medium,
                details: format!("GPS timestamp drift of {}ms detected", drift),
                detected_at: Utc::now(),
            })
        } else {
            None
        }
    })
}

/// Check for provider/accuracy mismatch (HIGH severity)
pub fn check_provider_mismatch(
    provider: Option<&str>,
    accuracy: Option<f64>,
) -> Option<GpsAnomalyResult> {
    provider.and_then(|prov| {
        if prov.contains(PROVIDER_NETWORK) {
            accuracy.and_then(|acc| {
                if acc < 20.0 {
                    Some(GpsAnomalyResult {
                        anomaly_type: ANOMALY_PROVIDER_MISMATCH.to_string(),
                        severity: Severity::High,
                        details: format!("Network-based GPS claiming {}m accuracy", acc),
                        detected_at: Utc::now(),
                    })
                } else {
                    None
                }
            })
        } else {
            None
        }
    })
}
