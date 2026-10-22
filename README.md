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
     the disc header, PS1 vs PS2 from the disc's `SYSTEM.CNF`, PSP, Sega CD and Saturn from their
     disc signatures. When the contents can't decide, folder names along the path ("PS2", "PSX")
     break the tie. Anything still ambiguous is listed under the scan's warnings.
   - A folder assigned to GameCube or Wii accepts both; each disc is placed by its header.
   - **PS3**: assign disc-dump folders (containing `PS3_GAME`) to PlayStation 3, and use
     "Add RPCS3 installed games" for PSN/HDD titles in RPCS3's `dev_hdd0`.
3. **Scan library.** Rescans are incremental, and games whose files disappear are hidden (not
   deleted), so play time and favorites survive a drive being unplugged.

Keyboard: arrows move, `Enter` opens the game sidebar (and `Enter` again plays), `F` the full
details, `Q`/`E` (or PgUp/PgDn) switch platform, `Home`/`End` jump by letter, `Esc` back (and
Settings when there's nothing to close), `F1` settings, `F11` fullscreen. Mouse: double-click a
cover for the sidebar, right-click for the full details.

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
- **Removed games**: when a game's file disappears, a scan deletes it from the library unless you
  played or favorited it; those stay hidden so their play time and favorite come back with the
  file (an unplugged drive, a moved folder). **Forget removed games…** deletes the hidden ones for
  good (play time, favorite, collection places, pictures); **Forget removed games automatically**
  makes scans do that right away. Pictures of games that left the library are always deleted.

## Controllers

| Action | PlayStation | Xbox | Keyboard |
|---|---|---|---|
| Move | D-pad / left stick (hold to repeat) | D-pad / left stick | Arrows |
| Select; on a game: the sidebar, then Play | Cross | A | Enter |
| Back / close | Circle | B | Esc |
| Search | Square | X | `/` |
| Full game details | Triangle | Y | F |
| Previous / next tab (platform, Settings section) | L1 / R1 | LB / RB | Q / E |
| Previous / next letter | L2 / R2 | LT / RT | Home / End |
| Settings | Options | Menu | F1 (or Esc with nothing to close) |

**Game sidebar:** choosing a game opens a panel on the right with its background and logo,
platform, year, developer, genres, series, play time, achievement progress (as last fetched; it
never goes online) and **Play**, **Favorite** and **More…** (the full details: covers, Fix match,
collections, launch command). Play is ready, so Cross/A twice starts the game; B/Circle closes
the panel. With the mouse, clicking another cover shows that game. On narrow windows the panel
opens over the grid. Favorites are set from the sidebar or the full details.
| Bigger / smaller cards | right stick up / down | right stick up / down | `+` / `-`, Ctrl + wheel |
| Platform sections on / off (All, Favorites, search) | R3 | RS (click) | V |
| TV Mode 3D shelf: turn / tilt / flip the focused case | right stick / right stick up, down / R3 | right stick / right stick up, down / RS | `[` `]` / `+` `-` / V |
| Genre filter (with IGDB) | Share | View | G |
| **Exit the running game** | **hold Share + Options 1 s** | **hold View + Menu 1 s** | |

**Sidebar (library):** the tabs (All, Favorites, Recent, Collections, then each platform) are in
a slim icon sidebar on the left. Press left on the first column of the grid to move into it: it
opens with names and game counts, up/down picks, A/Cross switches, B/Circle or right goes back
to the grid. Hovering opens it too. The button at its bottom cycles: opens on hover, always
open, always closed (icons only; the top bar names the icon you're on). LB/RB still switch tabs
from anywhere. Up from the first row of covers reaches the top bar (Search, Genre, TV Mode,
Settings; left/right to move, A to press, down to go back). In the Collections tab, Y/Triangle
edits the focused collection; inside one of your collections, up from the first row reaches
Rename… and Edit… first, then the top bar. The button hints follow the device you used last; a
mouse only counts when it really moves (not when the window moves under it).

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

### More than one emulator per platform

- **Per platform:** Settings → Platforms → the Emulator column opens a platform's emulators. Its
  built-in one is always there; add RetroArch (with a core, e.g. `swanstation` for PS1) or any
  emulator of your own, and pick which is the **default**. Each added emulator has its own core
  and launch template for that platform.
- **Your own emulators:** Settings → Emulators → **Add emulator…**: a name, its `.exe` and a
  launch template (`"{emulator}" "{rom}"` plus whatever options the emulator's documentation
  gives, e.g. a fullscreen flag). KB doesn't configure emulators, it only starts them.
- **Per game:** **Launch with** (details screen and sidebar) sets the emulator a game always uses,
  or "Default", which follows the platform's choice in Settings. It appears when a platform has more
  than one emulator. A Switch emulator joins the Switch platform as
  soon as its exe is set (Settings → Emulators, or Find emulators).
- If a game's emulator isn't set up (exe or RetroArch core missing, or removed), KB plays it with
  the platform's default and says so on the Now Playing screen.
- RetroArch cores that need the console's BIOS look for it in **RetroArch's own system folder**
  (its `system` folder, or `system_directory` in retroarch.cfg), not the standalone emulator's:
  `system\pcsx2\bios\` for the PS2 core, `scph….bin` in `system` for the PS1 cores (SwanStation,
  Beetle PSX), `bios_CD_U.bin` for Sega CD, `mpr-17933.bin` (US/EU) or `sega_101.bin` (Japan) for
  Saturn (Beetle Saturn; the names are case-sensitive). Settings → Platforms, the sidebar and the pickers warn
  when it looks missing (a warning only: KB still lets you try). If an emulator closes within a few
  seconds anyway, KB says so instead of quietly coming back, and doesn't count it as played.
- The pickers mark emulators **Achievements** / **No achievements** from RetroAchievements' list
  of supported emulators (RetroArch, DuckStation, PCSX2, Dolphin; not RPCS3, Azahar or the Switch emulators; your own
  emulators aren't marked). Unlocks belong to your RA account, so they count in any supported
  emulator you're signed in to; saves and save states usually don't carry over between
  emulators.
- Play time, last played, favorites and achievements stay with the game, whichever emulator ran it.

### Nintendo Switch

- Games are `.nsp` and `.xci` files. Titles come from the file names, and a `[0100…]` title ID in
  the name is kept as the game's ID. Update and DLC files are hidden (by their title ID, or an
  `[UPD]`/`[DLC]` tag); install them in the emulator itself. KB never decrypts files.
- **Eden** is the default. **Ryubing** (the maintained Ryujinx fork), **Ryujinx**, **Citron**,
  **Sudachi** and **Yuzu** (discontinued) join Switch as soon as their exe is set (Settings →
  Emulators, or Find emulators), then can be picked as the default or per game. Ryubing's exe is
  also called `Ryujinx.exe`: Find emulators tells them apart by folder (`Ryubing\…`).
  Launch commands: `-f -g "{rom}"` (Eden, Citron, Sudachi, Yuzu), `--fullscreen "{rom}"` (Ryubing, Ryujinx).
- Keys and firmware are your own and stay in each emulator's own folders; KB doesn't touch them.
- Get emulators only from their official sites: these projects move after takedowns, and fake
  installers circulate. KB never downloads emulators.
- Switch has no RetroAchievements.

### Xbox and Xbox 360

- **Xbox** plays in **xemu** (`-full-screen -dvd_path "{rom}"`). xemu needs your own BIOS, MCPX boot
  ROM and hard-drive image, set in xemu's own settings; KB never downloads them. xemu reads **XISO**
  images (`.xiso`, `.xiso.iso`); a full Redump `.iso` may need converting to XISO first (e.g. with
  extract-xiso).
- **Xbox 360** plays in **Xenia Canary** (`--fullscreen "{rom}"`), from its official GitHub releases.
  Disc images (`.iso`) are listed; extracted `.xex` folders, Games on Demand and Xbox Live Arcade
  content aren't scanned yet.
- In mixed folders, a full disc image is told apart by where its file system starts (original Xbox,
  or 360). An XISO looks the same for both consoles, so its folder name decides ("Xbox" or "Xbox 360").
- Neither has RetroAchievements.

## Covers and metadata

Everything works without accounts or API keys:

- **[LaunchBox Games Database](https://gamesdb.launchbox-app.com)** (keyless, used first): box
  art for your copy's region, clear logos and backgrounds (TV Mode), descriptions, release
  dates, developers, publishers, genres and player counts, for KB's platforms (~30,000 games for the first 12).
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
  **franchise** (e.g. "Jak and Daxter", "Final Fantasy"), plus its number in the series (used
  only to order games released the same year). When you have two or more games from a series, the
  game details show its name above the title and a series card under Play with the covers of your
  games from it in release order (select one to open it); with one game, nothing is shown.
  Each series is tinted with the main color of its first game's cover (computed from the
  thumbnail when the details open), shows that game's clear logo as its emblem when there is one,
  marks the games you've played and, once you've started it, glows on the next one you haven't.
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

## Collections

The **Collections** tab (next to Favorites and Recent) holds:
- **Your own collections.** "New collection" (the first tile) asks for a name. Add a game from its
  details with **Collections…** (tick any collections; a game can be in several), or start a new
  collection with it there. Open a collection to see its games; B goes back to the tiles.
- **Edit members from the collection page.** Choose **Edit… → Add games…** and tick games to add
  or remove them. In an open manual collection or series, press **Y** or right-click a game to
  remove it. Series edits are saved over Wikidata, so refreshing series data keeps your changes.
- **Series**, automatically: every series or franchise (from Wikidata, see Covers and metadata) you
  have two or more games from, in release order. Hide the ones you don't want; "Show hidden
  series" brings them back.

**Edit** (Y on a tile, or Edit… above an open collection): the logo (automatic, one of its games'
clear logos, a cover, or your own PNG/JPEG/WebP image, which is copied into `data\art\c\`), and
for your own collections the name, the order of the list, how its games are sorted (when added,
title, release date) and deleting it (its games stay in the library). A series' logo also shows on
its series card in the game details. Without a picked logo, a tile is a mosaic of its first four
covers with the name (series: their first game's logo). Collections are kept in the library
(`collections`, `collection_games`, `collection_looks`); TV Mode shows an open collection's games
but has no Collections overview yet.

## TV Mode

A big-screen view in the spirit of Steam's Big Picture, for the couch: switch with the left stick
button (L3), `F10` or the **TV Mode** button; Settings → General can start the app in it. It goes
fullscreen into Showcase: a logo list on the left, one large selected case in the middle,
and a dedicated information column with the game's title, console, play time and short synopsis.
Cached game logos are used whenever available, with the title as a fallback. More… opens the
full game details and complete description. The synopsis selects up to two useful sentences
locally and measures them in the TV font; its panel fits the text rather than reserving blank
lines. Desktop descriptions are unchanged. The full-width 16:9 preview below uses a cached
LaunchBox gameplay screenshot when available. Original gameplay proportions and HUDs are
preserved over a blurred fill; if no screenshot is available, existing artwork fills the frame
with a center crop. The background is unchanged. LaunchBox imports screenshot lists once on
upgrade (its existing metadata download), then caches one suitable screenshot per matched game;
TV navigation never fetches screenshots from the network.

The visible platform filter includes **All platforms** and each platform with its game count,
as well as Favorites, Recent and Collections. LB/RB cycle platforms. Up/Down browse games;
Up from the first game reaches the filter. Right moves to Play and More…; Left/B returns
to the game list. LT/RT jump by letter, X searches, Y favorites, and Start opens the TV menu.
The selected 3D case still supports turning, tilting and flipping; a flat-cover option is
available in Settings. TV Mode loads only when opened.

**Menu styles:** **KB** (KaneegaBell) keeps Showcase. **Wii** uses a full-screen 4×3
channel grid, page arrows, a curved clock dock, and a separate channel-opening screen with
Wii Menu, Start and Game information. D-pad moves across four columns; A/Enter opens a
channel. **PS3** uses XMB navigation: Left/Right slides between console tabs (plus All platforms), Up/Down moves
through the vertical game list, A/Enter starts the selected game, and Y/Triangle opens its
options. The active category and game thumbnails share one horizontal axis, so navigation
keeps them aligned. Platforms opens the shared filter with All platforms and every console.
Tabs use the console icons and keep their selected icon aligned with the game thumbnails.
The game thumbnails use an uncropped logo over background artwork, with a title fallback.
Both console menus show the active platform without a pop-up when it changes. In Wii, Down
from the bottom channel row reaches the dock; Left/Right then choose its buttons, Up returns
to the channels. The right stick also moves a local hand pointer; A/Cross or right-stick click
activates the visible button under its fingertip. No OS mouse input is generated.

Wii channel/page transitions, PS3 category/item slides and options-panel transitions use
transform/opacity. The PS3 waves move in Full animation mode and stop in Reduced, Off,
OS reduced-motion mode and while a game runs. The earlier performance figures below do
not verify these new console screens. Console menu effects are bundled and selected automatically for Wii/PS3; desktop and KB
keep the chosen sound profile. The console feedback update enables TV sounds and Full
animation once to apply the owner's request; later settings changes remain respected.
Explicit Full animation opts into motion even if Windows requests reduced motion.
The controller thread sends pointer velocity only while a stick moves with KB focused;
no pointer messages are sent while a game runs.

Switch instantly with **Menu style** in the corner, **G / View / Share**, or
**Start → Menu style…**. The choice is saved, and switching styles preserves the selected
game and filter. No console firmware or extracted system artwork is used.

Earlier shelf layout (before Showcase), measured on the 5,000-game test library: holding right stepped through the shelf with every frame
under 14 ms; idle (background drifting) 0.2% of one core.

Settings → General → TV Mode:
- **Display**: open TV Mode on a chosen display (listed by the monitor's own name, e.g. "SONY TV").
  KB moves there and goes fullscreen; leaving TV Mode puts the window back. If that display is
  off or unplugged, TV Mode opens where KB is and says so.
- **Text size**: Small / Medium / Large / Extra large.
- **Overscan margin**: Off / 2.5 / 5 / 7.5%, for TVs that cut off the picture's edges.

## Animations, intros and sounds

Settings → General:
- **Animations**: Full / Reduced / Off, for the desktop and TV Mode separately (both start at Full;
  Windows' "Show animations" setting isn't used, KB only has its own). Full adds the sidebar
  sliding in, dialogs growing in, toasts rising and covers fading in on the desktop; Reduced only
  fades; Off keeps everything still. Only `transform` and `opacity` animate, nothing loops on the
  desktop, and everything stops while a game runs.
- **Launch animation** (desktop and TV Mode, each on/off): when you press Play, the game's case
  opens and the disc spins up, or the cartridge goes into its console, then the emulator starts.
  Each platform has its own case (CD jewel case, DVD/Blu-ray keep cases, the GameCube mini disc,
  the PSP's UMD) or cartridge and console (SNES, N64, GBA, DS, 3DS, Switch). The disc and the back
  cover use LaunchBox's pictures when it has them, else the cover. Back cancels it.
- **TV case**: a large 3D case (turn it with the right stick or by dragging, tilt with right stick
  up/down, flip to the back with R3/V) or flat covers.
- **Start-up intro** (desktop and TV Mode): Random, one fixed, or Off. Ten intros in the spirit
  of famous ones: Konami (Metal Gear), Rockstar (Red Dead Redemption 2), Game Boy Advance,
  GameCube, PlayStation, PlayStation 2, PlayStation 3, PSP, Xbox 360 and Switch. The owner-provided recordings are bundled as the default audio; PSP plays both parts in order. Any button skips.
  **Your own sound for an intro:** put a file named after it (`konami`, `rockstar`, `gba`,
  `gamecube`, `ps1`, `ps2`, `ps3`, `psp`, `xbox360` or `switch`, as `.ogg`, `.mp3`, `.wav`, `.m4a`
  or `.flac`) in `data\intros` next to `KaneegaBell.exe`; the intro plays it from the start and
  stays open until the recording finishes. Files in data\intros override the bundled audio and are never shipped.
- **Sounds**: three profiles (Tactile, the default: real buttons and latches; Console: soft system
  tones; Retro console: warm filtered beeps and chunky switches) for moving, choosing, going back
  and starting a disc or cartridge game. On/off and volume for the desktop and TV Mode separately;
  silent while a game runs.

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
`.iso`), the first sector for Sega CD and Saturn, apploader + main executable for GameCube and Wii
(`.iso`, and Wii `.wbfs` read through its block table), and `PARAM.SFO` + `EBOOT.BIN` for PS3
folders. Compressed images hash exactly like the dumps they were made from (see
[Compressing your games](#compressing-your-games)). 3DS shows no achievements (hashing needs
console keys).

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

## Compressing your games

Compressed disc images take less space and play exactly like the originals: the formats are
lossless, so emulators see the same data, and per-game settings, patches and cheats (which
emulators tie to the game's ID and checksum, read from inside the disc) keep working. KB reads
them for RetroAchievements too, and they get the same match as the original dump.

| Platform | Format | Make it with | Plays in |
|---|---|---|---|
| PS1, Sega CD, Saturn | CHD | `chdman createcd -i "Game.cue" -o "Game.chd"` | DuckStation, RetroArch |
| PS2 (DVD games, `.iso`) | CHD | `chdman createdvd -i "Game.iso" -o "Game.chd"` | PCSX2 |
| PS2 (CD games, `.cue`) | CHD | `chdman createcd -i "Game.cue" -o "Game.chd"` | PCSX2 |
| PSP | CSO (or CHD) | `maxcso "Game.iso"` (or `chdman createdvd`) | PPSSPP |
| GameCube, Wii | RVZ | Dolphin: right-click the game → Convert File… → RVZ (defaults) | Dolphin |

chdman comes with [MAME](https://www.mamedev.org/release.html), maxcso from its
[GitHub releases](https://github.com/unknownbrackets/maxcso/releases); KB doesn't download or
run either. Good to know:

- **KB keeps your history.** When a game's file is replaced by a compressed one with the same
  name **in the same folder** (e.g. `Game (USA).cue` → `Game (USA).chd`), the next scan moves its
  play time, favorite, collections and picked art over to the new file. A copy that turns up in
  another folder while the original is missing (say, its drive is unplugged) stays a separate
  entry, so the history waits for the original; and if two games could match, KB doesn't guess.
- Discs whose cue sheet stores a pregap ahead of track 1 (`INDEX 00` / `INDEX 01 00:02:00`) are
  read from where the track starts, like emulators do, so the `.cue` and its `.chd` hash the same.
- Patch a disc image (translations, `.xdelta` romhacks) **before** compressing it; patching tools
  need the uncompressed file. Emulator-side patches and cheats don't care.
- Keep a PS1 game's `.sbi` file (LibCrypt protection, some PAL games) next to the `.chd`, named
  the same.
- Avoid NKit for GameCube/Wii: it throws data away, so RetroAchievements no longer recognizes
  the game. RVZ with Dolphin's defaults is lossless.
- Wii `.wbfs` files that RetroAchievements doesn't recognize won't start matching after
  conversion: the data its hash reads isn't in them. A fresh dump would match.
- Keep the originals until you've checked a few compressed games play and show their
  achievements in KB.

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

## Icons and credits
- Platform pictures (`src/assets/platforms/*.png`, 160 px white-on-transparent masks tinted by maker):
  supplied by the owner and cleaned up (smoothed, recolored; the PS3 "3", the Xbox X, the N64's
  facet lines and the PSP were redrawn from them). Saturn: the logo from
  [Simple Icons](https://simpleicons.org) (CC0).
- The app icon (`src-tauri/icons`) and the emulator badges are drawn for KB.
- Game cases, cartridges and consoles in the launch animation and the 3D shelf are drawn for KB
  (no logos).
- Console menu sounds (`src/assets/sounds/menu_*`): standalone audio from [danintosh/Wii-Menu-HTML](https://github.com/danintosh/Wii-Menu-HTML) (hover, channel hover, page scroll, Wii menu background music) and [JMRDev0/XMB-PS3-Icons-Sounds-Pack-](https://github.com/JMRDev0/XMB-PS3-Icons-Sounds-Pack-) (up, OK, cancel). Nintendo/Sony assets remain their respective owners’ property; these are not CC0. No firmware or code from these projects is bundled.
- Startup audio (`src/assets/sounds/intro_*.ogg`): converted from the owner-provided recordings in `audio files` using `tools/sounds/import-intros.ps1` (PSP joins part 1 and part 2).
- Menu and launch sounds (`src/assets/sounds/prof_*.ogg`, made with `tools/sounds`): CC0 sources include [VSCO 2 Community Edition](https://github.com/sgossner/VSCO-2-CE) by Versilian Studios,
  [The Free Firearm Sound Library](https://opengameart.org/content/the-free-firearm-sound-library)
  and [Kenney](https://kenney.nl)'s Interface Sounds and UI Audio.

## Styles

Settings → General → **Style** switches KB's look at once: colors, corner shape, how the
focused game is marked, and the bull's colors in the window and taskbar (the exe and
installer keep Ember's). Ember is the default; Midnight is the look of 0.3.10 and earlier.
Styles are data only, checked for readable text and a visible focus ring before use. The
format is documented in [docs/themes.md](docs/themes.md) (with a JSON Schema). After changing a
style's icon colors in `tools/theme-icons.mjs`, run `node tools/theme-icons.mjs`.

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

0.2.6 memory pass. Before it, 0.2.6 measured 445–457 MB after a game exits (+7–10% on 415) and ~1,100 MB after
scrolling all 5,000 covers. A per-process breakdown showed WebView2's GPU process holding ~850 MB of decoded covers
after scrolling. KB now starts WebView2 with `--force-gpu-mem-discardable-limit-mb=16` and
`--force-gpu-mem-available-mb=64` (and the network service inside the browser process, one process less). Three
runs after: cold start 441–521 ms, 258–263 MB private at start, scrolling 240 fps with p99 4.3 ms (one 125 ms hitch in
one fast flick, not repeated), 400–414 MB after scrolling (was ~1,100), 259–262 MB while a game runs (was ~370),
346–352 MB after a game exits (was ~450). TV Mode holding right for 12 s: every frame ~4.2 ms, max 4.4 ms.
The perf tool now also reports what **Task Manager** shows (private working set, which leaves out memory shared
between the processes): ~125 MB at start, ~225 MB right after scrolling all 5,000 covers, ~115 MB while a game runs,
~150 MB after it. Tried and rejected: GPU inside the browser process, CPU rasterization, `--disable-direct-composition`
and `--use-angle=gl`. Each lowered "private" memory only by moving the covers into shared memory (renderer working set
~650–690 MB), and the in-process GPU also dropped frames. The baseline was saved again after this pass.

0.2.7: the `--force-gpu-mem-available-mb=64` cap is gone. On a 2000×1125 window at 150% scaling with
two panels open (details + cover picker), it left screen tiles undrawn (holes in the grid, a half-drawn picker);
reproduced over DevTools and gone without it. Memory without it, same library: ~126 MB in Task Manager at start,
~230 MB right after scrolling all 5,000 covers (224 with it), ~106 MB while a game runs, ~140 MB after. The cover
cache limit (16 MB) stays: that's what keeps scrolling memory down.

0.2.7 release gate (game details sidebar, idle PC), 3 runs: cold start 427 / 431 / 448 ms, 116–118 MB in Task Manager at
start, scrolling 240 fps with p99 4.3 ms and no frame over 8.4 ms, 209–214 MB in Task Manager after scrolling all 5,000
covers, 108–110 MB while a game runs, 138–141 MB after. Runs 2 and 3 passed; run 1 tripped on "private" memory while
a game ran (309 MB vs 253 in the others) while its Task Manager figure was the lowest of the three (108 MB), so a
counting blip. TV Mode holding right: every frame ~4.2 ms, max 4.4 ms. Earlier runs the same day failed while a game
used ~4 CPU cores in the background (cold start 523–611 ms, single 21–25 ms frames); with the PC idle they passed.
0.2.8 (built after 0.2.7 was tagged): after the cover fixes and the 32 MB web cache cap (`--disk-cache-size`,
confirmed on the WebView2 process): cold start
429 / 432 ms, 240 fps, 107–142 MB in Task Manager; run 2 passed, run 1 tripped only on the first launch of the new exe
(one 1.7 s start and one high memory reading; its other four starts ~430 ms).
0.2.9 (storage clean-up, no component downloads): cold start 459 / 454 ms, 240 fps (p99 4.3 ms), 204–212 MB in Task
Manager after scrolling, 105–112 MB while a game runs. Run 2 passed; run 1 again tripped on private memory while a
game ran (305 MB vs 251) with Task Manager at its lowest (105 MB). Since 0.2.9 the gate checks memory with the Task
Manager figures; private memory only warns. On a copy of the installed library, the first start freed 22 MB
(96 → 74 MB: the WebView2 add-ons and one removed game's pictures).
0.3.0 (compressed disc readers; the exe grew 15.8 → 16.5 MB): cold start 481 ms median, 240 fps (p99 4.3 ms, max
4.4 ms), 201 MB in Task Manager after scrolling, 104–111 MB while a game runs; passed on the first run. Hashing a
compressed image takes 3–120 ms (background only). Verified on the owner's games: CHD (PS1, Sega CD, PS2 DVD, PSP
as CD and DVD, zstd and default codecs), CSO v1/v2 and ZSO (PS2, PSP) and RVZ (GameCube) all hash exactly like
the original dumps. Wii RVZ uses the same reader but couldn't be tested: the owner's Wii games are `.wbfs`, which
DolphinTool won't convert. After the review fixes (same-folder history, cue `INDEX 01`): cold start 461 ms, 240 fps,
205 MB in Task Manager after scrolling, 104–111 MB while a game runs; passed.
0.3.1 (several emulators per platform, core/BIOS warnings): cold start 456 ms, 240 fps (max frame 5.7 ms), 205 MB in
Task Manager after scrolling, 102–109 MB while a game runs; passed (with a game launcher downloading in the
background). A first run failed while a game used ~3.4 cores (cold start 561 ms, two 20–33 ms frames).
0.3.2 (Sega Saturn): cold start 437 ms, 240 fps (max frame 4.3 ms), 203 MB in Task Manager after scrolling,
104–110 MB while a game runs; passed on the first run.

0.3.5 (Settings touch-up, Settings loaded only when opened): cold start 460 ms, 240 fps (max frame 10.3 ms on the
first pass), 116 MB in Task Manager at start (−8%), 202 MB after scrolling (−10%), 104 MB while a game runs, 130 MB
after it (−16%); passed on the first run.
0.3.12 (animations, intros, sounds, 3D cases; intros, sounds and launch animation switched off in the test library,
animations at Full): cold start 428 ms median, idle 0.26% of one core, 105 MB in Task Manager at start, 215 MB after
scrolling, 106 MB while a game runs, 136 MB after it; scrolling 240 fps with p99 4.3 ms and no frame over 4.4 ms.
Run 2 passed; run 1 tripped on one 20.8 ms frame in the second scroll pass (p99 still 4.3 ms, one start of 930 ms
as the new exe launched for the first time), not repeated. Private memory is higher than the old baseline (+22–35%)
but only warns; the Task Manager figures, which the gate uses, are within 4%.
Added after the gate (only the back/spine/disc picker's Search and a LaunchBox pass that can keep a few extra entries; nothing
at startup, in the grid or while idle), so no second gate run.

0.3.13 release candidate (2026-10-22): two full runs failed the startup regression gate:
652 ms then 580 ms median versus the saved 426 ms baseline (+36% on the repeat), still under
1.5 seconds. The owner accepted this startup deviation for publication after a time-limited investigation. Run 2: 240 fps, p99 4.3 ms, maximum
4.4 ms; idle 0.77% of one core; Task Manager memory 107 MB at start, 213 MB after scrolling,
109 MB during a game and 139 MB after it. Other gated metrics passed; private memory only warned.
Native console regression checks passed 17/17 across 720p, 1080p, 1440p and 4K, including
PS3 footer bounds after console-tab focus changes, icon alignment/size, removed pop-up and
Wii dock action navigation. These are layout checks, not a console frame-rate measurement.
See `docs/release-0.3.13.md` for evidence, limitations and remaining release steps.

Final 0.3.13 publication check (2026-10-22), rerun after the owner stopped a CPU-intensive game:
startup 629 ms median versus the 426 ms baseline (+203 ms, +48%); the startup gate fails, while
all other gates pass. All three scroll passes 240 fps, p99 4.3 ms, maximum 4.4 ms, no frame over
20 ms. Idle 0.77% of one core; Task Manager RAM 108 MB at start, 217 MB after scrolling,
110 MB during the game, 139 MB after it. Private memory warnings: 250/448/340/367 MB.
The preceding 754 ms run happened while the owner was in a CPU-intensive game; an intervening
623 ms run lost its local DevTools connection before completing and is not a full gate result.
The owner authorized publication if a five-minute investigation found no quick fix. No app-code
fix was identified: current/previous screenshot queries both take about 11-19 ms for 5,000 rows;
one cold profile spent ~296 ms before page navigation, then loaded JS/CSS within 6 ms, completed
DOM loading at 59 ms and painted at 260 ms. A warm library IPC took 25 ms. Reloads of the same
UI painted in 28-32 ms. Cold WebView/first-paint overhead warrants further investigation;
no controlled old-binary comparison was completed in the time budget. Baseline left unchanged.


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

### 0.3.14
- New: in TV Mode, Esc opens the menu, which now has a Turn off PC button (asks you to press it twice).
- New: the desktop sidebar groups platforms into compact Nintendo, PlayStation, Sega and Microsoft folding rows, remembering which are open; A opens or closes a group.
- Fixed: start-up intro effects follow the complete supplied audio recordings, including PSP's second part, PS2's final chime and the Switch click.
- Fixed: skipping or replacing an intro cancels its pending playback and timers.
- New: PS3 TV Mode plays the existing PS3 sound pack's background track, following TV volume and pausing for intros, hidden windows and gameplay.

### 0.3.13
- New: switch between KB, Wii and PS3 TV menus without restarting; your choice is remembered.
- Changed: KB TV Mode has a logo list, a large 3D case, short game summaries, and an All platforms filter.
- New: Wii TV Mode has a channel grid, channel-opening screen, page transitions and a clock dock.
- New: Wii supports a right-stick hand pointer and D-pad access to the bottom controls.
- New: PS3 TV Mode has a console crossbar, a vertical game list, animated waves and sparkles, and an options panel.
- Improved: PS3 console icons are larger and game tiles show logos over backgrounds instead of cropped boxes.
- New: Wii and PS3 menus use their own sound effects with matched playback levels; Wii also plays its menu music.
- New: TV previews prefer cached gameplay screenshots in a 16:9 frame, preserving older games' proportions.
- Improved: TV descriptions fit their text; More… opens the full description and game information.
- Fixed: moving PS3 console tabs no longer scroll the screen and cut off the bottom controls.
- Changed: the current platform stays visible without a pop-up when switching consoles.
- Changed: start-up intros use the supplied recordings as their default sounds.

### 0.3.12
- New: animations. Pick Full, Reduced or Off for the desktop and for TV Mode in Settings → General → Animations.
- New: start-up intros, ten of them (Konami, Rockstar, Game Boy Advance, GameCube, PlayStation, PS2, PS3, PSP, Xbox 360 and Switch), random or fixed. Put your own sound for an intro in KB's `data\intros` folder.
- New: starting a game plays a launch animation: the case opens and the disc spins, or the game's own cartridge goes into its console. Turn it off for the desktop or TV Mode in Settings.
- New: TV Mode's shelf shows 3D game cases you can turn, tilt and flip to see the back.
- New: menu sounds in three styles (Tactile, Console, Retro console), with their own switch and volume for each mode.
- New: back covers, spines, discs and cartridges from the LaunchBox Games Database and GameTDB. Pick or search for your own in a game's details. The first start downloads LaunchBox's database once more.
- Changed: the side panels no longer scroll and show shorter descriptions, and the series name above a title opens its collection.

### 0.3.11
- New: styles. Pick one in Settings → General → Style: Ember (the new default), Midnight (the old look), Daylight, High contrast, Moss or Storm.
- New: each style marks the focused game its own way (brackets, glow, a ring, a lift, a bar or a spotlight).
- New: the bull icon in the window and taskbar takes the style's colors.
- Fixed: a game's window now comes to the front on its own, so you no longer have to click it in the taskbar.

### 0.3.10
- New: search SteamGridDB for a collection's logo (needs your SteamGridDB key).
- New: TV Mode has a Collections tab; open a collection to browse its games.
- New: collections show their total play time, how many games you've played and when.
- New: pick a background for a collection in TV Mode from SteamGridDB (needs your key).
- New: sort the Collections tab (A–Z, release date, play time, size, genre, or your own order).
- New: drag your collections and series into your own order (drop on a tile's left or right half; the others shift along), or pick one up with the controller and move it with the D-pad.

### 0.3.9
- New: filter your games by platform, genre, developer, publisher, release year, play time, whether you've played them, and favorites. Save a filter as a smart collection that updates as your library changes.
- New: edit a smart collection's rules from its collection tile; its games always follow the saved filter.
- New: add or remove games from a collection page. Your edits to a series stay saved when its Wikidata information refreshes.

### 0.3.8
- New: List view. Press V (or R3, or Settings → General) to see your games as rows, with a details panel on the right: the background, the whole cover, play buttons, your latest achievements, collections, links and the description, each in its own box.
- Changed: V now switches between the cover grid and the list. The old "One list" option is gone; the grid keeps its platform sections.
- New: favorites show a gold star left of the play time in the list, and the selected row lights up.
- Improved: the panel shows the game you're on as you move down the list. A, Enter or → moves into it; B or Esc comes back.

### 0.3.7
- New: every game has a small square icon, shown in the game sidebar, on Recent and inside collections. KB crops it from the cover on its own (no key needed) and keeps it up to date when the cover changes.
- New: Icon… on the details screen picks your own: SteamGridDB icons (with a key), an image file, or "Use the cover" to go back to the automatic one.
- Fixed: SteamGridDB icons that come as .ico files can now be picked.
- Fixed: small icons (32 px) are scaled up to fill the icon square instead of sitting tiny in it.
- Improved: games without an icon yet show a coloured letter in the sidebar until their cover arrives.

### 0.3.6
- New: every platform has its own picture in the platform list (SNES, N64, GameCube, Wii, Switch, PlayStation, Xbox and the rest), emulators get a small badge in Settings, and the app has a new bull icon (exe, window, title bar).
- Fixed: wide backgrounds (SteamGridDB) in the game sidebar show whole, with blurred bands above and below, instead of being cut off at the sides; 16:9 ones still fill the box.
- New: Change background shows each background's shape (16:9 or wide) and previews it the way the sidebar will show it.

### 0.3.5 (2026-10-18)
- Changed: Settings fits on one screen: two columns of one-line settings, with every button and switch lined up.
- Changed: lists and forms (ROM folders, emulator paths, keys, exit combo, TV Mode display, storage) open in a small panel instead of stretching the page.
- New: rest on a setting (mouse or controller) and a short explanation pops up beside it, so the long texts are gone from the page.
- Changed: Check now also shows what's new when you already have the latest version; the separate What's new button is gone.
- New: Storage has a "Clear web cache" button that empties only KB's page cache (your covers and game info stay).
- New: search in Settings finds any setting in any section (type in the box, or press Square / X); choosing a result jumps to it.
- Improved: KB starts a little lighter, because Settings now loads only when you open it.
- Fixed: a Switch game whose file name has both its own title ID and a high version number (a cartridge revision or a merged update) is no longer hidden as an update.

### 0.3.4 (2026-10-18)
- New: Xbox, played with xemu (XISO disc images).
- New: Xbox 360, played with Xenia Canary (disc images).
- New: Xbox and Xbox 360 discs in mixed folders are recognized by their disc layout.
- Changed: one "Launch with" choice per game, on the details screen and in the sidebar ("Play with…" did the same thing and is gone).
- Changed: background progress (covers, game info, series…) shows in the top bar, where there's room; it no longer makes the bottom bar jump.
- Fixed: after a game, KB sometimes came back windowed instead of fullscreen.
- Fixed: Switch update files from scene releases (e.g. "hr-bgw5av131072.nsp") no longer show up as games.

### 0.3.3 (2026-10-18)
- New: Nintendo Switch, played with Eden (.nsp and .xci games).
- New: Ryubing, Ryujinx, Citron, Sudachi and Yuzu can be used as Switch emulators, picked as the default or per game.
- New: Switch update and DLC files are hidden, so only the games themselves are listed.
- New: a Switch game's title ID in its file name is kept as the game's ID.
- Improved: a version number in a file name (like "v1.0.0") no longer shows in the game's title.
- Fixed: games on a newly added platform get covers and info from the LaunchBox database right away, not a week later.
- Changed: "Play with…" now remembers the emulator you pick for that game, so it starts with it next time too.
- New: a Switch emulator is offered for Switch games as soon as its .exe is set up, no extra step in Settings → Platforms.
- Improved: new covers appear before series are looked up on Wikidata, instead of waiting for it.
- Improved: the footer says what's happening in the background (e.g. "Series (Wikidata): Switch, downloading its game list") and counts the seconds when a step takes a while.

### 0.3.2 (2026-10-18)
- New: Sega Saturn, played with RetroArch's Beetle Saturn core (Kronos, YabaSanshiro or Yabause can be picked instead in Settings → Platforms).
- New: Saturn games show their RetroAchievements.
- New: Saturn discs in mixed folders are recognized by their disc header, with the game's product number.
- New: KB warns when the Saturn BIOS (mpr-17933.bin or sega_101.bin) is missing from RetroArch's system folder.
- Fixed: a cue sheet whose disc images aren't there (e.g. a translation patch not applied yet) is no longer listed as a game; the scan report says why.
- Fixed: changing a game's background or logo a second time sometimes kept showing the old picture until a restart.
- Fixed: a damaged Wii or CSO disc image can no longer make KB close on start.
- Fixed: a game moved to another platform by a scan no longer keeps the old platform's "Launch with" emulator.

### 0.3.1 (2026-10-16)
- New: a platform can have more than one emulator: add RetroArch (with a core) or any emulator of your own, and pick the default, in Settings → Platforms → Emulator column.
- New: Settings → Emulators → "Add emulator…" adds any emulator by its .exe and launch template.
- New: "Launch with" on a game's details screen sets the emulator that game always uses.
- New: "Play with…" (details screen and sidebar) starts a game with another emulator just this once.
- New: emulator pickers say which emulators earn RetroAchievements.
- New: if a game's emulator isn't set up, KB plays it with the platform's default and says so on the Now Playing screen.
- New: Settings → Platforms says when a RetroArch core isn't installed or its BIOS seems to be missing (e.g. RetroArch's PS2 core needs a PS2 BIOS in RetroArch's own system folder); the game's sidebar and pickers warn too.
- New: when an emulator closes within a few seconds without running the game, KB says so (with the likely reason) and doesn't count it as played.

### 0.3.0 (2026-10-16)
- New: compressed games show their achievements: CHD (PS1, PS2, Sega CD, PSP), CSO (PSP, PS2), ZSO (PS2) and RVZ (GameCube, Wii; also WIA, GCZ, CISO) get the same RetroAchievements match as the original dump.
- New: when you replace a game's file with a compressed one of the same name in the same folder, KB keeps its play time, favorite, collections and picked art.
- Fixed: discs whose cue sheet stores a pregap before track 1 are read from where the track starts, so they're recognized for achievements and sorted to the right platform.
- Improved: compressed PS1, PS2, PSP and Sega CD discs in mixed folders are sorted to the right platform by looking inside them, not just by folder name.
- New: the README explains which format to use per platform and how to make it.

### 0.2.9 (2026-10-16)
- Improved: when a game leaves your library, its achievement badges and cached achievement lists go too (badges another game still uses stay).
- Improved: pictures and badges that removed games left behind in earlier versions are cleaned up once on the first start.
- Improved: KB no longer lets WebView2 download add-ons it never uses (ad-filter lists, a speech model, hyphenation data); the ones already downloaded (~20 MB) are removed on the first start.
- New: Settings → General → Storage and reset shows the library copy a reset keeps, with a "Delete reset backup" button.
- Changed: "Clear downloaded data" also empties the web cache (on the next start).

### 0.2.8 (2026-10-15)
- Fixed: Refresh no longer replaces a cover you picked yourself.
- Fixed: a disc scan that LaunchBox lists as a box front is no longer chosen as the cover when a real box exists.
- Improved: SteamGridDB backgrounds and logos are also searched by LaunchBox's names, so games named differently on disk are found (e.g. ".hack//Infection").
- Improved: KB's web cache stays under 32 MB; the cover pickers' previews had grown it past 300 MB (cleared once on the first start).

### 0.2.7 (2026-10-15)
- New: choosing a game (Cross/A, Enter or double-click) opens a sidebar on the right with its background, logo, facts, play time and achievement progress, plus Play, Favorite and More….
- Changed: Triangle/Y, F or right-click opens the full game details (the sidebar's More… too).
- Changed: favorites are set from the sidebar or the full details; Triangle/Y and F no longer favorite.
- New: Esc opens Settings when there's nothing left to close.
- Fixed: on big or high-DPI screens, parts of the screen could stay undrawn with two windows open (a half-drawn background or cover picker, holes in the grid).
- Improved: Settings → Game info names the games LaunchBox and Wikidata couldn't find (and those with several possible matches), instead of only counting them.
- Improved: Settings → Library → Covers lists the games that still have no cover.

### 0.2.6 (2026-10-15)
- New: Settings → General → Updates → What's new lists every update and what it changed.
- New: after an update, KB shows what's new once.
- New: when you have two or more games from a series, its name shows above the title (e.g. "Jak and Daxter").
- New: a series card under Play shows the covers of your games from that series in release order; select one to open it.
- New: each series gets its own color, taken from its first game's cover.
- New: the series card shows the series logo (its first game's clear logo) when there is one.
- New: covers in the series card show which games you've played, and glow on the next one you haven't played yet.
- New: a Collections tab next to Favorites: make your own collections and put any game in as many as you like.
- New: game details have a Collections… button to add the game to collections, or start a new one with it.
- New: series you have two or more games from show up in Collections automatically; hide any you don't want.
- New: pick the logo of any collection or series: one of its games' logos, a cover, or your own image.
- New: your collections can be renamed, reordered, sorted by when added, title or release date, and deleted.
- Changed: the platform tabs moved to a slim sidebar on the left: icons, opening to names and game counts on hover or with the controller (left from the first column).
- New: the sidebar's button at the bottom switches between opening on hover, always open and always closed (with a controller: down past the last system, then A).
- New: rename your own collection right from its page (Rename… at the top; with a controller, press up from the first row).
- New: collection tiles show a pencil on hover to edit them.
- New: TV Mode's hint bar shows how to go back to desktop mode (and it's clickable).
- New: the top bar works with a controller: press up from the first row of covers to reach Search, Genre, TV Mode and Settings.
- Fixed: switching between desktop and TV Mode no longer changes the button hints from your controller to keyboard keys.
- New: Settings → General → Storage and reset → Forget removed games: delete games whose files are gone, with their play time and pictures.
- New: an option to forget removed games automatically at each scan.
- Fixed: pictures of games that left the library are now deleted instead of left behind.
- Fixed: at some card sizes the grid flickered between two sizes (the scrollbar kept appearing and disappearing).
- New: the card-size slider shows a zoom percentage (100% is the default).
- Fixed: with the controller in the sidebar or top bar, the card you came from no longer looks selected too.
- Fixed: the sidebar's focus outline was cut off at the edges.
- Fixed: the logo above an open series or collection was tiny; it's now shown full size next to its name.
- Improved: KB uses much less memory: about 125 MB in Task Manager at rest, and no more growing past 1 GB after scrolling a big library.
- Changed: a slimmer top bar shows where you are, a search box, and icons for Genre, TV Mode and Settings.
- Changed: series are looked up once more after this update.

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
