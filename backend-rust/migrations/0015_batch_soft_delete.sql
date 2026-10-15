-- Deleting a batch that sessions (or recurring rules) still reference used to
-- fail with an opaque 500: sessions.batch_id and recurring_session_rules.batch_id
-- are plain FKs to batches(id), and attendance exports / absent lists / stats
-- for those sessions are computed from the batch's roster.
--
-- Instead of cascading (which would wipe the historical roster) or nulling the
-- FK (which would silently turn a roster-restricted session into an open one
-- and empty its absent list), a batch that is still referenced is *archived*:
-- `deleted_at` is set, the row and its students stay, and every admin-facing
-- listing hides it. Batches nothing references are still hard-deleted.
ALTER TABLE batches ADD COLUMN deleted_at TIMESTAMPTZ;

-- Listings only ever want live batches.
CREATE INDEX idx_batches_live_created_by ON batches (created_by) WHERE deleted_at IS NULL;
