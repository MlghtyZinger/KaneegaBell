-- A small square picture per game for the sidebar, Recent and collections
-- (0.3.7). icon_version is a cache-buster for /i/{id}?v=…: 0 means none yet,
-- and every new icon (a fresh crop of a new cover, or the user's own pick)
-- bumps it. icon_manual = 1 is a picture the user chose: a new cover never
-- replaces it.

ALTER TABLE games ADD COLUMN icon_version INTEGER NOT NULL DEFAULT 0;
ALTER TABLE games ADD COLUMN icon_manual INTEGER NOT NULL DEFAULT 0;
