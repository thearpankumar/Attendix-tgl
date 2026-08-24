-- The admin behavior view (student_behavior.rs::get_student_behavior) filters
-- on session_id AND roll_number together. The existing
-- idx_telemetry_events_session index only covers (session_id, recorded_at),
-- so roll_number is a sequential filter within a session's chunk rather than
-- an index seek — fine at today's scale, but worth closing before a
-- 500-student session's chunk gets large.
CREATE INDEX idx_telemetry_events_session_roll ON telemetry_events (session_id, roll_number, recorded_at DESC);
