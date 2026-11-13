# Sidebar family icons and performance - 2026-11-13

The approved minimal sidebar now uses the owner's supplied Mario cap for Nintendo, PlayStation/Sega/Xbox symbols and a monitor for PC launchers. Other groups retain a generic controller symbol. Assets are bundled locally. The cap bitmap remains unchanged; a static color filter keeps the badge/M detail and inherits the theme's current icon color. All other family symbols use the same current-color behavior. A subtle selected pill and edge indicator replace the stronger fill; PS2 retains its existing selection treatment. Folding, platform order and input handlers are preserved.

## Functional validation

- TypeScript and release build passed. 46 JavaScript tests passed.
- Full app with synthetic IPC: five family symbols, selected cap tint, folded/expanded groups, 1024x768 / 1280x720 / 1920x1080. Checked all 13 shipped palette files (including legacy palettes); no page errors, external asset requests or sidebar clipping. Forge and Daylight screenshots visually reviewed.
- Final release build: 2m58s, no installer. Development executable: C:/Emulator files/src-tauri/target/release/KaneegaBell.exe. Installed copy untouched.

## Native performance comparison

Ran tools/perf/perf.ps1 twice against the same task-owned copy of the generated 5,000-game library and artwork. Owner confirmed rebuilding now and explicitly requested performance checks. No owner library, settings, game files or real emulator session were used. Game-running mode uses the generator's 25-second PowerShell stand-in. Existing tools/perf/baseline.json was compared unchanged, never reset.

Five startup runs and five idle sampling windows over 30 seconds per configuration. Native viewport: 1600x900 at 1.5x; monitor sampling reached 240 fps. First configuration: Forge/Full animations. Second: only the generated profile's animDesktop changed to off. The second run reused the profile and its WebView cache. This compares animation settings on the new build; it does not isolate sidebar cost from the previous executable.

| Measurement | Saved baseline | Full | Off |
| --- | ---: | ---: | ---: |
| Startup median (ms) | 426 | 497 | 485 |
| Idle CPU (% of one core) | 0.52 | 19.85 | 0.77 |
| First scroll maximum frame (ms) | 4.5 | 4.5 | 4.4 |
| Second scroll maximum frame (ms) | 9.4 | 4.3 | 4.3 |
| Startup Task Manager memory (MB) | 110 | 121.0 | 118.0 |
| Scrolling Task Manager memory (MB) | 212 | 210.0 | 201.0 |
| Game-running Task Manager memory (MB) | 102 | 110.0 | 110.0 |
| After-game Task Manager memory (MB) | 132 | 140.0 | 135.0 |

Both runs held 240 fps in both normal scroll passes and the fast flick; no frames exceeded 20 ms. Full startup range: 491-1450 ms; Off: 470-1095 ms. On this 16-thread host, Full idle CPU is 1.241% of total CPU; Off is 0.048%.

**Full gate FAILED**: median startup exceeded the baseline by 71 ms (+17%), and idle CPU was elevated. Startup still met the absolute 1.5-second target, including the slowest measured start. **Off gate PASSED** under the existing script's relative threshold and noise tolerances; this does not mean the Full configuration or the overall milestone passed. Private committed memory produced warning-only increases as recorded by the harness; Task Manager memory stayed within gating tolerances. The baseline remains unchanged.

Turning animations off sharply reduced idle CPU in this run, supporting the existing concern about animated backgrounds. No unrelated performance repair or user-preference change was made.

Evidence (ignored local outputs): test-output/maker-sidebar-build.log, maker-sidebar-js.log, maker-sidebar-ui.log, maker-sidebar-perf-full.log/.json, maker-sidebar-perf-off.log/.json; generated data under test-output/sidebar-perf-full/data. The performance harness closed its own test KB windows on completion.
