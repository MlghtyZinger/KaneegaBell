# Activity center validation - 2026-11-13

Owner requested visible progress for scans and downloads, including covers, icons and every artwork type, with an expandable browser-style panel. The owner explicitly requested a development build and closed KB for replacement.

## Implemented

- Persistent compact Activity status bar, toolbar controls in Library and Settings, TV menu entry and Ctrl+J. Expandable panel uses existing controller navigation; Escape/Back dismisses and Tab stays inside the panel. Progress updates preserve the focused row.
- Current work first, followed by queued library jobs and up to 20 recent results. Known totals use real counts/percentages or download bytes; unknown totals are indeterminate, with no invented percentage. A final progress event does not itself imply success.
- Existing scan, cover, LaunchBox, Wikidata, IGDB, SteamGridDB artwork, achievement and updater events feed the panel. Individual artwork search/apply, collection artwork, manual preparation/search and explicit achievement refresh requests show their own activity and errors. Cached tile/image reads do not create entries.
- Added real icon-generation progress/results/errors and incremental library refreshes during icon creation. The shared worker now reports queued jobs and waiting for a running game. Stop library jobs clears queued work and requests cancellation after the current item.
- Added manual-matching counts and Steam scan/artwork-cache stages. Scans also queue needed icon backfills for newly imported cached covers. No new scheduler, runtime dependency, idle polling, provider request or credential storage.
- Session-only history. Failed/partial jobs remain visible until cleared; retry is performed from the original screen. The Stop control affects the library worker, not separate manual requests or update installation.

## Checks completed

- TypeScript: passed.
- JavaScript suite: 45 passed, 0 failed (includes real progress/unknown-total/clamping checks).
- Native Rust suite: 297 passed, 0 failed, 17 ignored. Icon tests verify counts and cancellation without claiming unfinished items; existing manual and Steam import tests verify their new progress callbacks. Log: test-output/activity-native-tests.log.
- Headless browser checks against the actual Solid activity component: counted and unknown-total progress, queued-to-running transitions, completed/partial/failed results, stopping queued work, manual artwork request failures, stable focus during updates, keyboard/controller dispatch, bounded history and clearing only finished entries. Passed at 960x540, 1280x720 and 1920x1080, with no page errors and no added interval timers. Light Wii and dark Forge appearances inspected. Report: test-output/activity-browser-report.json.
- Full App synthetic checks: Library, Settings and TV controls, progress visibility and popup access passed with no page errors. Inspected test-output/activity-library.png, activity-settings.png and activity-tv.png.
- Browser checks used isolated headless profiles, mocked IPC and local synthetic fixtures only; external requests were blocked. No real scans, downloads, game launches or installed-copy changes were performed for these checks.
- Whitespace check passed. Existing sound-preview drafts in the original checkout were preserved.

## Limits

These browser tests are not a physical-controller or native WebView performance measurement. The prior native performance gate remains failed and its baseline has not been reset. No new performance pass is claimed. Artwork/manual operations without byte totals show indeterminate progress; the panel does not invent download sizes or completion times.

The source and build are in the separate codex/steam-integration checkout. The development executable is the only requested runtime replacement; the installed copy at C:/Games/KaneegaBell remains untouched. The final build log and result are recorded below when complete.

## Final development build

The final no-bundle Tauri build passed (exit 0), including the verified light-theme contrast and keyboard focus refinements. Frontend build passed; optimized Windows build took 3m 30s. Output: C:/Emulator files/src-tauri/target/release/KaneegaBell.exe. Log: test-output/activity-development-build-final.log. The elevated process check confirmed KB was closed before replacement. The temporary UI test servers were stopped. KB was not launched automatically, the development library was preserved and the installed copy was not modified. No merge or publication was performed.
