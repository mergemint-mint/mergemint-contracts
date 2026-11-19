-- Keep a weighted search vector for bounty title and description.
ALTER TABLE bounties
    ADD COLUMN IF NOT EXISTS search_vector tsvector;

UPDATE bounties
SET search_vector =
    setweight(to_tsvector('english', coalesce(title, '')), 'A') ||
    setweight(to_tsvector('english', coalesce(description, '')), 'B')
WHERE search_vector IS NULL;

CREATE OR REPLACE FUNCTION update_bounty_search_vector()
RETURNS trigger AS $$
BEGIN
    NEW.search_vector :=
        setweight(to_tsvector('english', coalesce(NEW.title, '')), 'A') ||
        setweight(to_tsvector('english', coalesce(NEW.description, '')), 'B');
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS bounties_search_vector_update ON bounties;
CREATE TRIGGER bounties_search_vector_update
    BEFORE INSERT OR UPDATE OF title, description ON bounties
    FOR EACH ROW EXECUTE FUNCTION update_bounty_search_vector();

CREATE INDEX IF NOT EXISTS idx_bounties_search_vector
    ON bounties USING GIN (search_vector);