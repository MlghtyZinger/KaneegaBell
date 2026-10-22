# KB's sounds

The menu/launch profiles and legacy intro mixes are made with ffmpeg from CC0
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
exists (see Settings → General → Start-up intro). These optional overrides are never shipped.

## Default startup recordings

The bundled `intro_*.ogg` files now use the owner-provided recordings in `audio files`.
Rebuild them with `& "C:\Emulator files\tools\sounds\import-intros.ps1"`.
The script leaves the original MP3s untouched, converts them to 48 kHz stereo Opus,
and joins PSP part 1 then part 2. It preserves the full recordings, including silence.
The intro host keeps the screen open for at least the decoded audio duration.
The old `build_sounds.py` mixes are optional alternatives; do not copy them over the defaults.

## Console menu effects

The owner requested the Wii/PS3 menu recordings on 2026-10-22. Six short effects were
downloaded as standalone WAV/MP3 files and converted with existing FFmpeg to Vorbis
quality 5, without altering the startup recordings.

- Wii: https://github.com/danintosh/Wii-Menu-HTML — hover.wav → menu_wii_move.ogg,
  hoverchannel.wav → menu_wii_select.ogg, scroll.wav → menu_wii_back.ogg.
- PS3: https://github.com/JMRDev0/XMB-PS3-Icons-Sounds-Pack- — up.ogg → menu_ps3_move.ogg,
  ok.ogg → menu_ps3_select.ogg, cancel.ogg → menu_ps3_back.ogg.

Nintendo/Sony recordings retain their respective ownership; the repositories' code
licenses do not make those recordings CC0. The Wii menu also loops bgm.wav from
the same danintosh repository, converted to 48 kHz stereo MP3 quality 4 as
menu_wii_theme.mp3 (74.208 seconds). It follows TV sound/volume and pauses for
intros, hidden windows, gameplay and other menu styles. PS3 keeps navigation sounds
without background music, as requested by the owner. No firmware, system code
or fonts were downloaded or bundled.
