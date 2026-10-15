-- Mentor-assigned (exam / normal-with-mentors) sessions can now be created
-- on a schedule or cron, so a recurring rule has to carry what those
-- sessions need: the mentors to assign, the exam shape and its college name.
--
-- `session_type` is the same "normal" | "exam" split `CreateSessionRequest`
-- uses. An exam rule's generated occurrence has `starts_at` = the moment the
-- scheduler fires, the rule's mentors in `session_admins`, and an OPTIONAL
-- location (geofence) -- exam sessions are manually marked by default, but
-- may now carry a location so a short link can geofence student check-in.
ALTER TABLE recurring_session_rules
    ADD COLUMN session_type TEXT NOT NULL DEFAULT 'normal'
        CHECK (session_type IN ('normal', 'exam')),
    ADD COLUMN assigned_admin_ids UUID[] NOT NULL DEFAULT '{}',
    ADD COLUMN college_name TEXT;

-- An exam rule may omit the location (as exam sessions always could).
ALTER TABLE recurring_session_rules
    DROP CONSTRAINT recurring_rules_location_required_for_attendance;
ALTER TABLE recurring_session_rules
    ADD CONSTRAINT recurring_rules_location_required_for_attendance
        CHECK (session_kind = 'intern_monitoring' OR session_type = 'exam' OR location_id IS NOT NULL);
