-- 0.4.0: Wikidata's series come first again, IGDB's fill in (owner, 2026-10-25,
-- after IGDB-first showed duplicate collections). IGDB franchises are dropped: they
-- are too broad ("Mario Bros." holding Luigi's Mansion, several D&D umbrellas
-- holding the same two games); only IGDB's series ("collections") are kept.

DELETE FROM igdb_game_series WHERE series_id IN (SELECT id FROM igdb_series WHERE kind = 'franchise');
DELETE FROM igdb_series WHERE kind = 'franchise';

DROP VIEW series_links;

-- Every game's series from both sources. A game Wikidata has series or
-- franchises for uses only those; IGDB's series count for the others. IGDB's
-- series nest ("Super Mario" holds "Paper Mario" and "Super Mario Galaxy"), so a
-- game counts only in its most specific one that has two or more of the
-- library's games. An IGDB series named like a Wikidata one takes the Wikidata
-- item's number, so the user's changes, hiding and logo for that series stay with it.
CREATE VIEW series_links AS
WITH sized AS (
    SELECT i.game_id, i.series_id,
           (SELECT COUNT(*) FROM igdb_game_series j JOIN games y ON y.id = j.game_id AND y.missing = 0
            WHERE j.series_id = i.series_id) AS n
    FROM igdb_game_series i
),
best AS (
    SELECT game_id, series_id FROM (
        SELECT game_id, series_id, ROW_NUMBER() OVER (PARTITION BY game_id ORDER BY n, series_id DESC) AS r
        FROM sized WHERE n >= 2)
    WHERE r = 1
)
SELECT gs.series_qid AS series_id, s.name AS name, gs.kind AS kind, g.id AS game_id,
       g.wd_id AS entry, w.year AS year, gs.ord AS ord, g.missing AS missing, g.sort_title AS sort_title
FROM wd_game_series gs
JOIN wd_series s ON s.qid = gs.series_qid
JOIN games g ON g.wd_id = gs.game_qid AND g.wd_status = 'ok'
LEFT JOIN wd_games w ON w.qid = gs.game_qid
UNION ALL
SELECT COALESCE((SELECT MIN(w.qid) FROM wd_series w WHERE w.name = s.name COLLATE NOCASE), s.id),
       s.name, s.kind, g.id,
       -g.igdb_id,
       COALESCE(CAST(strftime('%Y', COALESCE(l.release_date, m.release_date), 'unixepoch') AS INTEGER), wg.year),
       NULL, g.missing, g.sort_title
FROM best i
JOIN igdb_series s ON s.id = i.series_id AND s.kind = 'series'
JOIN games g ON g.id = i.game_id AND g.match_status IN ('ok', 'manual') AND g.igdb_id IS NOT NULL
LEFT JOIN metadata m ON m.game_id = g.id
LEFT JOIN lb_games l ON l.db_id = g.lb_id AND g.lb_status IN ('ok', 'manual')
LEFT JOIN wd_games wg ON wg.qid = g.wd_id AND g.wd_status = 'ok'
WHERE NOT EXISTS (
    SELECT 1 FROM wd_game_series gs JOIN games x ON x.wd_id = gs.game_qid AND x.wd_status = 'ok'
    WHERE x.id = g.id);
