//! Phase 2 of the session-monitoring feature: the batched ingestion endpoint
//! the (Phase 3) browser extension will POST to, plus the Redis pub/sub
//! fan-out a future live admin dashboard (Phase 4) will subscribe to. No
//! pairing flow exists yet (that's Phase 3), so a batch is only ever
//! accepted against a `session_device_locks` row created directly — by a
//! test, or later by the pairing flow itself.

use axum::{
    extract::{Json, Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::{DateTime, Utc};
use redis::AsyncCommands;
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    controllers::public_webauthn::load_active_short_link_and_session,
    error::{AppError, Result},
    models::SessionDeviceLock,
    AppState,
};

/// A single client-observed event within a batch. `event_type` is an open
/// string, not a Rust enum — see `models::TelemetryEvent`'s doc comment for
/// why.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TelemetryEventInput {
    pub event_type: String,
    #[serde(default)]
    pub event_data: serde_json::Value,
    /// When the extension observed the event (client clock).
    pub recorded_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IngestTelemetryRequest {
    pub extension_instance_id: Uuid,
    pub roll_number: String,
    pub events: Vec<TelemetryEventInput>,
}

/// Matches the extension's local batching window (15-30s) at a reasonable
/// event rate — generous enough for a real batch, tight enough that one
/// request can't be used to dump an unbounded amount of data.
const MAX_BATCH_SIZE: usize = 200;

/// `POST /{shortCode}/extension/events` — batched telemetry ingestion.
/// Rejects the batch outright unless it matches the session's current active
/// device lock for that student (see migrations/0006_extension_device_locks),
/// so a mismatched or unpaired device can never write telemetry.
pub async fn ingest_telemetry(
    State(state): State<Arc<AppState>>,
    Path(short_code): Path<String>,
    Json(payload): Json<IngestTelemetryRequest>,
) -> Result<impl IntoResponse> {
    if payload.events.is_empty() {
        return Err(AppError::BadRequest("No events in batch".to_string()));
    }
    if payload.events.len() > MAX_BATCH_SIZE {
        return Err(AppError::BadRequest(format!(
            "Batch too large: {} events (max {})",
            payload.events.len(),
            MAX_BATCH_SIZE
        )));
    }

    let (_short_link, session) = load_active_short_link_and_session(&state.db, &short_code).await?;

    let roll_number = payload.roll_number.trim().to_uppercase();
    if roll_number.is_empty() {
        return Err(AppError::BadRequest("rollNumber is required".to_string()));
    }

    let lock: Option<SessionDeviceLock> = sqlx::query_as(
        "SELECT * FROM session_device_locks \
         WHERE session_id = $1 AND roll_number = $2 AND status = 'active'",
    )
    .bind(session.id)
    .bind(&roll_number)
    .fetch_optional(&state.db)
    .await?;

    let Some(lock) = lock else {
        return Err(AppError::Forbidden(
            "No active device lock for this student in this session".to_string(),
        ));
    };

    if lock.extension_instance_id != payload.extension_instance_id {
        return Err(AppError::Forbidden(
            "This device is not the one locked to this session for this student".to_string(),
        ));
    }

    let mut tx = state.timescale_db.begin().await?;
    let mut builder = sqlx::QueryBuilder::new(
        "INSERT INTO telemetry_events \
         (id, session_id, roll_number, extension_instance_id, event_type, event_data, recorded_at, received_at) ",
    );
    builder.push_values(&payload.events, |mut row, event| {
        row.push_bind(Uuid::new_v4())
            .push_bind(session.id)
            .push_bind(&roll_number)
            .push_bind(payload.extension_instance_id)
            .push_bind(&event.event_type)
            .push_bind(&event.event_data)
            .push_bind(event.recorded_at)
            .push("now()");
    });
    builder.build().execute(&mut *tx).await?;
    tx.commit().await?;

    // Best-effort live fan-out for a future admin dashboard (Phase 4) to
    // subscribe to while a student's panel is open. A publish failure here
    // must never fail the request — the durable write above already
    // succeeded, and a missed live update just means the dashboard catches
    // up on its next poll/reconnect rather than losing data.
    if let Ok(mut conn) = state.redis.get_multiplexed_async_connection().await {
        let summary = serde_json::json!({
            "sessionId": session.id,
            "rollNumber": roll_number,
            "eventCount": payload.events.len(),
        });
        let channel = format!("session:{}:telemetry", session.id);
        let _: std::result::Result<(), _> = conn.publish(&channel, summary.to_string()).await;
    }

    Ok((
        StatusCode::ACCEPTED,
        Json(serde_json::json!({ "accepted": payload.events.len() })),
    ))
}
