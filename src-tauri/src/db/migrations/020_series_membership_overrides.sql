-- User changes to a Wikidata series are stored separately so refreshing its
-- source data never overwrites the user's additions or removals.
CREATE TABLE series_membership_overrides (
    series_qid INTEGER NOT NULL,
    game_id    INTEGER NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    member     INTEGER NOT NULL CHECK (member IN (0, 1)),
    series_name TEXT NOT NULL,
    series_kind TEXT NOT NULL CHECK (series_kind IN ('series', 'franchise')),
    PRIMARY KEY (series_qid, game_id)
);
CREATE INDEX series_membership_overrides_game ON series_membership_overrides(game_id);
