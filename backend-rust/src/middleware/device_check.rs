use axum::{body::Body, extract::State, http::Request, middleware::Next, response::Response};
use mongodb::bson::{doc, oid::ObjectId, Bson, DateTime as BsonDateTime};
use mongodb::Collection;
use std::sync::Arc;

use crate::error::Result;
use crate::models::{Attendance, Device, DeviceFlagEntry, DeviceMetadata};

#[derive(Debug, Clone)]
pub struct DeviceCheckResult {
    pub first_seen: bool,
    pub flags: Vec<String>,
    pub device_flag: Option<String>,
}

fn to_bson_datetime(dt: chrono::DateTime<chrono::Utc>) -> BsonDateTime {
    BsonDateTime::from_millis(dt.timestamp_millis())
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
        .and_then(|s| ObjectId::parse_str(s).ok());

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
    let db_name = state
        .config
        .mongodb_uri
        .split('/')
        .next_back()
        .unwrap_or("default");

    let collection: Collection<Device> = state
        .db
        .database(db_name)
        .collection(Device::collection_name());

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
        let existing_device = collection
            .find_one(doc! {
                "fingerprintHash": &fingerprint_hash,
                "sessionId": session_id,
            })
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

                    device.flags.push(DeviceFlagEntry {
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

            let flags_bson = Bson::Array(
                device
                    .flags
                    .iter()
                    .map(|f| {
                        Bson::Document(doc! {
                            "type": &f.flag_type,
                            "timestamp": to_bson_datetime(f.timestamp),
                            "details": &f.details,
                            "sessionId": f.session_id,
                        })
                    })
                    .collect(),
            );

            if let Err(e) = collection
                .update_one(
                    doc! { "_id": device.id },
                    doc! {
                        "$set": {
                            "lastSeenAt": to_bson_datetime(device.last_seen_at.unwrap_or_else(chrono::Utc::now)),
                            "attendanceCount": device.attendance_count,
                            "flags": flags_bson,
                        }
                    },
                )
                .await
            {
                tracing::warn!("Failed to update device: {}", e);
            }
        } else {
            let student_existing_device = collection
                .find_one(doc! {
                    "boundToStudent": &roll_upper,
                    "sessionId": session_id,
                })
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

            let new_device = Device {
                id: None,
                fingerprint_hash: fingerprint_hash.clone(),
                bound_to_student: Some(roll_upper.clone()),
                session_id: Some(session_id),
                first_seen_at: chrono::Utc::now(),
                last_seen_at: Some(chrono::Utc::now()),
                attendance_count: 1,
                flags: vec![],
                metadata: Some(DeviceMetadata {
                    user_agent,
                    platform: None,
                    browser: None,
                }),
                successful_submissions: 0,
                failed_submissions: 0,
                spoofing_attempts: 0,
                is_blocked: false,
                block_reason: None,
                blocked_at: None,
            };

            if let Err(e) = collection.insert_one(&new_device).await {
                tracing::warn!("Failed to insert device: {}", e);
            }
            result.first_seen = true;
        }

        let attendances: Collection<Attendance> = state
            .db
            .database(db_name)
            .collection(Attendance::collection_name());

        let ten_seconds_ago = chrono::Utc::now() - chrono::Duration::seconds(10);

        let recent_attendance = attendances
            .find_one(doc! {
                "sessionId": session_id,
                "rollNumber": &roll_upper,
                "capturedAt": { "$gte": to_bson_datetime(ten_seconds_ago) },
            })
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
