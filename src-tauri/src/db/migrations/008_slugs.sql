-- The IGDB match's URL name (slug), for "Open on Backloggd" (its game pages
-- use IGDB slugs). NULL = not fetched yet: matched games are looked up again
-- by id (a direct lookup) to fill it in; '' = IGDB has none.
ALTER TABLE metadata ADD COLUMN slug TEXT;
