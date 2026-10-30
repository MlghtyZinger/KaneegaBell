-- SteamGridDB (optional key): a background pass matches games by title
-- (games.sgdb_id, from 001) and fetches a hero (wide background) and a logo
-- for TV Mode, plus a cover for games no other source has.
-- 'none' = not looked up, 'ok' = matched, 'notfound', 'error'.
ALTER TABLE games ADD COLUMN sgdb_status TEXT NOT NULL DEFAULT 'none';
ALTER TABLE games ADD COLUMN sgdb_checked_at INTEGER;
-- 1 when the user picked the hero or logo themselves: never replaced automatically.
ALTER TABLE games ADD COLUMN extras_manual INTEGER NOT NULL DEFAULT 0;
