-- Demo-data seed for the batch attendance analytics feature (Batch Details
-- page / Student View tab / Student Lookup). Generates ONE batch with 5000
-- students, 40 daily sessions spanning the last 40 days, and randomized
-- attendance (mix of present / explicit absent / unmarked-no-row) so the
-- color-coded percentage feature and per-student session drill-down have
-- realistic, non-uniform data to show.
--
-- Idempotent / safe to re-run: deletes any prior run's demo batch first
-- (FK order: attendances -> sessions -> batches; students cascade via
-- students_batch_id_fkey ON DELETE CASCADE, sessions does NOT cascade from
-- batches so it must be deleted explicitly before the batch row).
--
-- Run against the project's own dev stack (docker-compose.yml +
-- docker-compose.override.yml) — this does NOT run inside a throwaway
-- container, it targets whatever `attendance-postgres` container is already
-- serving the admin app's DATABASE_URL:
--
--   docker exec -i attendance-postgres psql -U attendix -d attendance_geotag \
--     -v ON_ERROR_STOP=1 -f - < backend-rust/scripts/seed_demo_data.sql
--
-- (substitute the actual POSTGRES_USER/POSTGRES_DB from .env if this repo's
-- values ever change from the current attendix/attendance_geotag defaults)

BEGIN;

-- 1. Remove any prior run's demo batch.
DO $$
DECLARE
  v_old_batch_id uuid;
BEGIN
  SELECT id INTO v_old_batch_id FROM batches WHERE name = '[DEMO] 5000-Student Load Test';
  IF v_old_batch_id IS NOT NULL THEN
    DELETE FROM attendances WHERE session_id IN (SELECT id FROM sessions WHERE batch_id = v_old_batch_id);
    DELETE FROM sessions WHERE batch_id = v_old_batch_id;
    DELETE FROM batches WHERE id = v_old_batch_id; -- students cascade automatically
  END IF;
END $$;

-- 2. Create the batch, owned by an existing super_admin (so it's visible no
-- matter which admin account is used to log in).
DO $$
DECLARE
  v_admin_id  uuid;
  v_batch_id  uuid := gen_random_uuid();
