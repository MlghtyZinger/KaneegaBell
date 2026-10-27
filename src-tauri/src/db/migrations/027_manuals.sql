-- Game manuals (0.5.0). One row per game that has a manual, or was looked up and has none.
--   source:  'file' (the user's own PDF/CBZ, `url` is its path), 'retrokit', 'ia' (an Internet Archive
--            item, `url` is "ia:<identifier>"), or 'none' (looked up, nothing found; retried later).
--   manual:  1 when the user picked it (automatic lookups never replace it).
--   pages:   set once a download has been turned into page images in data\manuals\<game id>\.
--   bytes:   what that folder holds (the size cap and Settings → Storage add these up).
CREATE TABLE game_manuals (
    game_id    INTEGER PRIMARY KEY REFERENCES games(id) ON DELETE CASCADE,
    source     TEXT    NOT NULL,
    label      TEXT    NOT NULL DEFAULT '',
    url        TEXT,
    manual     INTEGER NOT NULL DEFAULT 0,
    pages      INTEGER,
    outline    TEXT,
    bytes      INTEGER NOT NULL DEFAULT 0,
    checked_at INTEGER NOT NULL DEFAULT 0,
    opened_at  INTEGER NOT NULL DEFAULT 0
);
