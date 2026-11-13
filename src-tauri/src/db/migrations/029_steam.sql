-- Steam identities are independent of manifest/library locations.
CREATE TABLE steam_games (
    game_id INTEGER PRIMARY KEY REFERENCES games(id) ON DELETE CASCADE,
    app_id INTEGER NOT NULL UNIQUE CHECK(app_id > 0 AND app_id <= 4294967295),
    library_path TEXT NOT NULL,
    install_dir TEXT NOT NULL,
    executables TEXT NOT NULL DEFAULT '[]',
    tracking_exe TEXT
);
