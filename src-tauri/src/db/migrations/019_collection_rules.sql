-- Saved filters (milestone n, part 2): a collection with `rules` is a smart
-- collection whose games follow those rules; NULL means a manual collection.
-- The rules are JSON matching `collections::Rules` (platforms, genres,
-- developers, publishers, year range, played/unplayed, favorite, play time).
ALTER TABLE collections ADD COLUMN rules TEXT;
