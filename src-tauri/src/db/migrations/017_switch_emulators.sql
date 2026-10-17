-- 0.3.3: Switch emulators whose exe was set up before they joined the Switch
-- platform automatically (later ones join when their exe is set).

INSERT OR IGNORE INTO platform_emulators (platform_id, emulator_id)
SELECT 'switch', id FROM emulators
WHERE id IN ('ryujinx', 'citron', 'sudachi', 'yuzu') AND exe_path IS NOT NULL AND exe_path != ''
  AND EXISTS (SELECT 1 FROM platforms WHERE id = 'switch');
