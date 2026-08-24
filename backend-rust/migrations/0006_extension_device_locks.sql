-- State tables for the browser-extension monitoring feature (Phase 2): which
-- physical extension install belongs to a student, and which one is
-- currently locked to a given session. Small, relational, low-write-volume
-- data — stays on the main database with real foreign keys, unlike the
-- high-volume `telemetry_events` hypertable (see migrations_timescale/),
-- which lives in a separate TimescaleDB instance.

-- The durable "which physical install is this" anchor: a UUID the extension
-- generates once and persists in its own storage. Stronger than device
-- fingerprinting alone, since fingerprints can coincidentally match across
-- different physical devices.
CREATE TABLE extension_installations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    roll_number TEXT NOT NULL,
    extension_instance_id UUID NOT NULL UNIQUE,
    first_seen_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    browser TEXT,
    os TEXT,
    is_active BOOLEAN NOT NULL DEFAULT true
);
CREATE INDEX idx_extension_installations_roll_number ON extension_installations (roll_number);

-- One *active* row per (session, roll_number): the device currently allowed
-- to report telemetry for that student in that session. Re-pairing on a new
-- device (Phase 3) writes a fresh row and marks the old one 'superseded'
-- rather than updating in place, so the switch is an audit trail, not a
-- silent overwrite.
CREATE TABLE session_device_locks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id UUID NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    roll_number TEXT NOT NULL,
    extension_instance_id UUID NOT NULL,
    webauthn_credential_id UUID REFERENCES webauthn_credentials(id),
    device_fingerprint_hash TEXT,
    locked_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    released_at TIMESTAMPTZ,
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'released', 'superseded')),
    superseded_by UUID REFERENCES session_device_locks(id)
);
CREATE INDEX idx_session_device_locks_session ON session_device_locks (session_id);
-- Enforces "one active lock per student per session" at the database level,
-- not just in application code.
CREATE UNIQUE INDEX idx_session_device_locks_active ON session_device_locks (session_id, roll_number)
    WHERE status = 'active';
