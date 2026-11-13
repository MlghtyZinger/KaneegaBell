-- Per-game override; the original platform, ROM and emulator choices stay intact.
CREATE TABLE direct_launch (
    game_id INTEGER PRIMARY KEY REFERENCES games(id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN ('port', 'recomp', 'decomp')),
    executable TEXT NOT NULL
);
