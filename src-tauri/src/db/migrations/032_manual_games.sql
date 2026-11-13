-- Entries explicitly added by the player, independent of scanned folders.
CREATE TABLE manual_games (
    game_id INTEGER PRIMARY KEY REFERENCES games(id) ON DELETE CASCADE
);
