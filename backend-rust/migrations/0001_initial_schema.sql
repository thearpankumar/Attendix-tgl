-- Initial Postgres schema, replacing the MongoDB collections.
-- Relational tables for core entities; JSONB for nested/variable blobs
-- that were embedded documents/arrays in Mongo and aren't queried relationally.

CREATE TABLE admins (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    username TEXT NOT NULL,
    email TEXT NOT NULL,
    password TEXT NOT NULL,
    role TEXT NOT NULL DEFAULT 'admin',
    full_name TEXT,
    college_name TEXT,
    is_active BOOLEAN NOT NULL DEFAULT true,
    failed_login_attempts INT NOT NULL DEFAULT 0,
    lock_until TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX idx_admins_username ON admins (username);
CREATE UNIQUE INDEX idx_admins_email ON admins (email);

CREATE TABLE locations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    latitude DOUBLE PRECISION NOT NULL,
    longitude DOUBLE PRECISION NOT NULL,
    radius_meters DOUBLE PRECISION NOT NULL DEFAULT 100.0,
    description TEXT,
    created_by UUID NOT NULL REFERENCES admins(id),
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_locations_created_by ON locations (created_by);

CREATE TABLE batches (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    description TEXT,
    created_by UUID NOT NULL REFERENCES admins(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_batches_created_by ON batches (created_by);

CREATE TABLE students (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    batch_id UUID NOT NULL REFERENCES batches(id) ON DELETE CASCADE,
    position INT NOT NULL DEFAULT 0,
    name TEXT NOT NULL,
    roll_number TEXT NOT NULL,
    college_name TEXT,
    email TEXT
);
CREATE INDEX idx_students_batch_id ON students (batch_id, position);
CREATE INDEX idx_students_roll_number ON students (roll_number);

CREATE TABLE sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    -- Nullable: only self-service sessions are geofenced. Exam sessions
    -- (assigned to mentors via session_admins below) are marked manually and
    -- have no location.
    location_id UUID REFERENCES locations(id),
    batch_id UUID REFERENCES batches(id),
    token_hash TEXT NOT NULL,
    token_prefix TEXT NOT NULL,
    description TEXT,
    created_by UUID NOT NULL REFERENCES admins(id),
    college_name TEXT,
    starts_at TIMESTAMPTZ,
    is_active BOOLEAN NOT NULL DEFAULT true,
    expires_at TIMESTAMPTZ NOT NULL,
    rotation_count INT NOT NULL DEFAULT 0,
    totp_secret TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_sessions_created_by_created_at ON sessions (created_by, created_at DESC);
CREATE INDEX idx_sessions_location_id ON sessions (location_id);
CREATE INDEX idx_sessions_batch_id ON sessions (batch_id);
CREATE INDEX idx_sessions_token_hash ON sessions (token_hash);

-- An exam session can be assigned to more than one mentor (was a single
-- assigned_admin_id FK on sessions).
CREATE TABLE session_admins (
    session_id UUID NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    admin_id UUID NOT NULL REFERENCES admins(id) ON DELETE CASCADE,
    PRIMARY KEY (session_id, admin_id)
);
CREATE INDEX idx_session_admins_admin_id ON session_admins (admin_id);

CREATE TABLE attendances (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id UUID NOT NULL REFERENCES sessions(id),
    student_name TEXT NOT NULL,
    roll_number TEXT NOT NULL,
    photo_url TEXT NOT NULL,
    photo_public_id TEXT NOT NULL,
    photo_hash TEXT,
    photo_reuse_detected BOOLEAN NOT NULL DEFAULT false,
    student_latitude DOUBLE PRECISION NOT NULL,
    student_longitude DOUBLE PRECISION NOT NULL,
    distance_from_location DOUBLE PRECISION NOT NULL,
    ip_address TEXT,
    user_agent TEXT,
    network_provider TEXT,
    network_org TEXT,
    verified BOOLEAN NOT NULL DEFAULT false,
    source TEXT NOT NULL DEFAULT 'self_submitted',
    status TEXT NOT NULL DEFAULT 'present',
    marked_by_admin_id UUID REFERENCES admins(id),
    face_detected BOOLEAN NOT NULL DEFAULT true,
    device_fingerprint TEXT,
    device_fingerprint_hash TEXT,
    device_first_seen BOOLEAN NOT NULL DEFAULT false,
    totp_code TEXT,
    totp_valid BOOLEAN,
    device_flag TEXT,
    webauthn_credential_id TEXT,
    webauthn_verified BOOLEAN NOT NULL DEFAULT false,
    webauthn_device_type TEXT,
    webauthn_authenticator_attachment TEXT,
    webauthn_counter INT,
    webauthn_replay_attack BOOLEAN NOT NULL DEFAULT false,
    flag_reviewed BOOLEAN NOT NULL DEFAULT false,
    flag_reviewed_by UUID REFERENCES admins(id),
    flag_reviewed_at TIMESTAMPTZ,
    flagged BOOLEAN NOT NULL DEFAULT false,
    flag_reason TEXT,
    flag_details TEXT,
    captured_at TIMESTAMPTZ NOT NULL,
    gps_accuracy DOUBLE PRECISION,
    gps_altitude DOUBLE PRECISION,
    gps_altitude_accuracy DOUBLE PRECISION,
    gps_speed DOUBLE PRECISION,
    gps_heading DOUBLE PRECISION,
    gps_timestamp BIGINT,
    gps_mock_location BOOLEAN NOT NULL DEFAULT false,
    gps_provider TEXT,
    gps_anomalies JSONB NOT NULL DEFAULT '[]',
    gps_confidence TEXT,
    emulator_detected BOOLEAN NOT NULL DEFAULT false,
    emulator_flags JSONB NOT NULL DEFAULT '[]',
    integrity_checks JSONB NOT NULL DEFAULT '[]',
    CONSTRAINT chk_attendances_source CHECK (source IN ('self_submitted', 'manual')),
    CONSTRAINT chk_attendances_status CHECK (status IN ('present', 'absent'))
);
CREATE UNIQUE INDEX idx_attendances_session_roll ON attendances (session_id, roll_number);
CREATE INDEX idx_attendances_session_captured ON attendances (session_id, captured_at DESC);
CREATE INDEX idx_attendances_flagged_session ON attendances (flagged, session_id);

CREATE TABLE devices (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    fingerprint_hash TEXT NOT NULL,
    bound_to_student TEXT,
    session_id UUID REFERENCES sessions(id),
    first_seen_at TIMESTAMPTZ NOT NULL,
    last_seen_at TIMESTAMPTZ,
    attendance_count INT NOT NULL DEFAULT 0,
    flags JSONB NOT NULL DEFAULT '[]',
    metadata JSONB,
    successful_submissions INT NOT NULL DEFAULT 0,
    failed_submissions INT NOT NULL DEFAULT 0,
    spoofing_attempts INT NOT NULL DEFAULT 0,
    is_blocked BOOLEAN NOT NULL DEFAULT false,
    block_reason TEXT,
    blocked_at TIMESTAMPTZ
);
CREATE INDEX idx_devices_fingerprint_session ON devices (fingerprint_hash, session_id);
CREATE INDEX idx_devices_bound_session ON devices (bound_to_student, session_id);

CREATE TABLE device_fingerprints (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    fingerprint_id TEXT NOT NULL,
    first_seen TIMESTAMPTZ NOT NULL,
    last_seen TIMESTAMPTZ NOT NULL,
    verification_failures INT NOT NULL DEFAULT 0,
    spoofing_attempts INT NOT NULL DEFAULT 0,
    last_spoofing_reason TEXT,
    inconsistencies TEXT[] NOT NULL DEFAULT '{}',
    claimed_device_types TEXT[] NOT NULL DEFAULT '{}',
    user_agents_seen JSONB NOT NULL DEFAULT '[]',
    sessions JSONB NOT NULL DEFAULT '[]',
    is_trusted BOOLEAN NOT NULL DEFAULT false,
    is_blocked BOOLEAN NOT NULL DEFAULT false,
    block_reason TEXT,
    last_metrics JSONB
);
CREATE UNIQUE INDEX idx_device_fingerprints_fingerprint_id ON device_fingerprints (fingerprint_id);

CREATE TABLE flags (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    flag_type TEXT NOT NULL,
    admin_id UUID REFERENCES admins(id),
    student_id UUID,
    details TEXT,
    session_id UUID REFERENCES sessions(id),
    "timestamp" TIMESTAMPTZ NOT NULL,
    resolved BOOLEAN NOT NULL DEFAULT false,
    resolved_by UUID REFERENCES admins(id),
    resolved_at TIMESTAMPTZ
);
CREATE INDEX idx_flags_session_id ON flags (session_id);
CREATE INDEX idx_flags_resolved ON flags (resolved);

CREATE TABLE photo_hashes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    roll_number TEXT NOT NULL,
    photo_hash TEXT NOT NULL,
    session_id UUID NOT NULL REFERENCES sessions(id),
    captured_at TIMESTAMPTZ NOT NULL,
    confidence DOUBLE PRECISION
);
CREATE INDEX idx_photo_hashes_session_id ON photo_hashes (session_id);
CREATE INDEX idx_photo_hashes_roll_number ON photo_hashes (roll_number);
-- Reuse detection compares a submitted photo against a rolling time window
-- across sessions (not just the current one), so it needs to range-scan by
-- recency in addition to the roll-number/session lookups above.
CREATE INDEX idx_photo_hashes_captured_at ON photo_hashes (captured_at);

CREATE TABLE short_links (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    short_code TEXT NOT NULL,
    session_id UUID REFERENCES sessions(id),
    created_by UUID NOT NULL REFERENCES admins(id),
    is_active BOOLEAN NOT NULL DEFAULT true,
    expires_at TIMESTAMPTZ,
    click_count INT NOT NULL DEFAULT 0,
    last_clicked_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX idx_short_links_short_code ON short_links (short_code);

-- Singleton row holding all nested config groups (gps_validation, emulator_detection,
-- trust_score, rate_limits, webauthn_config, photo_verification, session_config,
-- lockout_config, attendance_config) as a single JSONB blob, matching the previous
-- SystemConfig document shape.
CREATE TABLE system_configs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    dev_bypass_enabled BOOLEAN NOT NULL DEFAULT false,
    config JSONB NOT NULL DEFAULT '{}',
    updated_by UUID REFERENCES admins(id),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE webauthn_challenges (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    student_id TEXT NOT NULL,
    challenge TEXT NOT NULL,
    challenge_type TEXT NOT NULL,
    session_id UUID NOT NULL REFERENCES sessions(id),
    short_code TEXT,
    student_name TEXT,
    expires_at TIMESTAMPTZ NOT NULL,
    used BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
-- Mongo's TTL index auto-expired rows 300s after expires_at; Postgres has no
-- native TTL index, so this index backs a periodic cleanup DELETE instead
-- (see main.rs startup task).
CREATE INDEX idx_webauthn_challenges_expires_at ON webauthn_challenges (expires_at);
CREATE INDEX idx_webauthn_challenges_student_id ON webauthn_challenges (student_id);

CREATE TABLE webauthn_credentials (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    student_id TEXT NOT NULL,
    credential_id TEXT NOT NULL,
    public_key BYTEA NOT NULL,
    counter BIGINT NOT NULL DEFAULT 0,
    device_label TEXT NOT NULL DEFAULT 'Unknown Device',
    device_type TEXT NOT NULL DEFAULT 'multi_device',
    transports TEXT[] NOT NULL DEFAULT '{}',
    enrolled_at TIMESTAMPTZ NOT NULL,
    enrolled_ip_address TEXT,
    enrolled_user_agent TEXT,
    created_by_admin_id UUID REFERENCES admins(id),
    sign_count BIGINT NOT NULL DEFAULT 0,
    last_used_at TIMESTAMPTZ,
    last_session_id UUID REFERENCES sessions(id),
    is_suspended BOOLEAN NOT NULL DEFAULT false,
    suspended_reason TEXT,
    suspended_at TIMESTAMPTZ,
    suspended_by UUID REFERENCES admins(id),
    aaguid TEXT,
    reset_at TIMESTAMPTZ,
    reset_by UUID REFERENCES admins(id)
);
CREATE UNIQUE INDEX idx_webauthn_credentials_student_id ON webauthn_credentials (student_id);
CREATE UNIQUE INDEX idx_webauthn_credentials_credential_id ON webauthn_credentials (credential_id);

CREATE TABLE webauthn_reenrollment_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    student_id TEXT NOT NULL,
    admin_id UUID NOT NULL REFERENCES admins(id),
    reason TEXT,
    previous_credential_id TEXT,
    new_credential_id TEXT,
    action_type TEXT NOT NULL,
    "timestamp" TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_webauthn_reenrollment_logs_student_id ON webauthn_reenrollment_logs (student_id);

-- Tamper-evident admin audit log: each row's hash covers its own fields plus
-- the previous row's hash, so a row edited or deleted directly in Postgres
-- (bypassing the application) breaks the chain and is detectable by
-- recomputing hashes forward from the first row. `seq` (not `created_at`,
-- which can collide or skew under concurrency) is the authoritative chain
-- order — the hash preimage itself doesn't include `seq` since it's only
-- assigned by Postgres at insert time; verification walks rows in `seq`
-- order and checks each row's `previous_hash` against the prior row's
-- `entry_hash` (see models::audit_log).
CREATE TABLE admin_audit_log (
    seq BIGSERIAL PRIMARY KEY,
    id UUID NOT NULL DEFAULT gen_random_uuid(),
    admin_id UUID REFERENCES admins(id),
    event TEXT NOT NULL,
    detail JSONB NOT NULL DEFAULT '{}'::jsonb,
    ip_address TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    previous_hash TEXT NOT NULL,
    entry_hash TEXT NOT NULL
);
CREATE INDEX idx_admin_audit_log_created_at ON admin_audit_log (created_at);
CREATE INDEX idx_admin_audit_log_admin_id ON admin_audit_log (admin_id);

-- Canary admin account: a disabled, alluringly-named account whose password
-- hash never validates. The login handler intercepts this username before
-- ever querying this row (see controllers/admin/auth.rs::login), so any
-- login *attempt* against it — even a failed one — is treated as a near-
-- zero-false-positive signal of credential-stuffing/recon: logged and the
-- source IP added to the deny-list. This row exists mainly so the account
-- looks real to anyone inspecting the `admins` table directly.
-- `lock_until` is a concrete far-future date rather than `infinity` — sqlx
-- decodes TIMESTAMPTZ into `chrono::DateTime<Utc>`, which has no
-- representation for Postgres's infinity value.
INSERT INTO admins (id, username, email, password, role, failed_login_attempts, lock_until, created_at)
VALUES (
    gen_random_uuid(),
    'superadmin',
    'superadmin@localhost.invalid',
    -- Not a valid argon2/bcrypt hash for any password; verify_password()
    -- returns an error (detect_hash_type -> Unknown) rather than ever
    -- succeeding, and the account is additionally locked below.
    'canary-account-do-not-use',
    'admin',
    0,
    '9999-12-31 23:59:59+00',
    now()
)
ON CONFLICT (username) DO NOTHING;
