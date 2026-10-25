-- 0.4.0: series and franchises from IGDB (with the user's own key) come first;
-- Wikidata's fill in for games IGDB doesn't group (owner's choice).
-- IGDB ids are stored negative (collections -id, franchises -(1000000000 + id))
-- so they never meet a Wikidata item number in collection keys and overrides.

CREATE TABLE igdb_series (
    id   INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('series', 'franchise'))
);

CREATE TABLE igdb_game_series (
    game_id   INTEGER NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    series_id INTEGER NOT NULL,
    PRIMARY KEY (game_id, series_id)
);
CREATE INDEX igdb_game_series_series ON igdb_game_series(series_id);

-- Matched games are looked up once more (by id, 100 per request) for their series.
ALTER TABLE metadata ADD COLUMN series_checked INTEGER NOT NULL DEFAULT 0;

-- Every game's series from both sources. A game with IGDB series uses only
-- those; Wikidata's count for the others. An IGDB series named like a Wikidata
-- one takes the Wikidata item's number, so the user's changes, hiding and logo
-- for that series stay with it.
CREATE VIEW series_links AS
SELECT COALESCE((SELECT MIN(w.qid) FROM wd_series w WHERE w.name = s.name COLLATE NOCASE), s.id) AS series_id,
       s.name AS name, s.kind AS kind, g.id AS game_id,
       -g.igdb_id AS entry,
       COALESCE(CAST(strftime('%Y', COALESCE(l.release_date, m.release_date), 'unixepoch') AS INTEGER), wg.year) AS year,
       NULL AS ord, g.missing AS missing, g.sort_title AS sort_title
FROM igdb_game_series i
JOIN igdb_series s ON s.id = i.series_id
JOIN games g ON g.id = i.game_id AND g.match_status IN ('ok', 'manual') AND g.igdb_id IS NOT NULL
LEFT JOIN metadata m ON m.game_id = g.id
LEFT JOIN lb_games l ON l.db_id = g.lb_id AND g.lb_status IN ('ok', 'manual')
LEFT JOIN wd_games wg ON wg.qid = g.wd_id AND g.wd_status = 'ok'
UNION ALL
SELECT gs.series_qid, s.name, gs.kind, g.id, g.wd_id, w.year, gs.ord, g.missing, g.sort_title
FROM wd_game_series gs
JOIN wd_series s ON s.qid = gs.series_qid
JOIN games g ON g.wd_id = gs.game_qid AND g.wd_status = 'ok'
LEFT JOIN wd_games w ON w.qid = gs.game_qid
WHERE NOT EXISTS (
    SELECT 1 FROM igdb_game_series i JOIN games x ON x.id = i.game_id
    WHERE i.game_id = g.id AND x.match_status IN ('ok', 'manual') AND x.igdb_id IS NOT NULL);
