-- Unifies the two divergent flagged-attendance review systems (System A:
-- controllers/admin/flags.rs, keyed on the legacy `device_flag` column and
-- discarding review notes; System B: controllers/admin_security.rs, session-
-- scoped only, notes persisted into `flag_details`) into one global,
-- severity-sortable review queue. See controllers/admin_security.rs for the
-- new unified endpoint.

-- Rollup of the max severity across gps_anomalies/emulator_flags/device_flag
-- for this row, computed in Rust at write time (attendance.rs /
-- public_webauthn.rs, alongside where `flagged` itself is set) rather than a
-- DB trigger, since the anomaly-scoring logic already lives there. Nullable:
-- unflagged rows (the overwhelming majority) carry no severity at all.
ALTER TABLE attendances ADD COLUMN flag_severity TEXT
    CHECK (flag_severity IN ('high', 'medium', 'low'));

-- A real, queryable review-notes column. System A accepted `review_notes`
-- and silently discarded it; System B overloaded `flag_details` (already
-- used for the anomaly summary) to hold reviewer notes instead. This
-- column is dedicated to what an admin *writes* when reviewing a flag.
ALTER TABLE attendances ADD COLUMN review_notes TEXT;

-- Primary access pattern for the unified queue: unreviewed, most-severe
-- first, optionally cross-session. The predicate mirrors the queue's WHERE
-- clause (a flag under either the legacy `device_flag` enum or the newer
-- `flagged` boolean) so this index actually gets used.
CREATE INDEX idx_attendances_review_queue
    ON attendances (flag_reviewed, flag_severity, captured_at DESC)
    WHERE flagged = true OR device_flag IS NOT NULL;
