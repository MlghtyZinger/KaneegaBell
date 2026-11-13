# Launcher expansion validation - 2026-11-13

Scope: Steam branding plus installed-only Epic Games, GOG Galaxy, Battle.net, Xbox PC, Amazon Games, Ubisoft Connect and EA compatibility integrations. Work stays in the isolated steam-integration checkout. No installed-copy replacement, account API use, game download or real game activation during validation.

## Automated checks

- Native regression suite: 306 passed, 18 intentionally ignored. Includes source retention on incomplete/cancelled scans, reconnect/history, moved installs, validated process paths, Steam process observation, Epic URI components, GOG DLC exclusion and client arguments, EA installed-state/XML parsing, Battle.net product variants, metadata-platform mapping and database migrations.
- JavaScript suite: 46 passed. TypeScript also passes in the release build.
- Hidden Edge preview with synthetic IPC and external network requests blocked: all eight logos render; connect/disconnect works; Steam, GOG and Xbox waiting/cancellation routes work; no page errors. Panel checked at 1280x720, 1920x1080 and 1024x768; screenshots visually reviewed at 1280 and 1024. Development preview only, not a native WebView2 performance measurement.
- An initial test fixture used a one-byte protobuf length and failed on long temporary paths; corrected to encode normal variable-length lengths. Two existing process-tree tests cannot operate in the restricted execution environment; both pass with normal Windows process access.

## Authorized local inventory smoke checks

Every reader imported into a temporary test database, never the owner's library. No launch action executed.

| Source | Validated installed games | Warnings |
| --- | ---: | ---: |
| Epic Games | 0 (engine installation excluded) | 0 |
| GOG Galaxy | 1 (DLC excluded) | 0 |
| Battle.net | 3 WoW variants, each with its exact executable | 0 |
| Xbox PC | 2 | 0 |
| Amazon Games | 0 installed rows | 0 |
| Ubisoft Connect | 0 installed records | 0 |
| EA | 0 confirmed installations | 1 |

EA has leftover install-data folders without validated installed-game records. They are not imported based on folder names; the scan keeps previous entries and reports the limitation. EA supports registered installer XML and compatible Origin installation manifests. Other local formats remain unsupported. Ubisoft and any unknown executable layout require an explicit game-executable choice before launch. Protected Xbox processes can also require that choice and may remain inaccessible without guessing.

The local launch actions for the seven added clients still require owner trials. Fixture and inventory checks establish parsing, identity, paths and UI behavior, not successful sign-in/licensing/game startup. Existing Steam launch success was confirmed by the owner before this expansion.

## Build and remaining gates

Final Tauri release build (`--no-bundle`) passed, including TypeScript and Vite, in 3m05s. The final executable contains the cancellation-before-client-open safeguard. Built development executable: C:/Emulator files/src-tauri/target/release/KaneegaBell.exe. The installed copy at C:/Games/KaneegaBell is untouched; adjacent development data is preserved. No version bump or installer publication.

No benchmark run and no performance baseline reset. The prior unresolved performance gate remains unresolved.
