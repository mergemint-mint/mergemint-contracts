-- Add indexes on the columns list_bounties / list_bounties_by_assignee
-- filter on.
--
-- The indexer (src/indexer.rs) writes bounty rows as it processes contract
-- events. list_bounties_by_assignee filters on `assignee`, and status-based
-- listing/filtering queries filter on `status`. Neither column is covered
-- by the primary key, so without these indexes both queries degrade to a
-- sequential scan as the bounties table grows.
--
-- IF NOT EXISTS makes this safe to run against a database that already has
-- either index from a prior ad-hoc migration.

CREATE INDEX IF NOT EXISTS idx_bounties_assignee ON bounties (assignee);
CREATE INDEX IF NOT EXISTS idx_bounties_status ON bounties (status);
