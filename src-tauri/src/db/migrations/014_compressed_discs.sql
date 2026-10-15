-- RetroAchievements can now hash compressed disc images (CHD, CSO/ZSO, RVZ,
-- WIA, GCZ, CISO, TGC): ones marked "no hash possible" are hashed again, as
-- are playlists, whose first disc may be one of them.
UPDATE games SET ra_hash = NULL, ra_game_id = NULL
WHERE ra_hash = ''
  AND (lower(launch_path) LIKE '%.chd' OR lower(launch_path) LIKE '%.cso' OR lower(launch_path) LIKE '%.zso'
    OR lower(launch_path) LIKE '%.rvz' OR lower(launch_path) LIKE '%.wia' OR lower(launch_path) LIKE '%.gcz'
    OR lower(launch_path) LIKE '%.ciso' OR lower(launch_path) LIKE '%.tgc' OR lower(launch_path) LIKE '%.m3u');
