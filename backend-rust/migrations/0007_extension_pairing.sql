-- Phase 3: short-lived pairing requests for the browser extension. Small,
-- relational, low-write-volume state — stays on the main database, same
-- rationale as migration 0006 (extension_installations/session_device_locks).
--
-- Flow: extension calls .../extension/pair/start (creates a 'pending' row) ->
-- renders the returned pairing code as a QR the student scans on their phone
-- -> phone completes the EXISTING WebAuthn short-link authentication ceremony
-- with this pairing_code attached, which flips this row to 'completed' and
-- records who authenticated -> extension polls .../pair/status, sees
-- 'completed', then calls .../pair/finish to actually write the
-- session_device_locks row.
CREATE TABLE extension_pairing_requests (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id UUID NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    extension_instance_id UUID NOT NULL,
    pairing_code TEXT NOT NULL UNIQUE,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'completed', 'expired')),
    -- Populated once the phone-side passkey ceremony completes.
    roll_number TEXT,
    webauthn_credential_id UUID REFERENCES webauthn_credentials(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ NOT NULL,
    completed_at TIMESTAMPTZ
);
CREATE INDEX idx_extension_pairing_requests_code ON extension_pairing_requests (pairing_code);
CREATE INDEX idx_extension_pairing_requests_session ON extension_pairing_requests (session_id);
