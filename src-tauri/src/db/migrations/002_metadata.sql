-- Milestone d: artwork and metadata.

-- Folder name of this platform on thumbnails.libretro.com (keyless box art).
ALTER TABLE platforms ADD COLUMN libretro_system TEXT;

-- 'none' (never tried), 'ok', 'notfound' (no match; retried weekly), 'error' (retried next run)
ALTER TABLE games ADD COLUMN art_status TEXT NOT NULL DEFAULT 'none';
-- Changes whenever the chosen art changes; used to bust the image cache.
ALTER TABLE games ADD COLUMN art_version INTEGER NOT NULL DEFAULT 0;
ALTER TABLE games ADD COLUMN art_checked_at INTEGER;

CREATE TABLE artwork (
    id         INTEGER PRIMARY KEY,
    game_id    INTEGER NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    kind       TEXT NOT NULL,              -- 'boxart' | 'grid' | 'hero' | 'logo'
    source     TEXT NOT NULL,              -- 'libretro' | 'igdb' | 'sgdb'
    remote_id  TEXT,                       -- libretro name, IGDB image_id, SGDB asset id
    url        TEXT NOT NULL,
    width      INTEGER,
    height     INTEGER,
    selected   INTEGER NOT NULL DEFAULT 0,
    fetched_at INTEGER NOT NULL,
    UNIQUE (game_id, kind, source, remote_id)
);
CREATE INDEX artwork_game ON artwork(game_id, kind, selected);

CREATE TABLE metadata (
    game_id      INTEGER PRIMARY KEY REFERENCES games(id) ON DELETE CASCADE,
    title        TEXT,                     -- official title from the source
    description  TEXT,
    release_date INTEGER,                  -- unix seconds
    developer    TEXT,
    publisher    TEXT,
    source       TEXT NOT NULL,
    fetched_at   INTEGER NOT NULL
);

CREATE TABLE genres (
    id   INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE
);

CREATE TABLE game_genres (
    game_id  INTEGER NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    genre_id INTEGER NOT NULL REFERENCES genres(id) ON DELETE CASCADE,
    PRIMARY KEY (game_id, genre_id)
);

-- Raw API responses and remote listings, so refreshes work offline.
CREATE TABLE api_cache (
    key        TEXT PRIMARY KEY,
    body       BLOB NOT NULL,
    fetched_at INTEGER NOT NULL
);
