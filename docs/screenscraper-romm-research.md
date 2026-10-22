# ScreenScraper in RomM: implications for KB

Investigated 2026-10-22. Research only; no metadata provider changed.

## Owner update and setup prerequisite (2026-10-22)

The owner now accepts creating a ScreenScraper user account if that is the only
personal setup required. The earlier no-account requirement is superseded.
Rechecked current RomM and ScreenScraper documentation: KB still needs its own
application developer credentials, in addition to the user's account. No
ScreenScraper integration or developer credential configuration was found in
KB's application source. The connection therefore cannot yet be activated.

ScreenScraper directs developers to present their application on its forum to
obtain application credentials. Once KB has these, the intended player-facing
setup can be username/password only, with credentials saved through KB's existing
secret storage. Quotas and concurrency must follow the account's returned limits.
Keep current providers working until the new connection is validated.

Draft application request, prepared but **not sent**:

> Hello ScreenScraper team,
>
> I develop KaneegaBell (KB), a free Windows emulator frontend built with Tauri,
> Rust and SolidJS: https://github.com/MlghtyZinger/KaneegaBell.
> It organizes and launches users' own games; it does not download ROMs or BIOS.
> I would like to integrate ScreenScraper for game metadata and artwork, with
> individual users signing in using their own ScreenScraper accounts. The
> integration will respect account quotas and concurrency limits, cache results,
> and keep credentials out of logs and source control.
>
> Could you grant KaneegaBell application developer credentials and confirm the
> requirements for distributing them in a desktop client so players only need
> their own account login?

No account was created, forum message sent, or credentials requested externally.
The intro work remains paused.

The owner wants ScreenScraper as KB's main provider, with the same setup as
RomM, and no account, API key or developer-key setup. These are recorded as
requirements, not assumed to be achievable by RomM's integration.

## What RomM actually requires

RomM's current setup guide asks the user to create a ScreenScraper account and
configure its username/password. Official Docker images supply the application's
developer credentials at build time, so the user need not obtain developer keys.
Source builds and other packages need that second credential pair configured.
The documented RomM process therefore removes developer setup for ordinary users,
but does not remove the ScreenScraper account requirement.

RomM also supports ordering metadata and artwork providers separately, including
putting ScreenScraper first. Its documentation identifies Hasheous as an option
for users avoiding API keys, with hash matching and proxied IGDB metadata/art.
That is a different service and would need its own KB feasibility check.

Source: [RomM metadata providers](https://docs.romm.app/latest/getting-started/metadata-providers/#screenscraper).

## How its requests work

The service adapter adds developer id/password, software name, and user
username/password to requests. It calls `jeuInfos.php` with available CRC/MD5/SHA1,
system id, ROM type/name/size, serial, or game id. It also supports name searches.
These are identification requests; they do not upload the ROM.

RomM reads account quotas, limits concurrent requests and request rate, and applies
the same limits to media. Daily quota exhaustion stops further provider calls for
the scan. User and application credential failures receive distinct handling;
credentials are redacted from error output. These behaviors are worth reproducing
in KB if an authorized integration becomes feasible.

Source: [RomM ScreenScraper adapter](https://github.com/rommapp/romm/blob/master/backend/adapters/services/screenscraper.py).

## Guest access is conditional, not a dependable replacement

ScreenScraper's own API reference labels user credentials optional on game
lookups, and describes non-member limits. It also explicitly documents HTTP 401
when access closes to non-members/inactive members under load. Thus the old
project note that anonymous access is simply closed was too absolute: guest
access is documented, but availability is conditional. No anonymous API probe
was made in this session because KB has no authorized developer credential pair.

Developer credentials still identify the calling application. The reference
directs application developers to present their software on the forum to obtain
access, limits integrations to free distributed apps unless otherwise approved,
and requires quota management. KB cannot promise reliable account-free main
metadata through RomM's documented process, and cannot claim zero developer
setup for the KB project itself.

Source: [ScreenScraper API v2](https://www.screenscraper.fr/webapi2.php), introduction,
error table, and `jeuInfos.php` parameters.

## Decision for this update

Keep customization milestone u intact. Record ScreenScraper as the desired
future primary provider, contingent on an access model that satisfies the owner.
Do not replace working keyless metadata with an unverified guest-only dependency.
Do not extract or reuse RomM's application credentials for KB.

If access becomes feasible, evaluate game match/media coverage and quotas on an
isolated test profile first. Preserve manual metadata/art choices, offline cache
and fallbacks. Plan hashing separately: existing RetroAchievements hashes use
platform-specific rules and are not automatically ScreenScraper file hashes,
especially for CHD/RVZ/CSO and multi-file games. Do not hash large games at startup.

No account was created, credential requested, ROM/library read, or service
integration added. No build, installer, commit, push or release was performed.
