# KaneegaBell (KB)

A fast, controller-first library frontend for emulators (a lightweight LaunchBox/Big Box
alternative). It organizes and launches your existing game dumps with your existing emulators.
It never downloads ROMs, BIOS files, or any game content.

Stack: Tauri v2 (Rust) + SolidJS/TypeScript + SQLite + SDL3 (controllers).

## Setup (Windows 10/11)

Prerequisites:
- Visual Studio 2022 Build Tools with the "Desktop development with C++" workload
- WebView2 runtime (preinstalled on Windows 11)
- Rust (stable) and Node.js LTS:

```bash
winget install Rustlang.Rustup OpenJS.NodeJS.LTS
```

- CMake, for the SDL3 source build. VS Build Tools ships one; if `cmake` isn't on your PATH,
  point cargo at it with a git-ignored `.cargo/config.toml` in the repo root:

```toml
[env]
CMAKE = { value = 'C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin\cmake.exe', force = false }
```

Then from the repo root:

```bash
npm install
```

```bash
npm run tauri dev
```

Release build. The app is `src-tauri/target/release/KaneegaBell.exe`; drop
`-- --no-bundle` to also get an NSIS installer in `src-tauri/target/release/bundle/`:

```bash
npm run tauri build -- --no-bundle
```

Logs go to stderr; set `RUST_LOG=kaneegabell_lib=debug` for more detail.

Rust unit tests:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

