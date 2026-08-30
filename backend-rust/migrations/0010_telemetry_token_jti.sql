-- Tracks the signed telemetry token issued for a device lock, so ongoing
-- telemetry ingestion (controllers::telemetry::ingest_telemetry) can be
-- authenticated by a short-lived JWT instead of trusting the client-supplied
-- `extensionInstanceId` outright. `telemetry_token_expires_at` is stored
-- alongside the jti (rather than re-derived) so `finish_pairing` can
-- blacklist a just-superseded lock's token with its exact remaining TTL,
-- matching the admin-JWT revocation pattern (middleware/auth.rs) exactly.
ALTER TABLE session_device_locks ADD COLUMN telemetry_token_jti TEXT;
ALTER TABLE session_device_locks ADD COLUMN telemetry_token_expires_at TIMESTAMPTZ;
