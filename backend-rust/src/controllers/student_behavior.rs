//! Phase 4 of session monitoring: the admin-facing "view student behavior"
//! endpoints — a summary pulled from `session_device_locks` (main DB) +
//! `telemetry_events` (TimescaleDB), and a live WebSocket view that forwards
//! the Redis pub/sub fan-out `controllers::telemetry::ingest_telemetry`
//! already publishes to (`session:{id}:telemetry`).

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Extension, Path, State,
    },
    response::IntoResponse,
    Json,
};
use chrono::{DateTime, Utc};
use futures::StreamExt;
use serde::Serialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    controllers::session::find_session_for_admin,
    error::{AppError, Result},
    middleware::AuthenticatedAdmin,
    models::{SessionDeviceLock, TelemetryEvent},
    AppState,
};

/// Capped so a long-running session's summary endpoint stays cheap — the
/// derived `BehaviorSummary` below is computed from this same capped window,
/// not the full history, so it reflects "recent behavior" rather than the
/// entire session once a session runs long enough to exceed the cap.
const MAX_EVENTS_RETURNED: i64 = 500;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceLockSummary {
    pub id: String,
    pub extension_instance_id: String,
    pub status: String,
    pub locked_at: DateTime<Utc>,
    pub released_at: Option<DateTime<Utc>>,
    pub superseded_by: Option<String>,
}

