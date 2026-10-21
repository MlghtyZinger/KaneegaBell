# KB's sounds

The sounds in `src/assets/sounds` are mixed by these scripts with ffmpeg from CC0
recordings (no synthesized "game" sounds, owner's request 2026-10-21):

| Source | License | Used for |
|---|---|---|
| [VSCO 2 Community Edition](https://github.com/sgossner/VSCO-2-CE) (Versilian Studios) | CC0 | intros: glockenspiel, marimba, piano, timpani, strings, horns, gong, cymbals |
| [The Free Firearm Sound Library](https://opengameart.org/content/the-free-firearm-sound-library) | CC0 | the Rockstar intro's shots |
| [Kenney Interface Sounds and UI Audio](https://kenney.nl/assets) | CC0 | menu clicks and switches (sound profiles) |

To rebuild, download the three into `tools/sounds/snd/` (`vsco/`, `guns/`, `kenney/interface/`, `kenney/ui/`;
not committed), set `FFMPEG` to ffmpeg's path if it isn't on PATH, then:

- `python build_sounds.py`: the intros (`snd/out/<intro>.ogg`; also older drafts the app doesn't use);
- `python make_profiles.py`: the three sound profiles (`snd/out/prof_<profile>_<move|select|back|disc|cart>.ogg`).

Copy the results into `src/assets/sounds`: intros as `intro_<key>.ogg` (`xbox` becomes `intro_xbox360.ogg`),
profiles under their own names. `make_system.py` holds the tone helpers `make_profiles.py` uses.

A user's own intro sounds don't come from here: KB plays `data\intros\<intro>.ogg|mp3|wav|m4a|flac` when it
exists (see Settings → General → Start-up intro). They're never shipped.