### Where KB keeps its files
Everything stays in **KB's own folder**, in `data\` next to `KaneegaBell.exe` (for example
`C:\Games\KaneegaBell\data`):
- `library.db` (games, play time, favorites, settings), `art\` (covers, backgrounds, logos),
  `ra\` (achievement badges), `playlists\` (generated multi-disc `.m3u` files);
- `keys.bin`: your API keys, encrypted with your Windows login (DPAPI), so a copy can't be read by
  another Windows user or on another PC (keys are re-entered there);
- `webview\`: WebView2's own files (its cache).

Updates replace only the program files; a normal uninstall leaves `data\` in place, and ticking
"Delete the application data" in the uninstaller removes it. If KB's folder can't be written to
(for example under Program Files), KB uses `%APPDATA%\com.kaneegabell.kb` instead and says so.
The only things outside the folder are the Start menu shortcut and uninstall entry (Windows'
own), and the update installer for the few seconds it runs from Windows' temp folder.

Earlier versions kept the library in `%APPDATA%\com.kaneegabell.kb` (or, before the KaneegaBell
rename, `%APPDATA%\com.sultanmatoo.emufrontend`) and the keys in Windows Credential Manager. On
first start the installed app copies the library into `data\`, checks the copy (integrity check,
same number of games), switches to it and leaves the old folder untouched; the keys move into
`keys.bin` (read back to check, then removed from Credential Manager).

A **development copy** (run from `src-tauri\target\...`) keeps its own separate library in
`target\release\data`, starts empty and moves nothing (note: `cargo clean` deletes it).
`KB_DATA_DIR` points KB at any other folder (the perf harness's test library), with its own keys.

## Installing (for players) and updates

KaneegaBell is shared privately: get the installer (`KaneegaBell_<version>_x64-setup.exe`) and an
**update access code** from whoever gave you KB, and run the installer:

- No administrator rights needed: it installs for your Windows user (pick a folder you own, e.g.
  `C:\Games\KaneegaBell`), adds a Start menu entry and an uninstaller (Settings → Apps). If the
  PC somehow lacks Microsoft's WebView2 runtime, the installer fetches it.
- The installer isn't code-signed, so Windows SmartScreen may say "Windows protected your PC" the
  first time: choose **More info → Run anyway**.
- Your library, settings and keys stay in KB's `data\` folder; updates never touch them. Get
  emulators only from their official sites.

**Updates:** paste the access code once in Settings → General → Updates. KB then checks once,
~10 s after it starts (or "Check now"), and offers new versions with their changelog: **Update now
/ Later / Skip this version**. Updating never happens while a game runs; KB downloads the
installer, verifies its signature against the key built into the app (anything not signed by the
project's key is refused), closes, installs and starts again by itself. A code lasts up to a year;
when it expires KB says so, and a new one is pasted the same way. Copies run from a build folder
(`target\...`) don't check automatically.

## Making a release (maintainer)

One-time setup:
- The updater signing key in `%USERPROFILE%\.tauri\kaneegabell.key` (made with `tauri signer
  generate`), backed up with its password. Its public half is in `src-tauri/tauri.conf.json` →
  `plugins.updater.pubkey`. **If the key or password is lost, installed copies can never update
  again.** Never commit it (`*.key` is git-ignored).
- GitHub's `gh` tool (`winget install GitHub.cli`), signed in once: `gh auth login` (GitHub.com,
  HTTPS, log in with a web browser).

```bash
powershell -ExecutionPolicy Bypass -File tools/release/release.ps1 -Version 0.3.0 -Publish
```

First add the version to **Patch notes** (at the end of this file) and commit it: the script uses
those lines as the changelog shown in KB's update window (`-Notes "..."` or `-NotesFile` override
that). It checks the work tree is clean, sets the version in every file, runs the tests, builds the
installer signed with the key (asking for the password; nothing is stored), commits "Release
v0.3.0" and tags it. With `-Publish` it asks for confirmation, pushes, creates the GitHub release
with the installer and its `.sig`, and puts the installer and `latest.json` on the repo's `updates`
branch: that's what installed copies read, through GitHub's contents API with their access code
(so it works with a private repo), and the script downloads it back the way KB does to check it.
Nothing is uploaded by hand. `-Repoint -Version X` redoes just that step for a published version. Day-to-day builds (`--no-bundle`) don't need the key.

### Update access codes (one per person)
A code is a GitHub fine-grained token that can only *read* this repository:
1. github.com → your picture → **Settings → Developer settings → Personal access tokens →
   Fine-grained tokens → Generate new token**.
2. Name it after the person (e.g. "KB updates: Alex"), expiration up to 1 year.
3. **Repository access: Only select repositories → KaneegaBell.**
   **Permissions → Repository permissions → Contents: Read-only** (Metadata: read-only is added
   automatically). Nothing else.
4. Generate, copy the token (shown once) and send it privately; they paste it in Settings →
   General → Updates. Revoke a person's token to stop their updates.
With a code someone can read the repository (code and releases), nothing more. Make a new one
before it expires (GitHub emails you a reminder).
## First run

1. **Settings → Emulators → "Find emulators in a folder…"** and pick the folder that holds your
   emulators (e.g. `E:\Emulators`). It fills in every emulator it recognizes and the RetroArch
   cores folder. Or set each path by hand.
2. **Settings → Library → ROM folders**: add each ROM folder. The platform is preselected
   from the folder name ("PS2", "GBA", "GameCube"...); change it if needed.
   - **Auto** works out each file's platform from its type and contents: GameCube vs Wii from
     the disc header, PS1 vs PS2 from the disc's `SYSTEM.CNF`, PSP and Sega CD from their disc
     signatures. When the contents can't decide, folder names along the path ("PS2", "PSX")
     break the tie. Anything still ambiguous is listed under the scan's warnings.
   - A folder assigned to GameCube or Wii accepts both; each disc is placed by its header.
   - **PS3**: assign disc-dump folders (containing `PS3_GAME`) to PlayStation 3, and use
     "Add RPCS3 installed games" for PSN/HDD titles in RPCS3's `dev_hdd0`.
3. **Scan library.** Rescans are incremental, and games whose files disappear are hidden (not
   deleted), so play time and favorites survive a drive being unplugged.

Keyboard: arrows move, `Enter` details (and `Enter` again to play), `Q`/`E` (or PgUp/PgDn)
switch platform, `Home`/`End` jump by letter, `F` favorite, `F1` settings, `Esc` back, `F11`
fullscreen.

### Storage and reset (Settings → General)
Both only delete inside KB's data folder (`data\` next to the program); game files,
emulators and saves are never touched.
- **Clear downloaded data**: deletes downloaded covers, backgrounds, logos, game info, achievement
  data and cached web responses, then fetches them again in the background. The library, play
  time, favorites, folders, settings, keys and anything you picked yourself (covers,
  backgrounds, logos, "Fix match" choices) stay.
- **Reset KaneegaBell**: back to a first start (library, play time, favorites, folders, emulator
  paths and settings are forgotten; optionally the saved keys too). A copy of the old library is
  kept as `library.before-reset.db`. To undo: close KB, delete `library.db`, `library.db-wal`
  and `library.db-shm`, rename `library.before-reset.db` to `library.db`, start KB, then use
  Clear downloaded data so the covers come back.

## Controllers

| Action | PlayStation | Xbox | Keyboard |
|---|---|---|---|
| Move | D-pad / left stick (hold to repeat) | D-pad / left stick | Arrows |
| Select / details, then Play | Cross | A | Enter |
| Back / close | Circle | B | Esc |
| Search | Square | X | `/` |
| Favorite | Triangle | Y | F |
| Previous / next tab (platform, Settings section) | L1 / R1 | LB / RB | Q / E |
| Previous / next letter | L2 / R2 | LT / RT | Home / End |
| Settings | Options | Menu | F1 |
| Bigger / smaller cards | right stick up / down | right stick up / down | `+` / `-`, Ctrl + wheel |
| Platform sections on / off (All, Favorites, search) | R3 | RS (click) | V |
| Genre filter (with IGDB) | Share | View | G |
| **Exit the running game** | **hold Share + Options 1 s** | **hold View + Menu 1 s** | |

The exit combo is configurable in Settings → General. Every screen works with a controller
alone: D-pad moves focus to the nearest control, A/Cross presses it, and the hints bar shows
the buttons for the device you used last.

- **Text fields** open an on-screen keyboard (□/X space, ○/B delete, △/Y shift, Options/Menu
  done). A physical keyboard types into it too.
- **Search** (□ / X / `/`) searches every platform as you type; ○ / B clears it.
- **Dropdowns** are pickers you can open with A, and **Browse…** opens an in-app folder/file
  browser (A enters, B goes up, □ / X picks the current folder). The native Windows dialog
  can't be used with a controller.
- **Tabs**: All, ★ Favorites and Recent (most recently played first) appear next to the
  platforms once they have games.

Controller input uses SDL3 rather than gilrs: gilrs' Windows backend (Windows.Gaming.Input)
only delivers input to the focused window, so the exit combo couldn't work while an emulator
has focus. SDL3 reads XInput, raw input and HIDAPI (native DualShock 4 / DualSense) with
background events enabled. It is told not to change PlayStation pads' report mode or LEDs, so
emulators reading the same pad see it unchanged. When Steam Input or DS4Windows exposes a
virtual Xbox pad next to the real one, the same press arriving from both is counted once.

## Launching and play time

- The emulator is started directly (no shell) from the platform's launch template, inside a
  Windows Job Object. The session ends when the last process in the job exits, so emulators
  that spawn child processes are tracked correctly, without polling.
- While a game runs, the frontend minimizes (WebView2 stops rendering) and unmounts the
  library. When the game exits, it comes back to the front, fullscreen again if it was
  fullscreen, with focus on the game you launched.
- Exit combo / "Close game": the emulator's windows get a normal close request (so it can save),
  then the whole process tree is terminated after 4 seconds if it hasn't exited.
- Multi-disc games without an `.m3u` get one generated in the app's data folder (never next
  to your files) for emulators that support playlists (RetroArch, DuckStation, Dolphin), so
  in-emulator disc swapping works. PCSX2 and RPCS3 boot disc 1.
- Play time is the session's wall-clock duration, added when the game exits. If the frontend
  crashes mid-session, "last played" is kept but that session's duration isn't guessed.

## Covers and metadata

Everything works without accounts or API keys:

- **[LaunchBox Games Database](https://gamesdb.launchbox-app.com)** (keyless, used first): box
  art for your copy's region, clear logos and backgrounds (TV Mode), descriptions, release
  dates, developers, publishers, genres and player counts, for 12 platforms (~30,000 games).
  - LaunchBox publishes everything as one daily `Metadata.zip` (~100 MB, no per-game access).
    KB downloads it only when there's something to gain (no data yet, games whose details aren't
    stored, or LaunchBox changed while some games still have no match, checked at most weekly
    with a tiny request; or "Update now"), reads it as it streams in (about 2 seconds of CPU) and
    keeps only a name list for your enabled platforms plus your games' details (a few MB in
    `data\`). Pictures are fetched just for your games.
  - Matching: same platform, same cleaned-up title (main name or another region's title, e.g.
    "Slap Stick (Japan)" → Robotrek), then a close title with matching sequel numbers. Tested on
    1,776 real No-Intro/Redump names: 91% matched (93–99% on most platforms), correct in spot
    checks; when two entries fit equally, nothing is guessed: **Fix match** lists LaunchBox's
    entries (searched offline) and your pick is kept for good.
  - Settings → Game info can turn it off. Credit: game info and pictures from the LaunchBox
    Games Database, made by its community. LaunchBox staff have said other apps use the file.
- **[Wikidata](https://www.wikidata.org)** (keyless, CC0): each game's **series** and
  **franchise** (e.g. "Jak and Daxter", "Final Fantasy") and its **number** in the series
  ("series ordinal"). The game details show the series and number above the title, and a series
  card under Play with the other games you have from it, in series order (select one to open it).
  - KB asks Wikidata's query service once per platform with games (its games' English titles,
    alternate titles, first release year and IGDB ID; a few seconds each, one at a time), matches
    your games in memory, then asks for the series of the matched games only. Only those entries
    are kept (a few KB in `data\`). A platform is asked again only for new games, once a month
    while some games have no match, or on "Update now".
  - Matching: the IGDB match when Wikidata lists it (exact), else the LaunchBox match's title
    (better than file names) or the game's own, by the LaunchBox rules; when two entries share a
    title, the release year decides, else nothing is guessed.
  - The service asks apps to identify themselves: KB sends `KaneegaBell/<version>
    (https://github.com/MlghtyZinger; personal game library)` and stays well inside its limit
    (60 s of query time per minute). Settings → Game info can turn it off.
- **Box art** also comes from [libretro-thumbnails](https://thumbnails.libretro.com) (the set RetroArch
  uses), matched by game name: exact No-Intro/Redump file name first, then the cleaned title
  with your region preferred, then a strict word-overlap match for loose names. Sequel numbers
  must agree, so a wrong game is never picked just because the titles look alike.
- **[GameTDB](https://www.gametdb.com)** covers (the source Dolphin and the Wii USB loaders use)
  are looked up by the game's own ID read during the scan: PS3 title ID, GameCube/Wii disc ID,
  DS/3DS game code. It comes first for PS3, where libretro has few covers, and fills gaps
  elsewhere. A PS3 game's own icon (`ICON0.PNG`) is never picked automatically; it's offered
  in the cover picker.
- Covers are fetched in the background after each scan (toggle in Settings → Library), cached
  as small WebP thumbnails for the grid plus a larger image for details, and served from disk,
  so they work offline. Games without a match are retried weekly; "Refresh" in a game's
  details fetches again, and "Change cover…" lets you pick another one (other regions, a
  search of the whole cover set, or an image file). Your choice is never overwritten.
- Each card takes its cover's shape (a SNES box is a wide card, a GBA box a square one), and a
  platform tab sizes its grid for that platform's usual cover shape. All, Favorites and search
  results are shown in one section per platform, so each row holds covers of one shape (no gaps
  inside rows); R3 / V switches to a single list.
- **Game info from IGDB (optional, free key):** descriptions, release dates, developer,
  publisher and genres, plus a genre filter in the library (Share / View / `G`). Set it up in
  Settings → Game info:
  1. Sign in at [dev.twitch.tv/console](https://dev.twitch.tv/console) with a free Twitch account
     (Twitch requires two-factor authentication).
  2. **Register Your Application**: any name, OAuth Redirect URL `http://localhost`, category
     *Application Integration*, client type **Confidential**.
  3. Open the app, copy the **Client ID**, press **New Secret** and copy the **Client Secret**.
  4. Paste both in Settings → Game info and press **Save and connect** (the keys are checked with
     Twitch before they're saved).

  The keys are stored in `data\keys.bin` (encrypted with your Windows login), never in the
  database or the repo. Lookups run in the background after covers: games are searched by title
  on their platform (one search per request, at most 4 requests/second, IGDB's limit) and matched with
  the same title rules as covers; matched IGDB ids are stored, so later refreshes are direct
  lookups and details work offline. A wrong match can be fixed from the game's details
  (**Fix match…**); your pick is never replaced. IGDB is free for non-commercial use under the
  Twitch Developer Services Agreement.
- **SteamGridDB (optional, free key):** community artwork. A background pass finds each game on
  SteamGridDB by title and stores its best-voted hero (wide background) and logo for TV Mode,
  plus a cover for games none of the other sources had. The cover picker lists SteamGridDB
  covers too, and a game's details have **Background…** / **Logo…** pickers (your choice
  sticks; "No background/logo" removes one). Set it up in Settings → Game info:
  1. Sign in at [steamgriddb.com](https://www.steamgriddb.com) (with Steam).
  2. Open [steamgriddb.com/profile/preferences](https://www.steamgriddb.com/profile/preferences)
     and generate an API key in its API section.
  3. Paste it in Settings → Game info and press **Save and connect** (checked with one search
     first, then stored encrypted in `data\keys.bin`).

  The key is sent as a Bearer token; SteamGridDB publishes no rate limit, so requests are
  sequential, 250 ms apart, with a back-off on HTTP 429. Only static images without NSFW, humor
  or epilepsy tags are used. Its terms allow personal, non-commercial use and forbid scraping:
  the app only uses the documented API. Games not found are retried weekly.
- MobyGames isn't used: its API now requires a paid subscription.

## TV Mode

A big-screen view in the spirit of Steam's Big Picture, for the couch: switch with the left stick
button (L3), `F10` or the **TV Mode** button; Settings → General can start the app in it. It goes
fullscreen and shows the focused game's SteamGridDB background (crossfading and slowly drifting;
a blurred cover when there's none), its logo or title, platform, year, developer, genres, play
time and description, with a shelf of covers along the bottom. LB/RB switch platform, LT/RT jump
by letter, A opens the game's details, X searches, Y favorites, Start opens a menu (Search,
Settings, Desktop mode). It shares the desktop library's tabs, search, filters and focus.

Measured on the 5,000-game test library: holding right steps through the shelf with every frame
under 14 ms; idle (background drifting) 0.2% of one core.

Settings → General → TV Mode:
- **Display**: open TV Mode on a chosen display (listed by the monitor's own name, e.g. "SONY TV").
  KB moves there and goes fullscreen; leaving TV Mode puts the window back. If that display is
  off or unplugged, TV Mode opens where KB is and says so.
- **Text size**: Small / Medium / Large / Extra large.
- **Overscan margin**: Off / 2.5 / 5 / 7.5%, for TVs that cut off the picture's edges.

## Screens, scaling and monitors

- Works from a 960×540 window up to 4K, ultrawide (21:9) and super-ultrawide (32:9), at Windows
  scaling from 100% to 300%.
- **Interface size** (Settings → General → Display): "Fit the screen" (default) zooms the page on
  screens bigger than 1080p-equivalent, so a 4K TV at 100% scaling looks like 1080p instead of
  tiny text in a sea of covers. Screens up to 1080p, or with Windows scaling turned up (a 1440p
  monitor at 150%), are untouched. "Follow Windows scaling only" turns the zoom off.
- KB remembers its window's position, size and maximized state, per display. If that display
  is gone next time, Windows' default placement is used instead of opening off screen.
- Resolution, scaling and display changes while KB runs are picked up without a restart.
- `tools/perf/screens.mjs` screenshots every main screen at 8 display sizes × 4 scaling levels
  and checks for sideways scrolling, content past the window edge and cut-off text. Run it
  against the mock UI in a browser started with `--remote-debugging-port=<port>`, or KB started
  with `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=<port>`:
  `node tools/perf/screens.mjs <port> perf-lib/screens` (writes an `index.html` gallery).

## RetroAchievements

Optional: each game's details show its achievements, your unlocks (softcore and hardcore),
points and how rare each one is. Set it up in Settings → Game info:

1. Sign in at [retroachievements.org](https://retroachievements.org) (free account, the one your
   emulators use).
2. Open [retroachievements.org/settings](https://retroachievements.org/settings) and copy your
   **Web API Key**. It works like a password for your account's data.
3. Enter your username and the key and press **Save and connect** (they're checked with RA first,
   then stored encrypted in `data\keys.bin`, never in the database or the repo).

How games are recognized: by RetroAchievements' own fingerprint of the game file, the same
method its emulators use (the rcheevos hashing rules, reimplemented in
`src-tauri/src/achievements/hash.rs`), so file names don't matter. That's whole-ROM MD5 for
GBA, the header-stripped ROM for SNES, byte-order-normalized ROM for N64, header + code + icon
for DS, the boot executable for PS1/PS2/PSP (read from the disc's file system, `.cue`/`.bin` or
`.iso`), the first sector for Sega CD, apploader + main executable for GameCube and Wii
(`.iso`, and Wii `.wbfs` read through its block table), and `PARAM.SFO` + `EBOOT.BIN` for PS3
folders. Compressed formats (`.chd`, `.rvz`, `.cso`…) aren't hashed yet. 3DS shows no
achievements (hashing needs console keys).

- Hashing runs in the background after scans (a file is only re-hashed when it changes), then
  each hash is looked up in RA's list of known hashes for that console, cached for a week. A
  console RA doesn't currently support shows no achievements section at all.
- When RA doesn't recognize a file, the details say why: if RA has achievements for a game by
  that title, it lists the versions RA accepts (often another region, or a dump with an
  official RA patch applied, marked "RA patch"); otherwise it says no achievements were found
  for that title on that console (matched by title, so a game RA lists under another name,
  like a Japanese title, can land here). The file's hash is shown either way.
- Unlocks are re-checked when you open a game's details (at most every 10 minutes; Refresh
  forces it) and shown from the cache when offline. Badge images are cached on disk.
- RA rate-limits its API without publishing numbers, so requests are sequential and spaced
  half a second apart.

## Scanning rules

- Only files with the platform's extensions are considered, so saves, states, patches and
  readmes never appear.
- Files referenced by an `.m3u` or `.cue` (and `.ccd`/`.mds` sidecars) are hidden, so a game
  appears once.
- Discs named `(Disc N)` / `(Disc N of M)` / `(CD N)` in the same folder are grouped into one
  entry. A real `.m3u` always wins over grouping.
- Auto folders also skip what is never a game: folders named like caches, shaders, logs,
  saves, states, BIOS or firmware; disc images under 1 MB (Amiibo dumps, save data); plain
  `.gz` files (only `name.iso.gz` counts); and folders named for a platform that isn't set up
  yet ("Switch", "PS4", "Xbox"...). Those are scanned automatically once that platform is added.
  PS3 game folders and an RPCS3 `dev_hdd0` are recognized inside Auto folders.
- Entries whose files disappear are deleted unless they have play time or are a favorite. Those
  are kept hidden, so they come back intact if the files return.
- After each scan (and when a folder is removed) a notice says what changed, and Settings →
  Library lists every game added, removed, hidden or moved to another platform, by platform.
- Titles are cleaned for display (`Legend of Zelda, The - The Wind Waker (USA)` → "The Legend
  of Zelda: The Wind Waker"), with hack/translation tags kept as a subtitle.

To scan a real library from the command line without the UI (read-only):

```bash
EMU_REAL_SOURCES="E:\Games\GBA=gba;E:\Hacks=auto" cargo test --manifest-path src-tauri/Cargo.toml real_library -- --ignored --nocapture
```

## Adding a platform

1. Add a `PlatformDef` to `PLATFORMS` in `src-tauri/src/platforms.rs` (id, names, emulator,
   default core for RetroArch, extensions). If it needs a new emulator, add an `EmulatorDef`
   with its launch template, checked against that emulator's own CLI docs/source.
2. Make sure `src-tauri/src/scanner/hints.rs` maps the platform's usual folder names to the
   new id (Switch, PS4, Xbox, Saturn and others are already listed), so Auto scans pick up its
   folders and Settings preselects it.
3. Restart the app: built-in definitions are re-seeded on every start without touching your
   settings.
4. If its extensions overlap another platform's (`.iso`, `.cue`, `.chd`…), files in Auto
   folders stay ambiguous unless you add a header check in `src-tauri/src/scanner/detect.rs`.
   Folders with an assigned platform always work.

## UI development without Tauri

`npm run dev` and open http://localhost:1420 in a browser: outside Tauri, the dev build serves
mock data (`src/dev/mockIpc.ts`, 5,000 fake games; `?games=N` to change). The mock is a
dev-only dynamic import and is not part of release builds.

## Performance

Measured on a generated 5,000-game library (every game with a cover), release build,
Ryzen 7 7700X / RTX 4070 Ti / 32 GB, 1600×900 window at 150% scaling, 240 Hz display
(October 2026):

| Target (CLAUDE.md) | Result |
|---|---|
| Cold start to interactive < 1.5 s | **~530 ms** median of 5 starts (512–667 ms), process start → grid painted |
| Smooth 60 fps grid scrolling | **240 fps** (every frame ~4.2 ms, worst 10 ms, none over 20 ms) scrolling at 2,500 and 6,000 px/s, first pass included (thumbnails decoded on the fly) |
| Idle CPU near 0% | **0.3–0.9% of one core** (≈0.05% of the CPU), app + WebView2, library on screen |
| Modest RAM | 278 MB private / 410 MB working set after start (the app itself: 13 MB); after scrolling all the way through, WebView2's GPU process holds ~1 GB of cover cache (≈680 MB actually in RAM) |
| Minimal resources while a game runs | 362 MB private / 385 MB working set, 0.9% of one core: the grid is unmounted, the window minimized, and WebView2 told to drop its caches |
| Background scans | 5,000 games scanned in ~0.3 s (rescan with no changes ~0.15 s), off the UI thread |

Re-checked after milestone i (KaneegaBell 0.2.0): cold start 513 ms median, 240 fps, idle 0.31% of one
core, 284 MB private after start, 0.5-0.9% of one core while a game runs; all within 10%.

Milestone k (perf gate against the published 0.2.1, same PC and library): cold start 511 ms median (was 507),
idle 0.57% of one core (0.52), 287 MB private after start (289), every scroll frame 4.3 ms at 240 fps (worst
frame 4.3 ms, was 9.7), while a game runs 0.31% of one core (0.77; the page is now also suspended) and 355 MB
private (355). Passed.

Milestone s (LaunchBox) against 0.2.1, gate measuring CPU as the middle of several windows: cold start 451 ms
(was 499), 282 MB after start (283), scrolling 240 fps with p99 4.3 ms, game running 352 MB (350). CPU readings
stay within Windows' counter resolution (a few 15.6 ms ticks). Passed.

Milestone s part 2 (Wikidata), 3 runs: cold start 577 / 503 / 502 ms, 256–342 MB after start, scrolling 240 fps
with p99 4.3 ms, game running 309–362 MB. Each run tripped the gate on a different single number (one start, one
memory reading at another display scale, single-frame spikes of 10–21 ms, 1.5% CPU in one game window); the 0.2.1
baseline exe measured right after tripped it too (a 16.6 ms frame, a 46 ms spike in the fast flick, start 506 ms,
282 MB), so these were the PC's noise at the time, not the change. Nothing new runs at startup or while scrolling.

Only the visible rows exist in the DOM (≈40–55 cards), and the grid only ever decodes the small
WebP thumbnails. Reproduce:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --release --lib generate_perf_library -- --ignored --nocapture
```

with `PERF_ROOT` set to an empty folder (it generates tiny fake game files, scans them into
`PERF_ROOT\data`, and gives every game a cover). Then, with the app closed:

```bash
powershell -File tools/perf/perf.ps1 -Data <PERF_ROOT>/data
```

It starts the release app on that folder (`KB_DATA_DIR`, which also keeps it away
from your saved API keys) several times and reports start times, idle CPU, memory, frame times
(over the DevTools protocol) and the game-running state (a stand-in "emulator" that waits 25 s).

**Regression gate:** every run is compared with `tools/perf/baseline.json` and fails (exit 1)
if a number got more than 10% worse, beyond a per-metric noise floor (1% of a core for CPU, which
Windows counts in ~15.6 ms steps; 5 ms for p99 frame times). CPU is the middle of several short
windows, so a one-off spike doesn't decide it. `-CompareOnly` re-checks the last run without measuring. Save a new baseline with `-SaveBaseline` after a milestone
passes. The baseline is from the maintainer's PC (above); on other hardware, save your own first.

## Status

Built in milestones. See `CLAUDE.md` for the full spec.

- [x] 0. Repo + scaffold
- [x] a. Settings, library scanning, SQLite, basic grid
- [x] b. Launching, process tracking, exit hotkey, play time
- [x] c. Full controller navigation
- [x] d. Metadata (IGDB) and artwork (libretro-thumbnails, GameTDB, IGDB, SteamGridDB)
      (MobyGames dropped: paid API)
- [x] e. RetroAchievements
- [x] f. Performance pass (see Performance)
- [x] g. TV Mode: a switchable Big-Picture-style view with animations (desktop mode stays the
      main, lean UI), with SteamGridDB heroes and logos
- [x] h. Backloggd: a **Backloggd ↗** button in each game's details opens its Backloggd page in
      your browser (Backloggd's pages use IGDB slugs, stored with the IGDB match; without a match
      it opens a Backloggd search for the title). Backloggd has no public API (checked Oct 2026)
      and scraping is out, so syncing your backlog/played status waits for an official API.
- [x] i. Renamed to KaneegaBell (data and saved keys migrated automatically)
- [x] j. Installer (per-user NSIS) and signed auto-updates from GitHub Releases
- [x] k. Screens, scaling and monitors (fit-the-screen zoom, window memory, TV Mode display,
      text size and overscan), perf regression gate, everything in KB's own folder
- [x] s (part 1). LaunchBox Games Database: covers, logos, backgrounds and game info without a key
- [x] s (part 2). Wikidata: series and franchises without a key

## Patch notes

Newest first, one change per line. The release script copies a version's lines into KB's update
window, so they're written for players.

### 0.2.6 (not released yet)
- New: Settings → General → Updates → What's new lists every update and what it changed.
- New: after an update, KB shows what's new once.
- New: game details show the series above the title, with the game's number in it (e.g. "Jak and Daxter · #2").
- New: a series card under Play shows where the game falls in its series and the covers of the other games you have from it; select one to open it.
- Changed: series are looked up once more after this update, to add the numbers.

### 0.2.5 (2026-10-15)
- New: game details show the game's series and franchise (e.g. "Jak and Daxter", "Final Fantasy"), from Wikidata, no account needed.
- New: Settings → Game info has a Wikidata section with Update now and an off switch.

### 0.2.4 (2026-10-14)
- New: covers, logos and backgrounds from the LaunchBox Games Database, no account or key needed.
- New: game info from LaunchBox (description, release date, developer, publisher, genres, players).
- New: TV Mode gets logos and backgrounds without a SteamGridDB key.
- New: Fix match can search LaunchBox as well as IGDB.
- New: Settings → Game info shows how many games LaunchBox found, with Update now and an off switch.
- Fixed: PSP covers in TV Mode show the whole box instead of being cropped.
- Fixed: the platform tabs in TV Mode no longer show a boxy focus outline.
- Fixed: updating from 0.2.3 failed with "signature verification failed" (fixed on the download side, no reinstall needed).

### 0.2.3 (2026-10-14)
- New: private updates: enter your update access code once in Settings → General → Updates.
- New: KB says when the access code is missing or has expired.

### 0.2.2 (2026-10-14)
- New: KB fits big screens and TVs, from 1280×720 to 4K, ultrawide included.
- New: Settings → Interface size: "Fit the screen" or "Follow Windows scaling only".
- New: KB remembers the window's monitor, size and position, also when fullscreen.
- New: TV Mode can open on a chosen display (e.g. the TV) and moves back when you leave it.
- New: TV Mode text size presets and an overscan margin for TVs that cut off the edges.
- Changed: all of KB's files (library, covers, keys, browser data) are kept in the `data` folder next to KB, nothing in AppData.
- Changed: saved keys are stored encrypted in that folder (moved automatically from Windows Credential Manager).
- Changed: uninstalling with "Delete the application data" ticked also removes the `data` folder.
- Improved: KB uses almost no CPU while a game is running.

### 0.2.1 (2026-10-14)
- New: Settings → General → Storage and reset.
- New: Clear downloaded data: deletes the downloaded covers, game info and achievement data, then fetches them again (your games, play time, favorites, keys and own picks stay).
- New: Reset KaneegaBell: back to a fresh start, optionally removing saved keys (a copy of the old library is kept).

### 0.2.0 (2026-10-14)
- New: the app is now called KaneegaBell (KB); your library, play time and keys moved over automatically.
- New: installer (no admin rights needed).
- New: signed automatic updates, with Update now / Later / Skip this version.
- New: Settings → General → Updates: Check now, and an option to turn update checks off.
