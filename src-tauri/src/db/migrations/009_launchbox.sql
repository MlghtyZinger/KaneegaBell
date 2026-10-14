-- LaunchBox Games Database (keyless, milestone s). KB keeps only:
-- - a name list for the enabled platforms (main and alternate titles), for
--   matching new games and Fix match without downloading again;
-- - details and image lists for the games in the library (matched entries).

ALTER TABLE platforms ADD COLUMN launchbox_platform TEXT;

CREATE TABLE lb_names (
    db_id       INTEGER NOT NULL,              -- LaunchBox DatabaseID
    platform_id TEXT NOT NULL,                 -- KB platform
    name        TEXT NOT NULL,
    region      TEXT,                          -- alternate titles: the region they're used in
    is_alt      INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX lb_names_platform ON lb_names(platform_id);
CREATE INDEX lb_names_db ON lb_names(db_id);

CREATE TABLE lb_games (
    db_id        INTEGER PRIMARY KEY,
    name         TEXT NOT NULL,
    release_date INTEGER,                      -- unix seconds
    overview     TEXT,
    developer    TEXT,
    publisher    TEXT,
    genres       TEXT,                         -- "Action; Adventure"
    max_players  INTEGER,
    rating       REAL,                         -- community rating, 0-5
    fetched_at   INTEGER NOT NULL
);

CREATE TABLE lb_images (
    db_id  INTEGER NOT NULL,
    kind   TEXT NOT NULL,                      -- 'Box - Front', 'Clear Logo', 'Fanart - Background', ...
    region TEXT,
    file   TEXT NOT NULL,                      -- under https://images.launchbox-app.com/
    PRIMARY KEY (db_id, file)
);

-- games.lb_id: the matched LaunchBox game (NULL = not matched).
-- games.lb_status: 'none' (not tried), 'ok' (matched automatically), 'manual'
-- (picked by the user; never changed automatically), 'notfound', 'ambiguous'
-- (several equally good entries: left for Fix match).
ALTER TABLE games ADD COLUMN lb_id INTEGER;
ALTER TABLE games ADD COLUMN lb_status TEXT NOT NULL DEFAULT 'none';
