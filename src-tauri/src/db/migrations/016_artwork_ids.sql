-- A hero's or logo's row id is its picture's version in image URLs
-- (`/h/{game}?v={id}`). Without AUTOINCREMENT, SQLite hands a deleted
-- newest row's id to the next insert, so replacing a background could give
-- the new one the old URL and the webview kept showing the old picture.
-- AUTOINCREMENT never reuses an id.

CREATE TABLE artwork_new (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    game_id    INTEGER NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    kind       TEXT NOT NULL,              -- 'boxart' | 'grid' | 'hero' | 'logo'
    source     TEXT NOT NULL,              -- 'libretro' | 'igdb' | 'sgdb' | 'launchbox' | 'file' ...
    remote_id  TEXT,                       -- libretro name, IGDB image_id, SGDB asset id
    url        TEXT NOT NULL,
    width      INTEGER,
    height     INTEGER,
    selected   INTEGER NOT NULL DEFAULT 0,
    fetched_at INTEGER NOT NULL,
    UNIQUE (game_id, kind, source, remote_id)
);
INSERT INTO artwork_new SELECT id, game_id, kind, source, remote_id, url, width, height, selected, fetched_at FROM artwork;
DROP TABLE artwork;
ALTER TABLE artwork_new RENAME TO artwork;
CREATE INDEX artwork_game ON artwork(game_id, kind, selected);
