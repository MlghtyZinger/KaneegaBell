-- A collection's own TV Mode background (0 = none, so TV Mode uses the first game's).
ALTER TABLE collection_looks ADD COLUMN hero_version INTEGER NOT NULL DEFAULT 0;
