-- Optional IGDB metadata (descriptions, release dates, companies, genres).

-- IGDB platform id, for searches limited to the right system.
ALTER TABLE platforms ADD COLUMN igdb_platform INTEGER;

-- games.match_status (from 001): 'none' (never tried), 'ok' (matched
-- automatically), 'manual' (picked by the user; never changed automatically),
-- 'notfound' (retried weekly), 'error' (retried next run).
ALTER TABLE games ADD COLUMN meta_checked_at INTEGER;
