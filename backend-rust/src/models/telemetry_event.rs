use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// One row in the `telemetry_events` TimescaleDB hypertable — queried
/// against `AppState::timescale_db`, NOT the main `db` pool. `event_type` is
/// an open string (not a Rust enum) so a new collector added in Phase 3
/// needs no schema/model change here, only a new value on the wire.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct TelemetryEvent {
    pub id: Uuid,
    pub session_id: Uuid,
    pub roll_number: String,
    pub extension_instance_id: Uuid,
    pub event_type: String,
    pub event_data: serde_json::Value,
    pub recorded_at: DateTime<Utc>,
    pub received_at: DateTime<Utc>,
}

impl TelemetryEvent {
    pub fn table_name() -> &'static str {
        "telemetry_events"
    }
}
