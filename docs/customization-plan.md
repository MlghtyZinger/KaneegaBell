# Milestone u: customization proposal

Prepared 2026-10-22. Owner selected Compact folding rows on 2026-10-22.
No version number selected.

Interactive design preview prepared 2026-10-22 in the chat: Compact folding rows,
Spaced folding groups, and Optional grouping. These are mockups; the application
was unchanged at preview time. Preview interactions were checked in a temporary headless browser.

Implemented first: compact maker folding in the desktop sidebar, remembered state,
controller A toggling, stable group focus, and the existing flat shoulder-button
platform cycle. The other customization items below remain planned. The owner also
requested an intro synchronization check and PS3 menu music; those fixes passed
browser/native checks and are ready in the developer executable. See `audio-sync-qa.md`.

The owner also requested research into making ScreenScraper the primary metadata
provider using RomM's process, without account/key setup. See
`screenscraper-romm-research.md` for verified requirements and guest-access limits.
This research does not approve or implement a provider replacement.

## Starting state

- Local HEAD: `cf29965`, tagged `v0.3.13`. GitHub confirms a published release
  with installer, signature and manifest; installer size and GitHub SHA-256
  match the handoff (9,636,275 bytes, `e91dec03af230ed4e9ae48a087bb09fd3e1084791e37cb701b5992344f22bea9`).
  This session checked release metadata, not a fresh installer download/signature verification.
- Preserve the existing edits to `tools/release/release.ps1`,
  `docs/release-0.3.13.md`, and the untracked handoff. No commit or push of them.
- The installed application and all developer/installed library data remain untouched.

## What already exists

| Capability | Existing implementation | Remaining work |
|---|---|---|
| Grid/list view and card sizing | `src/state/library.ts`, Settings, library view | Reuse existing controls |
| Collection sorting | `src/state/collections.ts`, collection editors | Preserve collection-specific ordering |
| Recent games | Last-played ordering in `src/state/library.ts` | Preserve Recent's default behavior |
| Sidebar modes | Hover/open/closed, persisted `railMode` in Library | Add maker groups and navigation preferences |
| Empty platforms | `get_library` currently excludes zero-game platforms | Return enabled empty platforms; apply visibility preference in UI |
| Startup mode | Fullscreen and TV startup settings | Add a separate start-library-tab preference |
| Text sizing | Interface scaling and TV text presets | Add desktop text presets without changing card zoom |
| Controller hints | Automatic PS/Xbox choice in `Hints.tsx` | Add override and explicit generic hints |
| Settings persistence | Existing SQLite key/value settings and API | Add allowlisted, validated file transfer |
| Date added | SQLite `games.added_at` exists | Include it in the library payload for sorting |

## Proposed implementation order

1. **Library preferences and sorting.** Add a persisted sort picker using existing
   Picker controls: title, last played, play time, release year, date added, with
   appropriate direction choices. Keep stable title/id tie breaks and unknown
   dates last. Keep the current default order, Recent order and collection order
   unless the player explicitly selects another sort. When platform sections are
   shown, sort within sections; otherwise sort the whole displayed list.
   Preserve selected game by id when order changes. Check letter-jump behavior
   for non-title sorts, rather than applying the current title-order assumptions.
2. **Startup and platform navigation.** Settings controls for start tab,
   hide empty platforms, visibility, and order. Hiding a navigation entry must
   not disable scanning or remove its games from All/search. Invalid or hidden
   startup tabs fall back to All. Reuse preferences in desktop and TV platform
   selectors; desktop maker folding does not hide platforms from shoulder-button
   cycling. Provide controller-accessible Show/hide and Move up/down actions.
3. **Folding maker groups.** Use the selected option below; remember each group's
   open state. A toggles a group; A on a platform selects it. Keep group-heading
   focus separate from actual platform tabs. Collapse and preference changes
   retain a valid focus target. Include an Other group for future platforms.
4. **Readability and hints.** Auto / Xbox / PlayStation / generic glyph style.
   Keyboard hints continue to follow keyboard use. Reuse shared hint generation
   and audit hardcoded labels in TV and settings. Desktop text presets should
   work alongside existing interface scaling and independent TV text presets.
5. **Settings transfer.** Export a versioned JSON file of explicitly allowed
   preferences (appearance, audio/motion, library navigation/view/sort, glyphs,
   text and exit combo). Exclude keys, update access codes, library contents,
   cache/migration markers, machine-specific window/display state, ROM paths and
   emulator configuration. Validate file size, schema, types, enums and ranges
   before any write; preview changes and confirm import; apply in one transaction.
   Use existing in-app file browser for controller access and show useful errors.

## Sidebar visual options

All options retain All, Favorites, Recent and Collections when applicable, plus
the existing sidebar mode button. Counts below are illustrative.

### A. Compact folding rows (recommended)

```text
All games                     90
Favorites                     12
Recent                        20
Collections
--------------------------------
v Nintendo                    35
    Game Boy Advance           8
    GameCube                  12
    Wii                       15
> PlayStation                 40
> Sega                         9
> Microsoft                    6
```

Slimmer expanded platform rows, existing icons/colors, maker total at the right.
Collapsed icon rail shows maker entries; entering the sidebar exposes names
according to the existing hover/open/closed mode. In always-closed mode, the
top bar names the focused maker/platform as it names focused tabs today.
Group expansion exposes its platform icons in the rail too.
Reorder makers and platforms within their maker in Settings.

### B. Spaced folding groups

```text
All / Favorites / Recent / Collections
--------------------------------
v NINTENDO                    35
    [icon] Game Boy Advance    8
    [icon] GameCube           12
    [icon] Wii                15

> PLAYSTATION                 40

> SEGA                         9

> MICROSOFT                    6
```

Same folding/navigation rules as A; more spacing and stronger maker headings.
Easier visual separation, fewer platforms fit vertically.

### C. Optional grouping

```text
Group platforms by maker      Off / On

Off: current flat icon/name/count rows
On:  compact folding rows from option A
```

Adds one preference for players who want arbitrary cross-maker ordering.
More combinations to maintain; recommend A unless flat ordering is important.

## Validation after approval

- Focused tests for sorting ties/missing values, start-tab fallback, visibility
  and order, maker folding/focus, and settings import rejection/atomicity.
- TypeScript and existing TS tests; Rust library tests for backend changes.
- Controller-only checks for all new controls, collapsed groups and imports;
  mouse/keyboard checks; empty/small/5,000-game synthetic libraries.
- Layout checks at 720p, 1080p and 4K, including larger text; preserve TV menus,
  reduced/off motion, audio pause, and game-running behavior.
- Before heavy tests/builds, ask whether the owner is playing and check actual
  host KaneegaBell processes. The owner closes their app if necessary.
- With explicit permission, run the performance gate because startup and grid
  behavior are touched. Retain the saved baseline and disclose deviations.
- Build the developer executable only after validation and owner availability.
  No publishing, tagging, installer changes or release follow-up commits.

## Scope limits

No theme editor, colors/borders customization, `.kbtheme` transfer, new console
styles, achievement window, dependencies, or build configuration changes.
General button remapping and missing-game display are outside the later u list.
