-- Milestone a schema. Later milestones add tables via new numbered migrations.

CREATE TABLE emulators (
    id               TEXT PRIMARY KEY,
    name             TEXT NOT NULL,
    exe_path         TEXT,                       -- NULL until the user sets it
    default_template TEXT NOT NULL,
    template_override TEXT,                      -- user override of default_template
    supports_m3u     INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE platforms (
    id                TEXT PRIMARY KEY,
    name              TEXT NOT NULL,
    short_name        TEXT NOT NULL,
    sort_order        INTEGER NOT NULL,
    emulator_id       TEXT NOT NULL REFERENCES emulators(id),
    default_core      TEXT,                      -- RetroArch core basename, e.g. 'mgba'
    core_override     TEXT,
    template_override TEXT,                      -- overrides the emulator template for this platform
    extensions        TEXT NOT NULL,             -- JSON array, lowercase, no dot (built-in default)
    extensions_override TEXT,                    -- JSON array set by the user
    ra_console_id     INTEGER,
    enabled           INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE rom_sources (
    id          INTEGER PRIMARY KEY,
    path        TEXT NOT NULL UNIQUE,
    platform_id TEXT REFERENCES platforms(id),   -- NULL = auto-detect by extension/header
    recursive   INTEGER NOT NULL DEFAULT 1,
    kind        TEXT NOT NULL DEFAULT 'files'    -- 'files' | 'ps3_hdd0'
);

CREATE TABLE games (
    id            INTEGER PRIMARY KEY,
    platform_id   TEXT NOT NULL REFERENCES platforms(id),
    title         TEXT NOT NULL,
    variant       TEXT,                          -- hack/translation/edition info kept from tags
    sort_title    TEXT NOT NULL,
    search_key    TEXT NOT NULL,
    region        TEXT,
    serial        TEXT,                          -- header/SFO game ID when cheaply available
    launch_path   TEXT NOT NULL UNIQUE,
    source_id     INTEGER REFERENCES rom_sources(id) ON DELETE SET NULL,
    total_size    INTEGER NOT NULL DEFAULT 0,
    mtime         INTEGER NOT NULL DEFAULT 0,
    is_multidisc  INTEGER NOT NULL DEFAULT 0,
    favorite      INTEGER NOT NULL DEFAULT 0,
    play_seconds  INTEGER NOT NULL DEFAULT 0,
    last_played   INTEGER,
    running_since INTEGER,
    missing       INTEGER NOT NULL DEFAULT 0,
    added_at      INTEGER NOT NULL,
    igdb_id       INTEGER,
    moby_id       INTEGER,
    sgdb_id       INTEGER,
    ra_game_id    INTEGER,
    ra_hash       TEXT,
    match_status  TEXT NOT NULL DEFAULT 'none'
);

CREATE INDEX games_platform_sort ON games(platform_id, sort_title);
CREATE INDEX games_last_played ON games(last_played);
CREATE INDEX games_favorite ON games(favorite);

-- Every file belonging to a game: discs of a multi-disc set, the files an .m3u/.cue
-- references, etc. Used for change detection and to hide companion files from scans.
CREATE TABLE game_files (
    game_id INTEGER NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    path    TEXT NOT NULL UNIQUE,
    disc_no INTEGER,
    size    INTEGER NOT NULL DEFAULT 0,
    mtime   INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX game_files_game ON game_files(game_id);

CREATE TABLE settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL                          -- JSON
);
