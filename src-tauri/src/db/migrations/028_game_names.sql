-- The user's own name for a game (owner, 2026-10-27). Shown instead of the name read from the file; scans,
-- cover matching and achievements keep using the file's. user_variant: NULL = the file's subtitle,
-- '' = none. user_sort: the sort key of user_title.
ALTER TABLE games ADD COLUMN user_title TEXT;
ALTER TABLE games ADD COLUMN user_variant TEXT;
ALTER TABLE games ADD COLUMN user_sort TEXT;