BEGIN
  SELECT id INTO v_admin_id FROM admins WHERE role = 'super_admin' ORDER BY created_at LIMIT 1;
  IF v_admin_id IS NULL THEN
    RAISE EXCEPTION 'No super_admin account exists in admins — cannot seed a demo batch';
  END IF;

  INSERT INTO batches (id, name, description, created_by, created_at)
  VALUES (
    v_batch_id,
    '[DEMO] 5000-Student Load Test',
    'Synthetic demo data for the batch attendance analytics feature: 5000 students, 40 days of daily sessions, randomized present/absent/unmarked attendance. Safe to delete — see backend-rust/scripts/seed_demo_data.sql for the cleanup SQL.',
    v_admin_id,
    now()
  );

  -- 3. 5000 students. Deterministic but varied names from small first/last
  -- name pools; ~1/6 have no college_name and ~1/5 have no email, to mirror
  -- real messy roster data and exercise the nullable columns.
  INSERT INTO students (id, batch_id, position, name, roll_number, college_name, email)
  SELECT
    gen_random_uuid(),
    v_batch_id,
    n,
    (ARRAY['Aarav','Vivaan','Aditya','Vihaan','Arjun','Sai','Reyansh','Krishna','Ishaan','Rohan',
           'Ananya','Diya','Priya','Isha','Aadhya','Myra','Sara','Anika','Kiara','Meera'])[1 + (n % 20)]
      || ' ' ||
      (ARRAY['Sharma','Verma','Gupta','Reddy','Iyer','Nair','Patel','Singh','Rao','Kumar',
             'Das','Menon','Joshi','Pillai','Chatterjee','Mehta','Shah','Bose','Kapoor','Agarwal'])[1 + ((n * 7) % 20)],
    '21B91A' || LPAD(n::text, 4, '0'),
    CASE WHEN n % 6 = 0 THEN NULL
         ELSE (ARRAY['ABC Institute of Technology','XYZ College of Engineering','National Polytechnic'])[1 + (n % 3)]
    END,
    CASE WHEN n % 5 = 0 THEN NULL ELSE 'student' || n || '@example.edu' END
  FROM generate_series(0, 4999) AS n;

  -- 4. 40 sessions, one per day for the last 40 days (created_at drives the
  -- new date-range filtering — see idx_sessions_batch_created).
  CREATE TEMP TABLE demo_sessions (id uuid, created_at timestamptz) ON COMMIT DROP;
  INSERT INTO demo_sessions
  SELECT gen_random_uuid(), now() - (d || ' days')::interval
  FROM generate_series(0, 39) AS d;

  INSERT INTO sessions (id, batch_id, token_hash, token_prefix, description, created_by, expires_at, created_at)
  SELECT
    ds.id,
    v_batch_id,
    encode(sha256(ds.id::text::bytea), 'hex'),
    substr(encode(sha256(ds.id::text::bytea), 'hex'), 1, 8),
    '[DEMO] Session on ' || to_char(ds.created_at, 'YYYY-MM-DD'),
    v_admin_id,
    ds.created_at + interval '2 hours',
    ds.created_at
  FROM demo_sessions ds;

  -- 5. Attendance: each student gets a personal "attendance propensity"
  -- (0.35-0.97, uniform). Per student x session: present if a fresh
  -- random() draw is below their propensity; of the remainder, 60% become
  -- an explicit absent row and 40% are left with no row at all (unmarked) —
  -- a single set-based INSERT...SELECT over the 5000x40=200,000-row cross
  -- join (one server-side statement, not 200,000 round trips).
  CREATE TEMP TABLE demo_students (id uuid, name text, roll_number text, propensity double precision) ON COMMIT DROP;
  INSERT INTO demo_students
  SELECT id, name, roll_number, 0.35 + random() * 0.62
  FROM students WHERE batch_id = v_batch_id;

  INSERT INTO attendances (
    id, session_id, student_name, roll_number, photo_url, photo_public_id,
    student_latitude, student_longitude, distance_from_location,
    verified, source, status, marked_by_admin_id, face_detected, captured_at
  )
  SELECT
    gen_random_uuid(),
    x.session_id,
    x.name,
    upper(x.roll_number),
    '', '',
    0.0, 0.0, 0.0,
    (x.roll_result < x.propensity),
    'manual',
    CASE WHEN x.roll_result < x.propensity THEN 'present' ELSE 'absent' END,
    v_admin_id,
    true,
    x.session_created_at + interval '1 hour'
  FROM (
    SELECT
      st.name, st.roll_number, st.propensity,
      ds.id AS session_id, ds.created_at AS session_created_at,
      random() AS roll_result,
      random() AS miss_bucket_result
    FROM demo_students st
    CROSS JOIN demo_sessions ds
  ) x
  WHERE x.roll_result < x.propensity          -- present
     OR x.miss_bucket_result < 0.6;           -- else: explicit absent (60% of misses); remaining 40% stay unmarked (no row)

  RAISE NOTICE 'Seeded demo batch % (admin %)', v_batch_id, v_admin_id;
END $$;

COMMIT;

-- Verification: row counts + a spread of percentages for the first few
-- students, mirroring what GET /api/admin/batches/{id}/students would show.
SELECT 'students' AS what, COUNT(*) FROM students WHERE batch_id = (SELECT id FROM batches WHERE name = '[DEMO] 5000-Student Load Test')
UNION ALL
SELECT 'sessions', COUNT(*) FROM sessions WHERE batch_id = (SELECT id FROM batches WHERE name = '[DEMO] 5000-Student Load Test')
UNION ALL
SELECT 'attendances', COUNT(*) FROM attendances WHERE session_id IN (
  SELECT id FROM sessions WHERE batch_id = (SELECT id FROM batches WHERE name = '[DEMO] 5000-Student Load Test')
);
