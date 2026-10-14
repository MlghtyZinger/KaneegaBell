-- Collections (milestone n, part 1): the user's own, plus the series KB knows
-- (Wikidata) shown as ready-made ones.

CREATE TABLE collections (
    id         INTEGER PRIMARY KEY,
    name       TEXT NOT NULL,
    sort_order INTEGER NOT NULL DEFAULT 0,
    game_order TEXT NOT NULL DEFAULT 'added',  -- 'added' | 'title' | 'release'
    created_at INTEGER NOT NULL
);

CREATE TABLE collection_games (
    collection_id INTEGER NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
    game_id       INTEGER NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    added_at      INTEGER NOT NULL,
    PRIMARY KEY (collection_id, game_id)
);
CREATE INDEX collection_games_game ON collection_games(game_id);

-- How a collection or series looks, by key: 'c:<collection id>' or 's:<series item>'.
-- logo: NULL = automatic (cover mosaic / the series' first logo), 'game:<id>' (that
-- game's clear logo), 'cover:<id>' (that game's cover), 'file' (the user's image,
-- stored in data\art\c\). Series can be hidden from the Collections tab.
CREATE TABLE collection_looks (
    key          TEXT PRIMARY KEY,
    logo         TEXT,
    logo_version INTEGER NOT NULL DEFAULT 0,
    hidden       INTEGER NOT NULL DEFAULT 0
);
