-- Admin usernames and emails are unique case-insensitively.
--
-- The old exact-match unique indexes let `ZZ_M1` and `zz_m1` (or
-- `A@x.test` and `a@x.test`) coexist. They are replaced by `lower(...)`
-- unique indexes. If a database already holds a case-insensitive duplicate
-- the unique index cannot be built; in that case a plain (non-unique)
-- `lower()` index is created instead and a NOTICE names the situation, so the
-- migration never fails or deletes anyone. The application-level pre-checks
-- (controllers/admin/users.rs) enforce uniqueness for new rows either way.
DO $$
BEGIN
    DROP INDEX IF EXISTS idx_admins_username;
    DROP INDEX IF EXISTS idx_admins_email;

    IF EXISTS (SELECT 1 FROM admins GROUP BY lower(username) HAVING COUNT(*) > 1) THEN
        RAISE NOTICE 'admins has case-insensitive duplicate usernames; created a non-unique lower(username) index';
        CREATE INDEX idx_admins_username_lower ON admins (lower(username));
    ELSE
        CREATE UNIQUE INDEX idx_admins_username_lower ON admins (lower(username));
    END IF;

    IF EXISTS (SELECT 1 FROM admins GROUP BY lower(email) HAVING COUNT(*) > 1) THEN
        RAISE NOTICE 'admins has case-insensitive duplicate emails; created a non-unique lower(email) index';
        CREATE INDEX idx_admins_email_lower ON admins (lower(email));
    ELSE
        CREATE UNIQUE INDEX idx_admins_email_lower ON admins (lower(email));
    END IF;
END $$;
