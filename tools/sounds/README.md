# KB's sounds

The menu/launch profiles and legacy intro mixes are made with ffmpeg from CC0
recordings (no synthesized "game" sounds, owner's request 2026-10-21):

| Source | License | Used for |
|---|---|---|
| [VSCO 2 Community Edition](https://github.com/sgossner/VSCO-2-CE) (Versilian Studios) | CC0 | intros: glockenspiel, marimba, piano, timpani, strings, horns, gong, cymbals |
| [The Free Firearm Sound Library](https://opengameart.org/content/the-free-firearm-sound-library) | CC0 | earlier Rockstar intro drafts |

KB's own menu/launch sound profiles were removed (owner, 2026-10-27); only the Wii/PS3 menu sounds remain.

To rebuild the old intro mixes, download the first two into `tools/sounds/snd/` (`vsco/`, `guns/`; not
committed), set `FFMPEG` to ffmpeg's path if it isn't on PATH, then run `python build_sounds.py`
(`snd/out/<intro>.ogg`). Copy results into `src/assets/sounds` as `intro_<key>.ogg`.

A user's own intro sounds don't come from here: KB plays `data\intros\<intro>.ogg|mp3|wav|m4a|flac` when it
exists (see Settings → General → Start-up intro). These optional overrides are never shipped.

## Bundled startup videos

The 0.3.16 candidate plays nine reviewed MP4s from `src/assets/intros`. Their embedded
soundtracks use the same media clock as the video; no separate default intro OGG is loaded.
Xbox was rejected and is excluded. The final selections, file hashes, and approved cue
measurements are recorded in `docs/intro-videos-0.3.16.json`.

Run `node tools/perf/intro-videos.mjs <output-folder> [mock-Vite-URL]` with the existing
FFmpeg, Playwright and Edge to check asset identity, full decode, A/V lengths, actual
player audio/playback, mute, desktop/TV volume, skipping, overrides, and replacement cleanup.
The check uses the browser's generated library, never an owner library.

## Legacy startup recordings

The legacy `intro_*.ogg` files use the owner-provided recordings in `audio files`; they are retained as source material and excluded from the app bundle.
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
intros, hidden windows, gameplay and other menu styles.

The owner changed the PS3 music preference on 2026-10-22: the same PS3 sound pack's
`oggs/bgm.ogg` is now converted to 48 kHz stereo MP3 quality 4 as
`menu_ps3_theme.mp3` (about 134.374 seconds decoded). It is the pack's background
track, not a claim that the stock PS3 XMB had this music. Switching Wii/PS3 swaps
tracks; both obey TV volume/mute, intro/visibility/gameplay pauses and TV exit cleanup.
No firmware, system code or fonts were extracted or bundled.

## Intro synchronization checks

`python tools/sounds/check_intro_audio.py <output-directory>` decodes the ten
bundled recordings, reports 10 ms energy envelopes and writes a waveform PNG.
It needs the existing FFmpeg plus numpy/Pillow, and leaves originals untouched.
The full recordings include leading/trailing silence and Konami's repeated phrase.
`intro.css` follows those recordings; arbitrary user overrides retain their full
duration but cannot automatically have their musical cues inferred.

`node tools/perf/intro-audio.mjs <output-directory> [Vite URL]` checks decoded
durations, audio/animation starts, cancellation, controller folding and menu music
using a browser-only generated library. Set `KB_PLAYWRIGHT` and `KB_BROWSER` if
the existing local package/browser paths differ. No real library is opened.
