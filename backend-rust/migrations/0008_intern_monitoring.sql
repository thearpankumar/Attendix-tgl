-- Phase 5 of session monitoring: reuses the existing Session/RecurringRule
-- machinery for interns instead of inventing a parallel "monitoring
-- assignment" concept. An intern monitoring rule IS a recurring_session_rule
-- (its "9am-6pm Mon-Fri" schedule is structurally identical to
-- run_times_local/days_of_week); its generated occurrences ARE sessions,
-- just location-less/batch-less/short-link-less with monitoring forced on.
-- Everything downstream (extension pairing, session_device_locks,
-- telemetry_events, the behavior-viewer UI) needs zero changes — roll_number
-- is already free-text throughout that pipeline (no format constraint),
-- so an employee ID works exactly like a roll number already does.

-- `sessions.location_id` is already nullable (exam sessions have none) --
-- only the recurring-rule table required one.
ALTER TABLE recurring_session_rules ALTER COLUMN location_id DROP NOT NULL;

ALTER TABLE sessions
    ADD COLUMN session_kind TEXT NOT NULL DEFAULT 'attendance'
        CHECK (session_kind IN ('attendance', 'intern_monitoring'));

ALTER TABLE recurring_session_rules
    ADD COLUMN session_kind TEXT NOT NULL DEFAULT 'attendance'
        CHECK (session_kind IN ('attendance', 'intern_monitoring'));

-- "attendance rule requires a location" stays enforced -- just moved from a
-- blanket NOT NULL to a kind-conditional one, since `sessions.location_id`
-- already allows NULL for exam sessions and can't take the equivalent
-- constraint without breaking those existing rows.
ALTER TABLE recurring_session_rules
    ADD CONSTRAINT recurring_rules_location_required_for_attendance
        CHECK (session_kind = 'intern_monitoring' OR location_id IS NOT NULL);
