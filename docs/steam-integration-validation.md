# Steam implementation validation - 2026-11-13

Status: Windows compilation and automated validation passed. Native UI, manual real-game behavior and the performance gate remain unverified; this is not release readiness.

The owner explicitly authorized validation while still playing ("still playing run the validation"), superseding the earlier build deferral. Compilation and automated tests ran in the isolated checkout with two build jobs. KB and real games were not launched. Native UI/game tests and performance measurements remain deferred until play ends. Steam read permission is already granted.

Checkout: C:/Emulator files/tmp/steam-integration, branch codex/steam-integration, base 8703d75. Original sound-preview changes and unrelated drafts remain in the original checkout. The existing development executable, its data and C:/Games/KaneegaBell are unchanged. The AI Team remains removed.

## Completed checks

- TypeScript and Vite production build: passed, exit 0.
- Existing JavaScript suite: 43 passed, 0 failed, exit 0.
- Windows Rust library compilation and full default test suite: 295 passed, 0 failed, 17 ignored, exit 0. The new parser, import/reconciliation, manifest preflight, exact-path/candidate, immediate cancellation/timeout and controlled new-process/exit checks passed. Log: test-output/native-validation.log.
- Separately invoked opt-in local Steam metadata import: 1 passed, exit 0. Used only an in-memory test database and temporary KB artwork directory; no Steam files or real KB library were changed. Imported 11 classified games/demos, 10 with validated automatic tracking candidates, 1 requiring an explicit executable choice, 4 cached portrait covers, no unclassified apps or warnings. Repeated import added no duplicates and marked no games missing. Every imported manifest was rechecked and every cached executable candidate passed canonical path/helper validation. Log: test-output/steam-import-validation.log.
- A standalone read-only check with the production KeyValues parser found 4 library paths, 17 installed manifests and all 17 corresponding entries in the current version-41 cache (4,807,619 bytes). Classified 11 game/demo apps and 6 other app types. No credentials, saves, screenshots, Workshop or account inventory were read.
- In-memory SQLite migration check: all 29 migrations applied; verified Steam App ID bounds/uniqueness, parent FK rejection and cascading deletion. No actual library DB was used.
- Whitespace check passed; tracked files retain their original newline conventions.

The first native build failed inside SDL's existing Windows build tooling under restricted access. Retrying the authorized isolated build with normal build-tool access succeeded. The first complete suite caught Steam being enabled before connecting, which unnecessarily included Windows metadata. Fixed the initial seed to start Steam disconnected, preserve connection state on reseed, and restore the disconnected default after reset. The final full suite passes, including the existing metadata and updated reset regression checks.

## Source behavior prepared

Installed manifests and version-guarded appinfo classification; explicit selection for unknown apps; transactional import keyed by App ID; offline/corrupt-library protection; cached cover/hero/logo imports; player-initiated Windows Steam-location discovery; Steam protocol dispatch; exact executable/creation-identity tracking; pending-launch cancellation/timeout; no adoption of pre-existing games; polite-only exit with refusal feedback; executable picker; Steam settings tile and existing scan reports. Connection/disconnection is atomic. Disconnect disables Steam without marking its games uninstalled, preserving history. Actual uninstalls follow the player's existing removed-game retention policy.

## Checks still required

1. Verify the actual Settings, waiting/cancel screen, tracking picker and close-refusal notice with synthetic data, keyboard and controller. The standalone preview is not actual UI QA.
2. Exercise process handoffs, PID reuse, protected/headless cases and polite window-close refusal/retry. Controlled exact-process detection/exit passed; it does not establish every bootstrapper or anti-cheat variant. The Steam tracker has no forced-termination operation, and the shared launcher's Steam exit branch returns before emulator termination.
3. Agree on a manual real-game launch/exit test, including the explicit executable fallback. No real game was opened automatically.
4. Run the native performance gate on synthetic mixed libraries, Full versus Off, under non-playing conditions. The previous 0.6.0 gate FAILED; its baseline has not been reset and no new performance pass is claimed.
5. After verification, prepare a development executable only after checking for a running development KB process. The current executable has not been replaced. Merge/release/publish and changes to the installed copy remain outside current authorization.

Local cache formats can change. Existing metadata/title fallbacks and cached Steam artwork provide coverage; no new Steam web API or SteamGridDB-ID endpoint was added. A cached executable's existence does not itself establish successful runtime tracking; one of this installation's games already needs the explicit fallback.
