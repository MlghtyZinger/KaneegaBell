# Longer PS1/PS2 timing and two-line PSP lettering

2026-10-22. The owner allowed a few extra seconds to retain more of the original
phrases and specified PSP text as KB with Kaneegabell Computer Entertainment
underneath. Earlier standalone previews are preserved.

- PS1 is 10.0 seconds, retaining source ranges 0–6.2 and 12.25–16.05 seconds.
  This adds one second before the final screen and one second of its chime tail.
  The final screen uses the established KB bull emblem, with no bottom KB footer.
- PS2 is 9.6 seconds, retaining source ranges 0–7.6 and 14.4–16.4 seconds.
  Towers remain in motion for two more seconds before the title transition.
  The company line and camera acceleration are paced for that longer sequence.
- PSP remains 6.165 seconds because both supplied recordings are already used
  completely. Its final screen now has a thin uppercase KB monogram, with
  `Kaneegabell Computer Entertainment` centered below it. The waves-only stage,
  white transition and gold flare remain aligned with the launch sound.

The PS1 final cue moves to 6.32 seconds and PS2's title midpoint to 7.79 seconds,
matching their longer edited audio timelines. PSP's cue remains at about 3.93
seconds. No original MP3 was modified; edits use temporary WAV copies.

Checks include full decode, browser playback, seeking and completion, edited
source/output audio-envelope comparison, and decoded-frame contrast against
the measured audio attacks. Audio/video lengths agree within one frame.
`verification.json` records cue timing and checks. Font usage and rendering
dependencies follow the previous revisions; no fonts are installed or bundled.

Render with the existing local font environment variables and tools:

```powershell
node tools/sounds/render-intro-videos.mjs --sony-revision docs/intro-video-previews/sony-revision-5
```

Use a new directory or `--replace` for these known outputs.