impl From<SessionDeviceLock> for DeviceLockSummary {
    fn from(l: SessionDeviceLock) -> Self {
        Self {
            id: l.id.to_string(),
            extension_instance_id: l.extension_instance_id.to_string(),
            status: l.status,
            locked_at: l.locked_at,
            released_at: l.released_at,
            superseded_by: l.superseded_by.map(|id| id.to_string()),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TelemetryEventOut {
    pub event_type: String,
    pub event_data: serde_json::Value,
    pub recorded_at: DateTime<Utc>,
}

impl From<TelemetryEvent> for TelemetryEventOut {
    fn from(e: TelemetryEvent) -> Self {
        Self {
            event_type: e.event_type,
            event_data: e.event_data,
            recorded_at: e.recorded_at,
        }
    }
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BehaviorSummary {
    pub total_idle_seconds: i64,
    pub focus_transition_count: i64,
    pub tab_switch_count: i64,
    pub meet_tab_ever_open: bool,
    pub event_count: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StudentBehaviorResponse {
    pub roll_number: String,
    pub device_locks: Vec<DeviceLockSummary>,
    pub summary: BehaviorSummary,
    /// Chronological (oldest first) — a timeline UI can render this directly
    /// without re-sorting.
    pub events: Vec<TelemetryEventOut>,
}

/// Derives idle time / focus-transition / tab-switch counts from the raw
/// event stream. Field-name/shape assumptions here are pinned to exactly
/// what `extension/entrypoints/background.ts` sends today:
/// - `idle_state` → `{ state: "active" | "idle" | "locked" }`
/// - `window_focus` → `{ focused: bool }` (one row per transition)
/// - `tab_url_change` → fired on every tab activation/navigation, so its
///   count is the best available proxy for "tab switches" (there is no
///   dedicated tab-switch event type)
/// - `meet_tab_open` → `{ open: bool }`
fn compute_summary(events_chronological: &[TelemetryEvent]) -> BehaviorSummary {
    let mut total_idle_seconds = 0i64;
    let mut idle_since: Option<DateTime<Utc>> = None;
    let mut focus_transition_count = 0i64;
    let mut tab_switch_count = 0i64;
    let mut meet_tab_ever_open = false;

    for event in events_chronological {
        match event.event_type.as_str() {
            "idle_state" => {
                let state = event
                    .event_data
                    .get("state")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                match state {
                    "idle" | "locked" => idle_since.get_or_insert(event.recorded_at),
                    "active" => {
                        if let Some(start) = idle_since.take() {
                            total_idle_seconds += (event.recorded_at - start).num_seconds().max(0);
                        }
                        continue;
                    }
                    _ => continue,
                };
            }
            "window_focus" => focus_transition_count += 1,
            "tab_url_change" => tab_switch_count += 1,
            "meet_tab_open"
                if event
                    .event_data
                    .get("open")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false) =>
            {
                meet_tab_ever_open = true;
            }
            _ => {}
        }
    }

    BehaviorSummary {
        total_idle_seconds,
        focus_transition_count,
        tab_switch_count,
        meet_tab_ever_open,
        event_count: events_chronological.len() as i64,
    }
}

/// `GET /api/admin/sessions/{id}/students/{rollNumber}/behavior` — role-
/// scoped the same way every other session sub-resource is (super-admins see
/// any session, mentors only ones assigned to them via `find_session_for_admin`).
pub async fn get_student_behavior(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Path((id, roll_number)): Path<(String, String)>,
) -> Result<impl IntoResponse> {
    let session_id = Uuid::parse_str(&id)
        .map_err(|e| AppError::BadRequest(format!("Invalid session ID: {}", e)))?;
    // Ownership/role check — discarded on purpose, we only need session_id below.
    find_session_for_admin(&state.db, session_id, &auth).await?;

    let roll_upper = roll_number.trim().to_uppercase();

    let locks: Vec<SessionDeviceLock> = sqlx::query_as(
        "SELECT * FROM session_device_locks WHERE session_id = $1 AND roll_number = $2 \
         ORDER BY locked_at ASC",
    )
    .bind(session_id)
    .bind(&roll_upper)
    .fetch_all(&state.db)
    .await?;

    let mut events: Vec<TelemetryEvent> = sqlx::query_as(
        "SELECT * FROM telemetry_events WHERE session_id = $1 AND roll_number = $2 \
         ORDER BY recorded_at DESC LIMIT $3",
    )
    .bind(session_id)
    .bind(&roll_upper)
    .bind(MAX_EVENTS_RETURNED)
    .fetch_all(&state.timescale_db)
    .await?;
    // Fetched newest-first (so the LIMIT caps the *most recent* window);
    // both the summary and the response are chronological.
    events.reverse();

    let summary = compute_summary(&events);

    Ok(Json(StudentBehaviorResponse {
        roll_number: roll_upper,
        device_locks: locks.into_iter().map(DeviceLockSummary::from).collect(),
        summary,
        events: events.into_iter().map(TelemetryEventOut::from).collect(),
    }))
}

/// `GET /api/admin/sessions/{id}/students/{rollNumber}/behavior/live` —
/// upgrades to a WebSocket that forwards `session:{id}:telemetry` Redis
/// pub/sub messages for this student only, for as long as the socket stays
/// open. The role/ownership check happens here, before the upgrade — once
/// upgraded there's no further HTTP request to gate.
pub async fn student_behavior_live(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Path((id, roll_number)): Path<(String, String)>,
) -> Result<impl IntoResponse> {
    let session_id = Uuid::parse_str(&id)
        .map_err(|e| AppError::BadRequest(format!("Invalid session ID: {}", e)))?;
    find_session_for_admin(&state.db, session_id, &auth).await?;

    let roll_upper = roll_number.trim().to_uppercase();

    Ok(ws.on_upgrade(move |socket| handle_behavior_socket(socket, state, session_id, roll_upper)))
}

async fn handle_behavior_socket(
    mut socket: WebSocket,
    state: Arc<AppState>,
    session_id: Uuid,
    roll_number: String,
) {
    let channel = format!("session:{}:telemetry", session_id);

    let mut pubsub = match state.redis.get_async_pubsub().await {
        Ok(p) => p,
        Err(_) => return,
    };
    if pubsub.subscribe(&channel).await.is_err() {
        return;
    }
    let mut messages = pubsub.into_on_message();

    // Ends the moment the admin closes the panel (socket close) or the
    // connection drops — nothing here holds the Redis subscription open past
    // that, keeping this to "no idle sockets for the other 499 students",
    // per the plan's transport design.
    loop {
        tokio::select! {
            redis_msg = messages.next() => {
                let Some(msg) = redis_msg else { break };
                let Ok(payload) = msg.get_payload::<String>() else { continue };
                // The channel is per-session, not per-student — filter here
                // so this socket only ever forwards events for the roll
                // number it was opened for.
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&payload) {
                    if parsed.get("rollNumber").and_then(|r| r.as_str()) != Some(roll_number.as_str()) {
                        continue;
                    }
                }
                if socket.send(Message::Text(payload.into())).await.is_err() {
                    break;
                }
            }
            client_msg = socket.recv() => {
                match client_msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Err(_)) => break,
                    _ => {}
                }
            }
        }
    }
}
