-- Roll numbers are stored trimmed and upper-cased.
--
-- Attendance rows, exports and every roll-number comparison already use the
-- upper-case form; roster rows kept whatever casing the import had. A row is
-- only rewritten when no other row of the same roster would collide with its
-- upper-cased value (rows involved in a case-insensitive collision keep their
-- current text and are reported by the application's duplicate checks).
UPDATE students s
SET roll_number = upper(btrim(s.roll_number))
WHERE s.roll_number <> upper(btrim(s.roll_number))
  AND NOT EXISTS (
        SELECT 1 FROM students o
        WHERE o.batch_id = s.batch_id
          AND o.id <> s.id
          AND upper(btrim(o.roll_number)) = upper(btrim(s.roll_number))
  );

UPDATE excel_batch_students s
SET roll_number = upper(btrim(s.roll_number))
WHERE s.roll_number <> upper(btrim(s.roll_number))
  AND NOT EXISTS (
        SELECT 1 FROM excel_batch_students o
        WHERE o.excel_batch_id = s.excel_batch_id
          AND o.id <> s.id
          AND upper(btrim(o.roll_number)) = upper(btrim(s.roll_number))
  );
