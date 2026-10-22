# PSP lettering, PS1 final emblem and audio cue alignment

2026-10-22, standalone review previews. Earlier versions are preserved.

- PS1 final screen uses the established KB bull artwork with its broken horn,
  cream outline and gold nose ring. There is no bottom KB footer. The first
  screen and white Zrnic name remain.
- PSP uses the full name in thin rectangular MINIMALHARD 2 lettering, with a
  clear adapted K, and a smaller company line. The waves-only stage finishes
  before the wordmark and gold flare appear.
- PS2 retains its revised wordmark and camera movement, with the title reveal
  moved to match the actual launch-chime attack.

## Audio and visual cue fixes

Earlier file-duration and audio-offset checks did not measure whether the
animation events landed on the sounds. The previous PSP logo reveal began
roughly 0.6 s after the game-launch attack. The previous PS1 edit began the
second audio segment at source 12.7 s, cutting into the final chime attack.

The PSP wordmark now reaches its midpoint at approximately 3.932 s, with the
gold flare on the same cue. Dark waves wash to white from 3.56 to 3.80 s;
the wordmark begins at 3.812 s. The full PSP recordings remain unchanged.

PS1 now uses source ranges 0–5.2 s and 12.25–15.05 s, totaling 8.0 s, retaining
the final attack. Segment fade-outs are 120 ms and 180 ms. Its black final screen
begins at 5.2 s and the emblem reveal begins at 5.32 s.
PS2 remains 7.6 s, using 0–5.6 s and 14.4–16.4 s.
All edits affect temporary audio copies only; original MP3s are unchanged.

Measured from decoded output audio and encoded video frames:

| Event | Audio attack | Half-visible visual | Difference |
|---|---:|---:|---:|
| PS1 final emblem | 5.320 s | 5.350 s | +30.0 ms |
| PS2 launch title | 5.810 s | 5.817 s | +6.7 ms |
| PSP wordmark | 3.950 s | 3.933 s | −16.7 ms |

Audio attacks use 10 ms RMS rises near the independently measured source cues.
Visual measurements use contrast against a settled frame at 60 fps; these are
frame/energy checks, not a subjective listening test on the owner's speakers.
Every video passed full decode, playback, seeking and completion. Audio matches
the intended edited timeline with zero offset at 10 ms resolution and envelope
correlation above 0.9999. Details are in `verification.json`.

## Reproduction

Use the existing local font paths described in the previous revision (Zrnic,
MINIMALHARD 2 and the already available Rodin). No fonts are installed or bundled.

```powershell
node tools/sounds/render-intro-videos.mjs --sony-revision docs/intro-video-previews/sony-revision-4
```

Set `KB_WORDMARK_FONT`, `KB_MONOGRAM_FONT` and `KB_INTRO_FONT` first.
Use a new output directory or `--replace` for these known preview files.
