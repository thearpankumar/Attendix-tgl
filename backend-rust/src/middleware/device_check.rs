use axum::{body::Body, extract::State, http::Request, middleware::Next, response::Response};
use std::sync::Arc;
use uuid::Uuid;

use crate::error::Result;
use crate::models::{Device, DeviceFlagEntry};

#[derive(Debug, Clone)]
pub struct DeviceCheckResult {
    pub first_seen: bool,
    pub flags: Vec<String>,
    pub device_flag: Option<String>,
}

pub async fn device_check_middleware(
    State(state): State<Arc<crate::AppState>>,
    request: Request<Body>,
    next: Next,
) -> Result<Response> {
    if state.config.node_env == "test" {
        let mut req = Request::new(Body::empty());
        req.extensions_mut().insert(DeviceCheckResult {
            first_seen: true,
            flags: vec![],
            device_flag: None,
        });
        return Ok(next.run(req).await);
    }

    let (parts, body) = request.into_parts();
    let bytes = axum::body::to_bytes(body, 1024 * 1024)
        .await
        .map_err(|e| crate::error::AppError::BadRequest(format!("Failed to read body: {}", e)))?;

    let parsed: Option<serde_json::Value> = serde_json::from_slice(&bytes).ok();

    let device_fingerprint = parsed
        .as_ref()
        .and_then(|v| v.get("deviceFingerprint")?.as_str().map(|s| s.to_string()));

    let roll_number = parsed
        .as_ref()
        .and_then(|v| v.get("rollNumber")?.as_str().map(|s| s.to_string()));

    let session_id = parsed
        .as_ref()
        .and_then(|v| v.get("sessionId")?.as_str())
        .and_then(|s| Uuid::parse_str(s).ok());

    let Some(device_fingerprint) = device_fingerprint else {
        let mut req = Request::from_parts(parts, Body::from(bytes));
        req.extensions_mut().insert(DeviceCheckResult {
            first_seen: true,
            flags: vec!["NO_DEVICE_FINGERPRINT".to_string()],
            device_flag: None,
        });
        return Ok(next.run(req).await);
    };

    let fingerprint_hash = Device::hash_fingerprint(&device_fingerprint);

    let mut result = DeviceCheckResult {
        first_seen: false,
        flags: vec![],
        device_flag: None,
    };

    let roll_upper = roll_number
        .as_ref()
        .map(|r| r.to_uppercase())
        .unwrap_or_default();

    if let (Some(session_id), Some(_roll_number)) = (session_id, roll_number.as_ref()) {
        let existing_device = sqlx::query_as::<_, Device>(
            "SELECT * FROM devices WHERE fingerprint_hash = $1 AND session_id = $2",
        )
        .bind(&fingerprint_hash)
        .bind(session_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();

        if let Some(mut device) = existing_device {
            device.last_seen_at = Some(chrono::Utc::now());
            device.attendance_count += 1;

            if let Some(ref bound) = device.bound_to_student {
                if bound != &roll_upper {
                    result.flags.push("MULTI_STUDENT_DEVICE".to_string());
                    result.device_flag = Some("MULTI_STUDENT_DEVICE".to_string());

                    device.flags.0.push(DeviceFlagEntry {
                        flag_type: "MULTI_STUDENT_DEVICE".to_string(),
                        details: Some(format!(
                            "Device previously used by {}, now {}",
                            bound, roll_upper
                        )),
                        session_id: Some(session_id),
                        timestamp: chrono::Utc::now(),
                    });
                }
            }

            if let Err(e) = sqlx::query(
                "UPDATE devices SET last_seen_at = $1, attendance_count = $2, flags = $3 WHERE id = $4",
            )
            .bind(device.last_seen_at)
            .bind(device.attendance_count)
            .bind(&device.flags)
            .bind(device.id)
            .execute(&state.db)
            .await
            {
                tracing::warn!("Failed to update device: {}", e);
            }
        } else {
            let student_existing_device = sqlx::query_as::<_, Device>(
                "SELECT * FROM devices WHERE bound_to_student = $1 AND session_id = $2",
            )
            .bind(&roll_upper)
            .bind(session_id)
            .fetch_optional(&state.db)
            .await
            .ok()
            .flatten();

            if let Some(prev_device) = student_existing_device {
                if prev_device.fingerprint_hash != fingerprint_hash {
                    result.flags.push("STUDENT_DEVICE_SWITCHED".to_string());
                    result.device_flag = Some("STUDENT_DEVICE_SWITCHED".to_string());
                }
            }

            let user_agent = parts
                .headers
                .get("user-agent")
                .and_then(|h| h.to_str().ok())
                .map(|s| s.to_string());

            let new_device = Device::new(
                fingerprint_hash.clone(),
                roll_upper.clone(),
                session_id,
                user_agent,
            );

            if let Err(e) = sqlx::query(
                "INSERT INTO devices \
                    (id, fingerprint_hash, bound_to_student, session_id, first_seen_at, last_seen_at, \
                     attendance_count, flags, metadata, successful_submissions, failed_submissions, \
                     spoofing_attempts, is_blocked, block_reason, blocked_at) \
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15)",
            )
            .bind(new_device.id)
            .bind(&new_device.fingerprint_hash)
            .bind(&new_device.bound_to_student)
            .bind(new_device.session_id)
            .bind(new_device.first_seen_at)
            .bind(new_device.last_seen_at)
            .bind(new_device.attendance_count)
            .bind(&new_device.flags)
            .bind(&new_device.metadata)
            .bind(new_device.successful_submissions)
            .bind(new_device.failed_submissions)
            .bind(new_device.spoofing_attempts)
            .bind(new_device.is_blocked)
            .bind(&new_device.block_reason)
            .bind(new_device.blocked_at)
            .execute(&state.db)
            .await
            {
                tracing::warn!("Failed to insert device: {}", e);
            }
            result.first_seen = true;
        }

        let ten_seconds_ago = chrono::Utc::now() - chrono::Duration::seconds(10);

        let recent_attendance: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM attendances WHERE session_id = $1 AND roll_number = $2 AND captured_at >= $3 LIMIT 1",
        )
        .bind(session_id)
        .bind(&roll_upper)
        .bind(ten_seconds_ago)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();

        if recent_attendance.is_some() {
            result.flags.push("RAPID_SUBMISSION".to_string());
            result.device_flag = Some("RAPID_SUBMISSION".to_string());
        }
    }

    let mut req = Request::from_parts(parts, Body::from(bytes));
    req.extensions_mut().insert(result);

    Ok(next.run(req).await)
}
