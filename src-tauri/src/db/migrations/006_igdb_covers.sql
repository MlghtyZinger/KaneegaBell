-- IGDB becomes a cover source too (for games libretro and GameTDB lack, and
-- instead of GameTDB's generic "PlayStation Network" frames for PSN titles).

-- 1 when the user chose (or removed) the cover: never replaced automatically.
ALTER TABLE games ADD COLUMN art_manual INTEGER NOT NULL DEFAULT 0;
UPDATE games SET art_manual = 1 WHERE art_status = 'removed';

-- Re-fetch details of matched games (a direct lookup by IGDB id, ~100 games
-- per request) so their IGDB covers get recorded.
DELETE FROM metadata;
