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

/// Upper bound on a buffered attendance request body. Photos are uploaded to
/// S3 out-of-band, so these bodies are small JSON documents.
const MAX_SECURITY_BODY_BYTES: usize = 256 * 1024;

/// Security telemetry as the student frontend actually sends it (camelCase,
/// with `integrityChecks` as an array of findings rather than an object).
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ClientSecurityEnvelope {
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub gps_metadata: Option<ClientGpsMetadata>,
    pub device_metrics: Option<ClientDeviceMetrics>,
    pub integrity_checks: Vec<ClientIntegrityFinding>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ClientGpsMetadata {
    pub accuracy: Option<f64>,
    pub altitude: Option<f64>,
    pub speed: Option<f64>,
    pub timestamp: Option<i64>,
    pub is_mock_location: Option<bool>,
    pub provider: Option<String>,
}

/// Raw device signals reported by the browser. These are *inputs* to the
/// server-side verdict, never the verdict itself — the client's own
/// `isEmulation` field is deliberately not read.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ClientDeviceMetrics {
    pub webgl_renderer: Option<String>,
    pub platform: Option<String>,
    pub user_agent: Option<String>,
    pub screen_width: Option<i32>,
    pub screen_height: Option<i32>,
    pub device_memory: Option<i32>,
    pub hardware_concurrency: Option<i32>,
    pub max_touch_points: Option<i32>,
    pub touch_event_support: Option<bool>,
    pub has_coarse_pointer: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ClientIntegrityFinding {
    #[serde(rename = "type")]
    pub finding_type: String,
    pub details: String,
}

/// Runs GPS, emulator and device-integrity analysis over the request body and
/// publishes the three results as request extensions.
///
/// This replaces three separate middlewares that each read their input from a
/// request extension that nothing ever populated, so every request took the
/// "no data" branch and was reported as `valid: true` / `detected: false` /
/// `passed: true`. The analysis functions were correct; they were simply never
/// reached. Reading the body here is what actually connects them.
///
/// The body is buffered and reinstated so downstream handlers still see it.
pub async fn security_analysis_middleware(request: Request, next: Next) -> Result<Response> {
    let (mut parts, body) = request.into_parts();

    let bytes = axum::body::to_bytes(body, MAX_SECURITY_BODY_BYTES)
        .await
        .map_err(|_| {
            crate::error::AppError::BadRequest("Request body too large or unreadable".to_string())
        })?;

    // GET requests (status, captcha, upload-url) carry no telemetry; an empty
    // body deserializes to the default envelope and is analysed as "absent".
    let envelope: ClientSecurityEnvelope = if bytes.is_empty() {
        ClientSecurityEnvelope::default()
    } else {
        serde_json::from_slice(&bytes).unwrap_or_default()
    };

    // The User-Agent the browser actually sent, as seen by the server. A client
    // that spoofs `deviceMetrics.userAgent` but not the header (or vice versa)
    // is caught by the cross-check below.
    let header_user_agent = parts
        .headers
        .get(axum::http::header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    parts.extensions.insert(analyse_gps(&envelope));
    parts
        .extensions
        .insert(analyse_emulator(&envelope, &header_user_agent));
    parts
        .extensions
        .insert(analyse_integrity(&envelope, &header_user_agent));

    let request = Request::from_parts(parts, axum::body::Body::from(bytes));
    Ok(next.run(request).await)
}

fn analyse_gps(envelope: &ClientSecurityEnvelope) -> GpsValidationResult {
    let (Some(latitude), Some(longitude)) = (envelope.latitude, envelope.longitude) else {
        // No coordinates at all: nothing to validate, and nothing to vouch for.
        return GpsValidationResult {
            valid: false,
            anomalies: vec![GpsAnomaly {
                anomaly_type: ANOMALY_MISSING_METADATA.to_string(),
                severity: Severity::Medium,
                details: "Request carried no GPS coordinates".to_string(),
            }],
            confidence: CONFIDENCE_LOW.to_string(),
        };
    };

    let Some(metadata) = envelope.gps_metadata.as_ref() else {
        // Coordinates but no metadata: cannot check accuracy, speed or mock
        // flags, so this must not be reported as high-confidence clean.
        return GpsValidationResult {
            valid: false,
            anomalies: vec![GpsAnomaly {
                anomaly_type: ANOMALY_MISSING_METADATA.to_string(),
                severity: Severity::Medium,
                details: "Coordinates supplied without GPS metadata; cannot verify".to_string(),
            }],
            confidence: CONFIDENCE_LOW.to_string(),
        };
    };

    validate_gps(&GpsDataPayload {
        latitude,
        longitude,
        accuracy: metadata.accuracy,
        altitude: metadata.altitude,
        speed: metadata.speed,
        timestamp: metadata.timestamp,
        mock_location: metadata.is_mock_location,
        provider: metadata.provider.clone(),
    })
}

fn analyse_emulator(
    envelope: &ClientSecurityEnvelope,
    header_user_agent: &str,
) -> EmulatorDetectionResult {
    let Some(metrics) = envelope.device_metrics.as_ref() else {
        return EmulatorDetectionResult {
            detected: false,
            flags: vec![EMULATOR_FLAG_METRICS_MISSING.to_string()],
            has_high_severity: false,
        };
    };

    let mut result = detect_emulator(&DeviceMetrics {
        webgl_renderer: metrics.webgl_renderer.clone(),
        platform: metrics.platform.clone(),
        // Prefer the header the server observed over the value the client
        // reported about itself.
        user_agent: Some(header_user_agent.to_string()),
        screen_width: metrics.screen_width,
        screen_height: metrics.screen_height,
        device_memory: metrics.device_memory,
        hardware_concurrency: metrics.hardware_concurrency,
    });

    // A mismatch between the claimed and observed User-Agent is a strong
    // tampering signal: an honest browser reports the same string twice.
    if let Some(claimed) = metrics.user_agent.as_deref() {
        if !claimed.is_empty() && !header_user_agent.is_empty() && claimed != header_user_agent {
            result.detected = true;
            result.has_high_severity = true;
            result
                .flags
                .push(EMULATOR_FLAG_USER_AGENT_MISMATCH.to_string());
        }
    }

    result
}

fn analyse_integrity(
    envelope: &ClientSecurityEnvelope,
    header_user_agent: &str,
) -> DeviceIntegrityResult {
    let Some(metrics) = envelope.device_metrics.as_ref() else {
        // Fail closed: absent telemetry is "unverified", not "passed". This
        // flags the submission for admin review rather than rejecting it, so a
        // browser that cannot supply the signals does not lose its attendance.
        return DeviceIntegrityResult {
            checks: vec![IntegrityCheck {
                name: "DEVICE_TELEMETRY_MISSING".to_string(),
                passed: false,
                details: Some("No device metrics supplied; integrity unverified".to_string()),
            }],
            passed: false,
        };
    };

    let device_info = super::mobile_check::check_mobile(header_user_agent);
    let is_mobile = device_info.is_mobile || device_info.is_tablet;

    let mut result = check_integrity(&IntegrityData {
        timing_manipulation: None,
        browser_inconsistency: None,
        performance_now: None,
        date_now: None,
        platform: metrics.platform.clone(),
        touch_support: metrics.touch_event_support,
        max_touch_points: metrics.max_touch_points,
        screen_width: metrics.screen_width,
        pointer_events_supported: metrics.has_coarse_pointer,
        touch_events_supported: metrics.touch_event_support,
        is_mobile: Some(is_mobile),
    });

    // Client-reported findings can only add suspicion — they are recorded, but
    // an empty list never clears a server-derived failure.
    for finding in &envelope.integrity_checks {
        result.checks.push(IntegrityCheck {
            name: format!("CLIENT_{}", finding.finding_type),
            passed: false,
            details: Some(finding.details.clone()),
        });
    }

    result.passed = result.checks.iter().all(|check| check.passed);
    result
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
