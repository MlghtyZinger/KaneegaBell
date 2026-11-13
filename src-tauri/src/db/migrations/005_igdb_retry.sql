-- The first IGDB build batched searches through the multiquery endpoint,
-- which silently returns nothing for searches, so every game was marked
-- 'notfound'. Look them up again.
UPDATE games SET match_status = 'none', meta_checked_at = NULL WHERE match_status = 'notfound';
