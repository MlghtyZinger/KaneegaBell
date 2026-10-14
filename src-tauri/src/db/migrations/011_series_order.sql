-- A game's place in its series (Wikidata's "series ordinal", P1545: "2", "1.5", ...).
ALTER TABLE wd_game_series ADD COLUMN ord TEXT;
-- Entries stored before this have no numbers: forgetting them makes the next
-- Wikidata pass ask again (matches stay; only the small entry rows go).
DELETE FROM wd_game_series;
DELETE FROM wd_series;
DELETE FROM wd_games;
