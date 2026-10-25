-- 0.4.0: an IGDB sub-series named after another series of the same game ("The
-- Legend of Zelda: Ocarina of Time", "Metroid Prime", "Super Mario Galaxy")
-- counts as that parent, so Zelda shows as one collection (owner, 2026-10-25).
-- The whole sub-series moves, so games listed only under it come along.
-- Otherwise as migration 025: Wikidata first, a game counts only in its most
-- specific IGDB series with two or more of the library's games.

DROP VIEW series_links;

CREATE VIEW series_links AS
WITH parents AS (
    -- A series' parent: the shortest other series whose name starts its name and
    -- that shares a game with it ("The Legend of Zelda" for "...: Ocarina of Time").
    SELECT s.id AS child,
           (SELECT t.id FROM igdb_series t
            WHERE t.kind = 'series' AND t.id <> s.id
              AND (s.name LIKE t.name || ':%' OR s.name LIKE t.name || ' %')
              AND EXISTS (SELECT 1 FROM igdb_game_series x JOIN igdb_game_series y ON y.game_id = x.game_id
                          WHERE x.series_id = s.id AND y.series_id = t.id)
            ORDER BY length(t.name) LIMIT 1) AS parent
    FROM igdb_series s WHERE s.kind = 'series'
),
lifted AS (
    SELECT DISTINCT i.game_id, COALESCE(p.parent, i.series_id) AS series_id
    FROM igdb_game_series i JOIN parents p ON p.child = i.series_id
),
sized AS (
    SELECT l.game_id, l.series_id,
           (SELECT COUNT(*) FROM lifted j JOIN games y ON y.id = j.game_id AND y.missing = 0
            WHERE j.series_id = l.series_id) AS n
    FROM lifted l
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
