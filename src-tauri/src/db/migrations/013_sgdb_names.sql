-- SteamGridDB is now also searched by LaunchBox's names and alternate names
-- (".hack//Infection" for "Dot Hack Part 1 - Infection"): look up the games it
-- didn't find again, once.
UPDATE games SET sgdb_status = 'none', sgdb_checked_at = NULL WHERE sgdb_status = 'notfound';
