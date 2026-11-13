CREATE TABLE launcher_games (
    game_id INTEGER PRIMARY KEY REFERENCES games(id) ON DELETE CASCADE,
    provider TEXT NOT NULL,
    product_id TEXT NOT NULL,
    install_dir TEXT NOT NULL,
    executables TEXT NOT NULL DEFAULT '[]',
    tracking_exe TEXT,
    action TEXT NOT NULL,
    UNIQUE(provider, product_id)
);
