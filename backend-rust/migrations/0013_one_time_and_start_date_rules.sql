-- One-time scheduled sessions and cron start dates.
--
-- A one-time schedule is a recurring rule that fires exactly once, on
-- `run_once_date` at its single `run_times_local` entry (interpreted in
-- `timezone`), then deactivates itself. Reusing the rule machinery means the
-- scheduler's claim/generate path, location/batch/monitoring/short-link
-- handling all work unchanged. `days_of_week` still holds the weekday of
-- that date so the existing NOT NULL / array_length CHECKs stay satisfied.
--
-- `start_date` is the first calendar day (in `timezone`) a recurring rule may
-- fire on; NULL means "from today". It never makes a rule fire earlier than
-- its next matching slot -- it only defers the start.
ALTER TABLE recurring_session_rules
    ADD COLUMN run_once_date DATE,
    ADD COLUMN start_date DATE;
