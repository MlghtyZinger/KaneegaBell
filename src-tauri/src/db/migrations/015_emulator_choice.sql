-- Several emulators per platform (milestone q, part 1).
--
-- A platform's built-in emulator (platforms.emulator_id, re-seeded on every
-- start) is always usable; platform_emulators lists the others the user added
-- for it, each with its own RetroArch core and template. platforms.emulator_choice
-- is the user's default (NULL = the built-in one); games.emulator_choice is a
-- game's own pick (NULL = the platform's default). Existing setups need no
-- change: every platform keeps its current emulator as its default.

ALTER TABLE emulators ADD COLUMN custom INTEGER NOT NULL DEFAULT 0;  -- added by the user
ALTER TABLE platforms ADD COLUMN emulator_choice TEXT;
ALTER TABLE games ADD COLUMN emulator_choice TEXT;

CREATE TABLE platform_emulators (
    platform_id       TEXT NOT NULL REFERENCES platforms(id) ON DELETE CASCADE,
    emulator_id       TEXT NOT NULL REFERENCES emulators(id) ON DELETE CASCADE,
    core              TEXT,  -- RetroArch core basename for this platform, e.g. 'swanstation'
    template_override TEXT,
    PRIMARY KEY (platform_id, emulator_id)
);
