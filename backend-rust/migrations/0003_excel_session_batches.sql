-- Excel-uploaded roster batches for the bulk multi-session creation feature.
-- Deliberately separate from `batches`/`students`: these rows are always
-- generated 1:1 alongside a session created via the Excel-upload flow and
-- are never selectable from the manual "create batch"/"pick a batch" UI.

CREATE TABLE excel_batches (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    -- Display name shown on the Batches list: "{track} — {class_label} — {session_time_raw}".
    name TEXT NOT NULL,
    description TEXT,
    source_filename TEXT,
    session_date DATE NOT NULL,
    track TEXT NOT NULL,
    -- The uploaded sheet's "class" column = room/location, kept as free text
    -- rather than a link to `locations` (that table is GPS-radius/geofencing
    -- specific and semantically unrelated to a room label).
    class_label TEXT NOT NULL,
    session_time_raw TEXT NOT NULL,
    duration_minutes INT NOT NULL,
    college_name TEXT,
    -- Union of every assigned_mentor email parsed for this group, kept for
    -- audit/export fallback even after emails are resolved to admin accounts
    -- via session_admins. Not used for access control.
    raw_mentor_emails TEXT[] NOT NULL DEFAULT '{}',
    created_by UUID NOT NULL REFERENCES admins(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_excel_batches_created_by ON excel_batches (created_by, created_at DESC);

CREATE TABLE excel_batch_students (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    excel_batch_id UUID NOT NULL REFERENCES excel_batches(id) ON DELETE CASCADE,
    position INT NOT NULL DEFAULT 0,
    name TEXT NOT NULL,
    roll_number TEXT NOT NULL,
    email TEXT,
    -- Per-student elective/language subgroup (e.g. "Python"/"Java"/"C++"),
    -- export-only column; not a grouping dimension.
    round TEXT
);
CREATE INDEX idx_excel_batch_students_batch_id ON excel_batch_students (excel_batch_id, position);
CREATE INDEX idx_excel_batch_students_roll_number ON excel_batch_students (roll_number);

-- Links a session to the excel batch that generated it. Nullable — only set
-- for sessions created via the bulk-upload flow. `is_exam_session` semantics
-- are still driven purely by session_admins non-emptiness (unchanged); this
-- column only tells roster/export code which table to resolve students from.
ALTER TABLE sessions ADD COLUMN excel_batch_id UUID REFERENCES excel_batches(id);
CREATE INDEX idx_sessions_excel_batch_id ON sessions (excel_batch_id);
ALTER TABLE sessions ADD CONSTRAINT chk_sessions_single_roster_source
    CHECK (batch_id IS NULL OR excel_batch_id IS NULL);
