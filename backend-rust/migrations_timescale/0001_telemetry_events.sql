-- Runs against the dedicated TimescaleDB instance (TIMESCALE_DATABASE_URL),
-- NOT the main application database — see AppState::timescale_db /
-- AppConfig::timescale_database_url. This is the only table in this
-- migration set for Phase 2; the extension's actual capability set (which
-- `event_type` values get written) is a Phase 3 decision, not a schema one —
-- `event_type`/`event_data` are intentionally open-ended so adding a new
-- collector later needs no migration here.

CREATE EXTENSION IF NOT EXISTS timescaledb;

CREATE TABLE telemetry_events (
    id UUID NOT NULL DEFAULT gen_random_uuid(),
    -- References `sessions.id` / `session_device_locks.extension_instance_id`
    -- on the MAIN database — no real FK is possible across databases, so
    -- these are validated at the application layer (the ingestion endpoint
    -- checks the lock exists before writing here) rather than by Postgres.
    session_id UUID NOT NULL,
    roll_number TEXT NOT NULL,
    extension_instance_id UUID NOT NULL,
    event_type TEXT NOT NULL,
    event_data JSONB NOT NULL DEFAULT '{}',
    -- When the extension observed the event (client clock); `received_at`
    -- below is when this row was written (server clock) — kept separate so
    -- ingestion-batching delay never gets confused with the event's real time.
    recorded_at TIMESTAMPTZ NOT NULL,
    received_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- TimescaleDB requires the partitioning column in every unique
    -- constraint, including the primary key.
    PRIMARY KEY (id, recorded_at)
);

-- Partitions by `recorded_at` so both writes (always near "now") and reads
-- scoped to a session's actual time window only ever touch a handful of
-- recent chunks, not the whole table.
SELECT create_hypertable('telemetry_events', 'recorded_at');

CREATE INDEX idx_telemetry_events_session ON telemetry_events (session_id, recorded_at DESC);

-- Raw per-event rows older than 90 days are dropped automatically — nothing
-- about this feature needs longer-than-a-term raw retention, and letting
-- chunks age out keeps storage bounded without a manual cleanup job.
SELECT add_retention_policy('telemetry_events', INTERVAL '90 days');
