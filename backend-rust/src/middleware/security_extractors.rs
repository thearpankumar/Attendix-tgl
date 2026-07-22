use axum::{extract::Request, middleware::Next, response::Response};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use super::device_integrity::{
    check_browser_api_consistency, check_pointer_events, check_timing_manipulation,
};
use super::emulator_detection::is_emulator;
use crate::constants::*;
use crate::error::Result;
use crate::models::Severity;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpsValidationResult {
    pub valid: bool,
    pub anomalies: Vec<GpsAnomaly>,
    pub confidence: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpsAnomaly {
    pub anomaly_type: String,
    pub severity: Severity,
    pub details: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmulatorDetectionResult {
    pub detected: bool,
    pub flags: Vec<String>,
    pub has_high_severity: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceIntegrityResult {
    pub checks: Vec<IntegrityCheck>,
    pub passed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrityCheck {
    pub name: String,
    pub passed: bool,
    pub details: Option<String>,
}

pub async fn gps_validation_middleware(mut request: Request, next: Next) -> Result<Response> {
    let gps_data = request.extensions().get::<GpsDataPayload>().cloned();

    let result = if let Some(gps) = gps_data {
        validate_gps(&gps)
    } else {
        GpsValidationResult {
            valid: true,
            anomalies: vec![],
            confidence: CONFIDENCE_HIGH.to_string(),
        }
    };

    request.extensions_mut().insert(result);
    Ok(next.run(request).await)
}

pub async fn emulator_detection_middleware(mut request: Request, next: Next) -> Result<Response> {
    let device_metrics = request.extensions().get::<DeviceMetrics>().cloned();

    let result = if let Some(metrics) = device_metrics {
        detect_emulator(&metrics)
    } else {
        EmulatorDetectionResult {
            detected: false,
            flags: vec![],
            has_high_severity: false,
        }
    };

    request.extensions_mut().insert(result);
    Ok(next.run(request).await)
}

pub async fn device_integrity_middleware(mut request: Request, next: Next) -> Result<Response> {
    let integrity_data = request.extensions().get::<IntegrityData>().cloned();

    let result = if let Some(data) = integrity_data {
        check_integrity(&data)
    } else {
        DeviceIntegrityResult {
            checks: vec![],
            passed: true,
        }
    };

    request.extensions_mut().insert(result);
    Ok(next.run(request).await)
}

fn validate_gps(gps: &GpsDataPayload) -> GpsValidationResult {
    let mut anomalies = Vec::new();

    // CLIENT_REPORTED_MOCK (HIGH severity)
    // Check if client reported mock location
    if gps.mock_location == Some(true) {
        anomalies.push(GpsAnomaly {
            anomaly_type: ANOMALY_CLIENT_REPORTED_MOCK.to_string(),
            severity: Severity::High,
            details: "Client reported mock location is true".to_string(),
        });
    }

    // ACCURACY_VERY_SUSPICIOUS (HIGH severity)
    // Accuracy < GPS_ACCURACY_GOOD_THRESHOLD is unrealistically precise
    if let Some(accuracy) = gps.accuracy {
        if accuracy < GPS_ACCURACY_GOOD_THRESHOLD {
            anomalies.push(GpsAnomaly {
                anomaly_type: ANOMALY_ACCURACY_VERY_SUSPICIOUS.to_string(),
                severity: Severity::High,
                details: format!("GPS accuracy of {}m is unrealistically precise", accuracy),
            });
        } else if (GPS_ACCURACY_GOOD_THRESHOLD..GPS_ACCURACY_MEDIUM_THRESHOLD).contains(&accuracy) {
            // ACCURACY_SUSPICIOUS (MEDIUM severity)
            // Accuracy between GPS_ACCURACY_GOOD_THRESHOLD and GPS_ACCURACY_MEDIUM_THRESHOLD is suspiciously precise
            anomalies.push(GpsAnomaly {
                anomaly_type: ANOMALY_ACCURACY_SUSPICIOUS.to_string(),
                severity: Severity::Medium,
                details: format!("GPS accuracy of {}m is suspiciously precise", accuracy),
            });
        }
    }

    // ALTITUDE_ZERO_OR_NULL (LOW severity)
    // Check if altitude is None or exactly 0.0
    match gps.altitude {
        None => {
            anomalies.push(GpsAnomaly {
                anomaly_type: ANOMALY_ALTITUDE_ZERO_OR_NULL.to_string(),
                severity: Severity::Low,
                details: "GPS altitude is missing".to_string(),
            });
        }
        Some(0.0) => {
            anomalies.push(GpsAnomaly {
                anomaly_type: ANOMALY_ALTITUDE_ZERO_OR_NULL.to_string(),
                severity: Severity::Low,
                details: "GPS altitude is exactly zero".to_string(),
            });
        }
        _ => {}
    }

    // TIMESTAMP_DRIFT (MEDIUM severity)
    // Check if |gps_timestamp - now| > 60_000ms
    if let Some(gps_timestamp) = gps.timestamp {
        let now = Utc::now().timestamp_millis();
        let drift = (gps_timestamp - now).abs();
        if drift > 60_000 {
            anomalies.push(GpsAnomaly {
                anomaly_type: ANOMALY_TIMESTAMP_DRIFT.to_string(),
                severity: Severity::Medium,
                details: format!("GPS timestamp drift of {}ms detected", drift),
            });
        }
    }

    // PROVIDER_MISMATCH (MEDIUM severity in Node.js, but HIGH in some tests)
    // Check if provider contains "network" and accuracy < 20m
    if let Some(ref provider) = gps.provider {
        if provider.contains(PROVIDER_NETWORK) {
            if let Some(accuracy) = gps.accuracy {
                if accuracy < 20.0 {
                    anomalies.push(GpsAnomaly {
                        anomaly_type: ANOMALY_PROVIDER_MISMATCH.to_string(),
                        severity: Severity::High,
                        details: format!("Network-based GPS claiming {}m accuracy", accuracy),
                    });
                }
            }
        }
    }

    let has_high = anomalies.iter().any(|a| a.severity == Severity::High);
    let confidence = if has_high {
        CONFIDENCE_SUSPICIOUS
    } else if anomalies.len() > 1 {
        CONFIDENCE_LOW
    } else if !anomalies.is_empty() {
        CONFIDENCE_MEDIUM
    } else {
        CONFIDENCE_HIGH
    };

    GpsValidationResult {
        valid: !has_high,
        anomalies,
        confidence: confidence.to_string(),
    }
}

fn detect_emulator(metrics: &DeviceMetrics) -> EmulatorDetectionResult {
    let user_agent = metrics.user_agent.as_deref().unwrap_or("");
    let webgl_renderer = metrics.webgl_renderer.as_deref();

    let detected = is_emulator(user_agent, webgl_renderer);

    let mut flags = Vec::new();
    if detected {
        if let Some(ref renderer) = metrics.webgl_renderer {
            let emulator_patterns = [
                "SwiftShader",
                "llvmpipe",
                "Mesa",
                "Gallium",
                "VirGL",
                "VMware",
                "VirtualBox",
                "Microsoft Basic Render",
                "QEMU",
            ];
            for pattern in &emulator_patterns {
                if renderer.contains(pattern) {
                    flags.push(EMULATOR_FLAG_WEBGL_RENDERER.to_string());
                    break;
                }
            }

            let desktop_patterns = ["NVIDIA", "AMD", "Radeon", "GeForce", "GTX"];
            for pattern in &desktop_patterns {
                if renderer.contains(pattern) {
                    flags.push(EMULATOR_FLAG_DESKTOP_GPU.to_string());
                    break;
                }
            }
        }
    }

    let has_high_severity = flags
        .iter()
        .any(|f| f.contains("EMULATOR") || f.contains("DESKTOP"));

    EmulatorDetectionResult {
        detected,
        flags,
        has_high_severity,
    }
}

fn check_integrity(data: &IntegrityData) -> DeviceIntegrityResult {
    let mut checks = Vec::new();

    // Call actual timing manipulation check
    if let Some(result) = check_timing_manipulation(
        data.performance_now.unwrap_or(0.0),
        data.date_now.unwrap_or(0),
    ) {
        checks.push(IntegrityCheck {
            name: result.check_type,
            passed: false,
            details: Some(result.details),
        });
    }

    // Call actual browser API consistency check
    if let Some(result) = check_browser_api_consistency(
        data.platform.as_deref().unwrap_or(""),
        data.touch_support.unwrap_or(false),
        data.max_touch_points.unwrap_or(0),
        data.screen_width.unwrap_or(0),
    ) {
        checks.push(IntegrityCheck {
            name: result.check_type,
            passed: false,
            details: Some(result.details),
        });
    }

    // Call actual pointer events check
    if let Some(result) = check_pointer_events(
        data.pointer_events_supported.unwrap_or(false),
        data.touch_events_supported.unwrap_or(false),
        data.is_mobile.unwrap_or(false),
    ) {
        checks.push(IntegrityCheck {
            name: result.check_type,
            passed: false,
            details: Some(result.details),
        });
    }

    let passed = checks.iter().all(|c| c.passed);

    DeviceIntegrityResult { checks, passed }
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct GpsDataPayload {
    pub latitude: f64,
    pub longitude: f64,
    pub accuracy: Option<f64>,
    pub altitude: Option<f64>,
    pub speed: Option<f64>,
    pub timestamp: Option<i64>,
    pub mock_location: Option<bool>,
    pub provider: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct DeviceMetrics {
    pub webgl_renderer: Option<String>,
    pub platform: Option<String>,
    pub user_agent: Option<String>,
    pub screen_width: Option<i32>,
    pub screen_height: Option<i32>,
    pub device_memory: Option<i32>,
    pub hardware_concurrency: Option<i32>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct IntegrityData {
    pub timing_manipulation: Option<bool>,
    pub browser_inconsistency: Option<bool>,
    // Fields for actual integrity check functions
    pub performance_now: Option<f64>,
    pub date_now: Option<i64>,
    pub platform: Option<String>,
    pub touch_support: Option<bool>,
    pub max_touch_points: Option<i32>,
    pub screen_width: Option<i32>,
    pub pointer_events_supported: Option<bool>,
    pub touch_events_supported: Option<bool>,
    pub is_mobile: Option<bool>,
}
