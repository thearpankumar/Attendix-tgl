-- Manual (mentor) marks made before the re-mark path kept `verified` in sync
-- with `status`: an absent -> present flip left verified = false, so the admin
-- session table showed the student as Unverified.
UPDATE attendances
SET verified = true
WHERE source = 'manual' AND status = 'present' AND verified = false;

UPDATE attendances
SET verified = false
WHERE source = 'manual' AND status = 'absent' AND verified = true;
