-- Supports the batch attendance-analytics feature (controllers/batch_analytics.rs):
-- per-student attendance percentages, per-batch and cross-batch roll-number
-- lookup, both scoped by a created_at date range.

-- "Sessions for batch X created within [from,to]" is needed both as a plain
-- COUNT (batch Overview tab) and as a GROUP BY batch_id (Student View /
-- global roll-number Lookup, which can touch many batches' sessions in one
-- query). idx_sessions_batch_id (batch_id alone) can't satisfy the range
-- predicate without a heap revisit per row; this composite lets Postgres
-- range-scan directly. idx_sessions_batch_id becomes fully redundant after
-- this (same leading column) but is left in place — not dropped here.
CREATE INDEX idx_sessions_batch_created ON sessions (batch_id, created_at);

-- The global roll-number Lookup resolves arbitrary-case roll numbers against
-- `students` across every batch in the system (roll numbers are only unique
-- within a batch — see idx_students_roll_number's original comment in
-- 0001_initial_schema.sql). The existing index is on the raw column and
-- can't serve an upper(roll_number) predicate; this functional index makes
-- that resolution an index scan instead of a full table scan of `students`.
CREATE INDEX idx_students_roll_number_upper ON students (upper(roll_number));
