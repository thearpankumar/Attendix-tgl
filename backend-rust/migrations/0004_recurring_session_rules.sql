-- Recurring ("cron job") attendance sessions: a rule that auto-creates a
-- fresh `sessions` row at one or more times of day, on selected days of the
-- week, for a fixed location. Each occurrence is a normal, independent
-- session row (own token/expiry/roster logic), so nothing about the
-- existing per-session machinery needs to change -- the rule is only what
-- creates rows over time.

CREATE TABLE recurring_session_rules (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    location_id UUID NOT NULL REFERENCES locations(id),
    batch_id UUID REFERENCES batches(id),
    description TEXT,
    duration_minutes INT NOT NULL CHECK (duration_minutes BETWEEN 5 AND 480),
    -- Local wall-clock times (not UTC) -- interpreted against `timezone` at
    -- occurrence-computation time so scheduling stays correct across DST.
    run_times_local TIME[] NOT NULL,
    timezone TEXT NOT NULL,
    -- 0=Sunday..6=Saturday. Admin-selected explicitly; no implicit "every day".
    days_of_week SMALLINT[] NOT NULL,
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_by UUID NOT NULL REFERENCES admins(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- The short link "locked" to this rule: re-pointed to whichever
    -- occurrence is currently active. NULL while active means the rule
    -- creates sessions with no stable link. Cleared when the rule is
    -- paused or deleted, freeing the code for ad-hoc reuse immediately.
    locked_short_code TEXT REFERENCES short_links(short_code),
    -- Precomputed next-fire instant in UTC. This is the only column the
    -- scheduler's hot-path claim query touches -- all timezone/DST math
    -- happens when this is (re)computed, not on every tick.
    next_run_at TIMESTAMPTZ NOT NULL,
    last_generated_at TIMESTAMPTZ,
    last_generated_session_id UUID REFERENCES sessions(id),
    -- Diagnostic only (which replica last claimed this rule) -- the actual
    -- concurrency guard is the `FOR UPDATE SKIP LOCKED` row lock taken by
    -- the scheduler, not this column.
    claimed_by TEXT,
    claimed_at TIMESTAMPTZ,
    consecutive_failures INT NOT NULL DEFAULT 0,
    last_error TEXT,
    CHECK (array_length(run_times_local, 1) >= 1),
    CHECK (array_length(days_of_week, 1) >= 1)
);

CREATE INDEX idx_recurring_rules_next_run ON recurring_session_rules (next_run_at) WHERE is_active = true;
CREATE INDEX idx_recurring_rules_created_by ON recurring_session_rules (created_by);
-- Only one *active* rule may hold a given code at a time; pausing/deleting
-- a rule clears locked_short_code so the code drops out of this index.
CREATE UNIQUE INDEX idx_recurring_rules_locked_code ON recurring_session_rules (locked_short_code)
    WHERE is_active = true AND locked_short_code IS NOT NULL;

ALTER TABLE sessions ADD COLUMN recurring_rule_id UUID REFERENCES recurring_session_rules(id) ON DELETE SET NULL;
CREATE INDEX idx_sessions_recurring_rule_id ON sessions (recurring_rule_id);
