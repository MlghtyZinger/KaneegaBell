-- Wikidata (keyless, CC0; milestone s part 2): series and franchises. KB keeps
-- only the entries matching the library's games; names are looked up again
-- per platform when needed, never stored.

CREATE TABLE wd_games (
    qid        INTEGER PRIMARY KEY,            -- Wikidata item number (Q123 -> 123)
    name       TEXT NOT NULL,
    year       INTEGER,                        -- first release year
    igdb       TEXT,                           -- IGDB game ID (its URL name)
    fetched_at INTEGER NOT NULL
);

CREATE TABLE wd_series (
    qid  INTEGER PRIMARY KEY,
    name TEXT NOT NULL
);

CREATE TABLE wd_game_series (
    game_qid   INTEGER NOT NULL,
    series_qid INTEGER NOT NULL,
    kind       TEXT NOT NULL,                  -- 'series' (P179) | 'franchise' (P8345)
    PRIMARY KEY (game_qid, series_qid, kind)
);
CREATE INDEX wd_game_series_series ON wd_game_series(series_qid);

-- games.wd_id: the matched Wikidata item (NULL = not matched).
-- games.wd_status: 'none' (not tried), 'ok', 'notfound', 'ambiguous'.
ALTER TABLE games ADD COLUMN wd_id INTEGER;
ALTER TABLE games ADD COLUMN wd_status TEXT NOT NULL DEFAULT 'none';
