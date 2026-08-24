-- Adds an in-session "monitoring" concept, distinct from the existing
-- attendance window (`expires_at` / duration_minutes on sessions,
-- duration_minutes on recurring_session_rules, both unchanged): the
-- attendance window is how long the check-in link stays live, while
-- `class_duration_minutes` / `monitoring_ends_at` describe the full class
-- time monitoring should cover -- this is the schema-level groundwork the
-- (future) browser-extension telemetry pipeline and cron scheduler will
-- both read.

ALTER TABLE sessions
    ADD COLUMN monitoring_enabled BOOLEAN NOT NULL DEFAULT false,
    ADD COLUMN class_duration_minutes INT
        CHECK (class_duration_minutes IS NULL OR class_duration_minutes BETWEEN 5 AND 480),
    ADD COLUMN monitoring_ends_at TIMESTAMPTZ;

ALTER TABLE recurring_session_rules
    ADD COLUMN monitoring_enabled BOOLEAN NOT NULL DEFAULT false,
    ADD COLUMN class_duration_minutes INT
        CHECK (class_duration_minutes IS NULL OR class_duration_minutes BETWEEN 5 AND 480);
