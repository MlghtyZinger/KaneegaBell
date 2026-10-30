-- GameTDB joins libretro as a cover source, and a PS3 game's own icon is no
-- longer picked automatically (it's an icon, not a cover). Games that got an
-- icon automatically, or found nothing on the platforms GameTDB covers, are
-- looked up again.
UPDATE games SET art_status = 'none', art_version = art_version + 1, art_checked_at = NULL
WHERE id IN (SELECT game_id FROM artwork WHERE kind = 'boxart' AND selected = 1 AND source = 'local');
DELETE FROM artwork WHERE source = 'local';

UPDATE games SET art_status = 'none', art_checked_at = NULL
WHERE art_status = 'notfound' AND platform_id IN ('ps3', 'wii', 'gc', 'nds', '3ds');
