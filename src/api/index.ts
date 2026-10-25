// Typed wrappers around the Rust commands. Argument keys are camelCase;
// Tauri maps them to the commands' snake_case parameters.
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export interface GameRow {
  id: number;
  platformId: string;
  title: string;
  variant: string | null;
  region: string | null;
  favorite: boolean;
  playSeconds: number;
  lastPlayed: number | null;
  isMultidisc: boolean;
  /** Art version (cache-buster) when the game has art, else 0. */
  art: number;
  /** Square game icon (cache-buster), else 0. */
  icon?: number;
  /** The cover's height / width (its card takes this shape). */
  artAspect: number | null;
  /** Genre ids (from IGDB); absent when none. */
  genres?: number[];
  /** TV Mode hero (wide background) / logo versions (SteamGridDB); absent when none. */
  hero?: number;
  logo?: number;
  /** Cached LaunchBox gameplay screenshot version; absent when none. */
  screenshot?: number;
  /** Developer / publisher / release year (LaunchBox, else IGDB, else Wikidata). */
  developer?: string | null;
  publisher?: string | null;
  year?: number | null;
}

export interface PlatformSummary {
  id: string;
  name: string;
  shortName: string;
  count: number;
  /** Typical cover height/width on this platform (from the library's covers). */
  artAspect: number | null;
}

export interface CoverCandidate {
  /** "lbgame": a LaunchBox entry found by name whose pictures aren't stored yet (`url` is its id; see `lbBrowse`). */
  source: "launchbox" | "libretro" | "gametdb" | "igdb" | "sgdb" | "local" | "lbgame";
  name: string;
  url: string;
  preview: string | null;
  label: string;
}

export interface Genre {
  id: number;
  name: string;
}

export interface Library {
  platforms: PlatformSummary[];
  games: GameRow[];
  /** Genres some game has (from IGDB); empty without an IGDB key. */
  genres: Genre[];
}

/** Game info: LaunchBox's first, IGDB's filling gaps (all null until matched). */
export interface GameMetadata {
  title: string | null;
  description: string | null;
  /** Unix seconds. */
  releaseDate: number | null;
  developer: string | null;
  publisher: string | null;
  genres: string[];
  igdbId: number | null;
  matchStatus: "none" | "ok" | "manual" | "notfound" | "error";
  /** IGDB's URL name (Backloggd uses the same); "" when IGDB has none. */
  slug: string | null;
  /** The LaunchBox match. */
  lbId: number | null;
  lbStatus: "none" | "ok" | "manual" | "notfound" | "ambiguous";
  players: number | null;
  /** Series, then franchises (Wikidata). */
  series: Series[];
}

/** A logo the user picked for a collection or series. */
export type CollectionLogo = { type: "game"; id: number } | { type: "cover"; id: number } | { type: "file"; version: number };

/** A smart collection's saved filter rules. Every field is optional. */
export interface CollectionRules {
  platforms?: string[];
  genres?: number[];
  developers?: string[];
  publishers?: string[];
  yearFrom?: number | null;
  yearTo?: number | null;
  played?: "played" | "unplayed" | null;
  favorite?: boolean;
  /** Play-time bounds in seconds (inclusive). */
  playFrom?: number | null;
  playTo?: number | null;
}

/** One of the user's collections, a series / franchise, or a smart collection. */
export interface Collection {
  /** "c:<id>" (yours, manual or smart) or "s:<series>" (from Wikidata). */
  key: string;
  name: string;
  kind: "mine" | "series" | "franchise" | "smart";
  /** The games in it, in its order (empty for a smart collection; the UI follows its rules). */
  games: number[];
  logo: CollectionLogo | null;
  /** Version of its own TV Mode background, when it has one. */
  hero?: number | null;
  /** Its place in "My order" (set once the user has placed it). */
  pos?: number | null;
  /** Series hidden from the Collections tab. */
  hidden: boolean;
  /** The user's collections: how its games are ordered. */
  gameOrder: "added" | "title" | "release" | null;
  /** The saved filter rules for a smart collection. */
  rules?: CollectionRules | null;
}

/** The app's `thumb` protocol (Windows webviews address custom protocols as http://<scheme>.localhost). */
const THUMB_BASE = navigator.userAgent.includes("Windows") ? "http://thumb.localhost" : "thumb://localhost";

/** A collection's or series' own logo image (the user's file, copied into KB's folder). */
export function collectionLogoUrl(key: string, version: number): string {
  const [kind, id] = key.split(":");
  return `${THUMB_BASE}/${kind === "s" ? "cs" : "cc"}/${id}?v=${version}`;
}

/** A collection's or series' own TV Mode background. */
export function collectionHeroUrl(key: string, version: number): string {
  const [kind, id] = key.split(":");
  return `${THUMB_BASE}/${kind === "s" ? "sh" : "ch"}/${id}?v=${version}`;
}

/** A series or franchise a game belongs to (Wikidata). */
export interface Series {
  /** Its collection key ("s:<item>"), for the logo the user picked. */
  key: string;
  name: string;
  kind: "series" | "franchise";
  /** The game's number in it ("2"), when known. */
  number: string | null;
  /** The library's games in it, this one included, in series order. */
  games: number[];
}

/** A game a source couldn't match (listed by name in Settings). */
export interface Unmatched {
  id: number;
  title: string;
  platform: string;
  /** Several possible matches rather than none. */
  ambiguous: boolean;
}

export interface WdStatus {
  enabled: boolean;
  /** Last time Wikidata was asked (unix seconds). */
  checkedAt: number | null;
  total: number;
  matched: number;
  withSeries: number;
  notFound: number;
  ambiguous: number;
  /** The games behind notFound and ambiguous, by name. */
  unmatched: Unmatched[];
}

export interface WdProgress {
  /** Platforms asked so far, of `total`. */
  done: number;
  total: number;
  /** The platform being asked about ("Switch") and its step; empty when done. */
  platform: string;
  step: "games" | "series" | "";
}

export interface WdSummary {
  platforms: number;
  matched: number;
  notFound: number;
  ambiguous: number;
  withSeries: number;
  cancelled: boolean;
}

/** One result in the LaunchBox "Fix match" search. */
export interface LbHit {
  dbId: number;
  name: string;
  /** The alternate title that matched (another region's name). */
  alsoKnownAs: string | null;
  score: number;
}

export interface LbStatus {
  enabled: boolean;
  /** When LaunchBox's data was last downloaded (unix seconds). */
  importedAt: number | null;
  matched: number;
  total: number;
  notFound: number;
  ambiguous: number;
  /** The games behind notFound and ambiguous, by name. */
  unmatched: Unmatched[];
}

export interface LbProgress {
  phase: "checking" | "downloading" | "reading" | "matching" | "images";
  done: number;
  total: number;
}

export interface LbSummary {
  downloaded: boolean;
  matched: number;
  notFound: number;
  ambiguous: number;
  coversQueued: number;
  heroes: number;
  logos: number;
  cancelled: boolean;
}

/** One result in the "Fix match" search. */
export interface IgdbCandidate {
  igdbId: number;
  name: string;
  year: number | null;
  cover: string | null;
  onPlatform: boolean;
  summary: string | null;
}

export interface Services {
  igdb: boolean;
  igdbClientId: string | null;
  /** RetroAchievements username, when RA is set up. */
  raUser: string | null;
  /** A SteamGridDB key is saved. */
  sgdb: boolean;
}

/** One style in the picker: `swatches` are its background, accent and focus colors. */
export interface ThemeSummary {
  id: string;
  name: string;
  focusLook: string;
  swatches: [string, string, string];
}

/** A style's CSS variables (`:root{...}`) and how it marks the focused cover. */
export interface AppliedTheme {
  id: string;
  css: string;
  focusLook: string;
  /** Which bull icon the style uses. */
  icon: string;
}

export interface AppInfo {
  version: string;
  /** Running from a build folder: no automatic update checks. */
  devBuild: boolean;
}

export interface DisplayRow {
  /** Stable id (survives reboots and re-plugging). */
  id: string;
  /** The monitor's own name, e.g. "LG ULTRAGEAR+" or "SONY TV". */
  name: string;
  width: number;
  height: number;
  scale: number;
  primary: boolean;
  /** KB's window is on it now. */
  current: boolean;
}

export interface Storage {
  /** Covers, backgrounds, logos, badges and cached web responses. */
  downloadedBytes: number;
  dataDir: string;
  /** Size of the library copy the last reset kept, if there is one. */
  resetBackupBytes: number | null;
  /** WebView2's disk cache (picker previews). */
  webCacheBytes: number;
  /** LaunchBox's kept game database file (~100 MB), once downloaded. */
  launchboxBytes: number | null;
}

export interface UpdateInfo {
  version: string;
  currentVersion: string;
  /** Changelog. */
  notes: string | null;
  date: string | null;
}

export interface UpdateProgress {
  downloaded: number;
  total: number | null;
}

export interface SgdbProgress {
  done: number;
  total: number;
  matched: number;
}

export interface SgdbSummary {
  matched: number;
  notFound: number;
  failed: number;
  heroes: number;
  logos: number;
  covers: number;
  cancelled: boolean;
}

export interface Achievement {
  id: number;
  title: string;
  description: string;
  points: number;
  /** Badge image name (see raMediaUrl). */
  badge: string;
  /** "YYYY-MM-DD HH:MM:SS" when unlocked. */
  earned: string | null;
  hardcore: boolean;
  kind: "progression" | "win_condition" | "missable" | null;
  /** Share of the game's players who have it (0..1). */
  rarity: number | null;
}

export interface RaProgress {
  raGameId: number;
  title: string;
  icon: string | null;
  total: number;
  earned: number;
  earnedHardcore: number;
  pointsTotal: number;
  pointsEarned: number;
  achievements: Achievement[];
}

/** Achievement progress at a glance (from the cache). */
export interface AchievementGlance {
  earned: number;
  total: number;
  pointsEarned: number;
  pointsTotal: number;
  raGameId: number;
  /** Up to 5: the latest unlocked first, then the most valuable locked ones. */
  shown: Achievement[];
}

/** A version of a game RetroAchievements accepts. */
export interface AcceptedDump {
  name: string;
  /** Made by applying a patch RA publishes. */
  patched: boolean;
}

export interface GameAchievements {
  /**
   * unsupported = no RA for this console (or RA not set up): show nothing.
   * unknown = RA has this game, but not this version (see accepted).
   * noset = RA has no achievements for a game by this title on this console.
   */
  status: "unsupported" | "nohash" | "unknown" | "noset" | "ok";
  raTitle: string | null;
  raGameId: number | null;
  accepted: AcceptedDump[];
  hash: string | null;
  progress: RaProgress | null;
  /** Unix seconds. */
  fetchedAt: number | null;
  /** Shown from the cache because RA couldn't be reached. */
  stale: boolean;
}

export interface RaJobProgress {
  done: number;
  total: number;
  matched: number;
}

export interface RaSummary {
  hashed: number;
  matched: number;
  unknown: number;
  cancelled: boolean;
}

export interface MetaProgress {
  done: number;
  total: number;
  matched: number;
}

export interface MetaSummary {
  matched: number;
  notFound: number;
  failed: number;
  cancelled: boolean;
  durationMs: number;
}

export interface GameDetail {
  id: number;
  platformId: string;
  title: string;
  variant: string | null;
  region: string | null;
  serial: string | null;
  launchPath: string;
  totalSize: number;
  isMultidisc: boolean;
  playSeconds: number;
  lastPlayed: number | null;
  art: number;
  artSource: "launchbox" | "libretro" | "gametdb" | "igdb" | "sgdb" | "local" | "file" | null;
  files: [string, number | null][];
  meta: GameMetadata;
}

export interface ArtProgress {
  done: number;
  total: number;
  found: number;
}

export interface ArtSummary {
  found: number;
  notFound: number;
  failed: number;
  cancelled: boolean;
  durationMs: number;
}

/**
 * URL of a game's cached artwork, served by the app's `thumb` protocol
 * (Windows webviews address custom protocols as http://<scheme>.localhost).
 */
export function artUrl(gameId: number, version: number, size: "t" | "f" | "h" | "l" | "i" | "d" | "b" | "s" | "q" = "t"): string {
  return `${THUMB_BASE}/${size}/${gameId}?v=${version}`;
}

/** The game's Backloggd page (by its IGDB slug), or a Backloggd search for its title. */
export function backloggdUrl(title: string, slug: string | null | undefined): string {
  return slug
    ? `https://backloggd.com/games/${encodeURIComponent(slug)}/`
    : `https://backloggd.com/search/games/${encodeURIComponent(title)}/`;
}

/** A RetroAchievements badge ("ra") or game icon ("rai"), cached on disk by the app. */
export function raMediaUrl(name: string, kind: "ra" | "rai" = "ra"): string {
  return `${THUMB_BASE}/${kind}/${encodeURIComponent(name)}`;
}

export interface RunningGame {
  gameId: number;
  title: string;
  /** The emulator running it. */
  emulator: string;
  /** Why it isn't the emulator picked for the game (it wasn't set up). */
  notice: string | null;
}

export interface LaunchPreview {
  argv: string[];
  emulator: string;
  notice: string | null;
}

/** An emulator a game can be played with. */
export interface EmulatorOption {
  id: string;
  name: string;
  /** Its exe (and RetroArch core) is there. */
  ready: boolean;
  /** On RetroAchievements' list (null: unknown or the console has none). */
  achievements: boolean | null;
  /** Something that will probably stop it (a RetroArch core's BIOS missing). */
  warning: string | null;
}

export interface GameEmulators {
  defaultId: string;
  /** The game's own pick (null: the platform's default). */
  chosen: string | null;
  options: EmulatorOption[];
}

export interface GameExited {
  gameId: number;
  sessionSeconds: number;
  /** The emulator closed right away without running the game: why, as far as KB can tell. */
  failed: string | null;
}

export interface ExitCombo {
  buttons: string[];
  holdMs: number;
}

export interface DirEntry {
  name: string;
  path: string;
  isDir: boolean;
}

export interface DirListing {
  path: string | null;
  parent: string | null;
  entries: DirEntry[];
  truncated: boolean;
}

export interface PadInfo {
  name: string;
  family: "playstation" | "xbox" | "nintendo" | "generic";
}

export interface EmulatorRow {
  id: string;
  name: string;
  exePath: string | null;
  exeExists: boolean;
  defaultTemplate: string;
  templateOverride: string | null;
  supportsM3u: boolean;
  /** Added by the user ("Add emulator…"). */
  custom: boolean;
  /** On RetroAchievements' list of supported emulators (null: unknown). */
  achievements: boolean | null;
}

export interface PlatformEmulator {
  emulatorId: string;
  core: string | null;
  templateOverride: string | null;
  /** RetroArch only: core not installed, BIOS missing… */
  coreProblem: string | null;
}

export interface PlatformRow {
  id: string;
  name: string;
  shortName: string;
  emulatorId: string;
  defaultCore: string | null;
  coreOverride: string | null;
  templateOverride: string | null;
  defaultExtensions: string[];
  extensionsOverride: string[] | null;
  enabled: boolean;
  gameCount: number;
  /** The default emulator when it isn't the built-in one (emulatorId). */
  emulatorChoice: string | null;
  /** Emulators added for this platform besides the built-in one. */
  extraEmulators: PlatformEmulator[];
  /** Emulators that could still be added. */
  addableEmulators: string[];
  /** The built-in RetroArch core's problem (not installed, BIOS missing…). */
  coreProblem: string | null;
}

export interface SourceRow {
  id: number;
  path: string;
  platformId: string | null;
  recursive: boolean;
  kind: "files" | "ps3_hdd0";
  exists: boolean;
}

export interface Settings {
  emulators: EmulatorRow[];
  platforms: PlatformRow[];
  sources: SourceRow[];
  values: Record<string, unknown>;
  scanning: boolean;
}

export interface PlatformUpdate {
  id: string;
  coreOverride: string | null;
  templateOverride: string | null;
  extensionsOverride: string[] | null;
  enabled: boolean;
}

export interface ScanProgress {
  phase: "scanning" | "done";
  source: string;
  /** Folders finished / total. */
  done: number;
  total: number;
  /** Files examined in the current folder / total. */
  filesDone: number;
  filesTotal: number;
}

export interface ScanSummary {
  added: number;
  updated: number;
  /** Gone from disk, kept hidden because they have play time or are favorites. */
  missing: number;
  /** Gone from disk and deleted. */
  removed: number;
  cancelled: boolean;
  gamesTotal: number;
  durationMs: number;
  warnings: string[];
  changes: GameChange[];
}

/** One line of the scan report. */
export interface GameChange {
  /** hidden = gone from disk but kept for its play time or favorite; moved = now detected as another platform. */
  kind: "added" | "removed" | "hidden" | "moved" | "replaced";
  platformId: string;
  fromPlatform: string | null;
  title: string;
  variant: string | null;
}

export const api = {
  getLibrary: () => invoke<Library>("get_library"),
  getGame: (id: number) => invoke<GameDetail>("get_game", { id }),
  setFavorite: (id: number, favorite: boolean) => invoke<void>("set_favorite", { id, favorite }),
  forgetRecent: (id: number) => invoke<void>("forget_recent", { id }),
  previewLaunch: (id: number) => invoke<LaunchPreview>("preview_launch", { id }),
  /** `emulator`: this launch only (else the game's pick or the platform's default). */
  launchGame: (id: number) => invoke<RunningGame>("launch_game", { id }),
  gameEmulators: (id: number) => invoke<GameEmulators>("game_emulators", { id }),
  /** null: follow the platform's default. */
  setGameEmulator: (id: number, emulator: string | null) => invoke<void>("set_game_emulator", { id, emulator }),
  exitGame: () => invoke<void>("exit_game"),
  getRunning: () => invoke<RunningGame | null>("get_running"),

  getSettings: () => invoke<Settings>("get_settings"),
  updateEmulator: (id: string, exePath: string | null, templateOverride: string | null) =>
    invoke<void>("update_emulator", { id, exePath, templateOverride }),
  detectEmulators: (folder: string) => invoke<string[]>("detect_emulators", { folder }),
  updatePlatform: (update: PlatformUpdate) => invoke<void>("update_platform", { update }),
  addCustomEmulator: (name: string, exePath: string | null, template: string, supportsM3u: boolean) =>
    invoke<string>("add_custom_emulator", { name, exePath, template, supportsM3u }),
  updateCustomEmulator: (id: string, name: string, template: string, supportsM3u: boolean) =>
    invoke<void>("update_custom_emulator", { id, name, template, supportsM3u }),
  removeCustomEmulator: (id: string) => invoke<void>("remove_custom_emulator", { id }),
  setPlatformDefault: (platform: string, emulator: string) => invoke<void>("set_platform_default", { platform, emulator }),
  addPlatformEmulator: (platform: string, emulator: string) => invoke<void>("add_platform_emulator", { platform, emulator }),
  updatePlatformEmulator: (platform: string, emulator: string, core: string | null, templateOverride: string | null) =>
    invoke<void>("update_platform_emulator", { platform, emulator, core, templateOverride }),
  removePlatformEmulator: (platform: string, emulator: string) =>
    invoke<void>("remove_platform_emulator", { platform, emulator }),
  addSource: (path: string, platformId: string | null) =>
    invoke<number>("add_source", { path, platformId }),
  addRpcs3InstalledSource: () => invoke<number>("add_rpcs3_installed_source"),
  updateSource: (id: number, platformId: string | null, recursive: boolean) =>
    invoke<void>("update_source", { id, platformId, recursive }),
  removeSource: (id: number) => invoke<GameChange[]>("remove_source", { id }),
  suggestPlatform: (path: string) => invoke<string | null>("suggest_platform", { path }),
  listDir: (path: string | null, extensions: string[] = []) =>
    invoke<DirListing>("list_dir", { path, extensions }),
  getSetting: <T,>(key: string) => invoke<T | null>("get_setting", { key }),
  setSetting: (key: string, value: unknown) => invoke<void>("set_setting", { key, value }),
  getSettingValues: (keys: string[]) => invoke<Record<string, unknown>>("get_setting_values", { keys }),
  /** The user's own sound file for a start-up intro (data\intros), empty when there's none. */
  introSound: (key: string) => invoke<ArrayBuffer>("intro_sound", { key }),
  startScan: () => invoke<boolean>("start_scan"),
  cancelScan: () => invoke<void>("cancel_scan"),
  fetchArt: () => invoke<boolean>("fetch_art"),
  /** "kept" when the cover is the user's own pick (Refresh leaves it). */
  refreshArt: (id: number) => invoke<"queued" | "kept">("refresh_art", { id }),
  coverCandidates: (id: number, query: string | null = null, kind: "cover" | "hero" | "logo" | "icon" | "back" | "disc" | "spine" = "cover") =>
    invoke<CoverCandidate[]>("cover_candidates", { id, query, kind }),
  setCover: (id: number, source: CoverCandidate["source"] | "file", name: string, url: string, kind: "cover" | "hero" | "logo" | "icon" | "back" | "disc" | "spine" = "cover") =>
    invoke<void>("set_cover", { id, source, name, url, kind }),
  removeCover: (id: number) => invoke<void>("remove_cover", { id }),
  removeExtra: (id: number, kind: "hero" | "logo" | "icon") => invoke<void>("remove_extra", { id, kind }),
  useCoverIcon: (id: number) => invoke<void>("use_cover_icon", { id }),
  lbStatus: () => invoke<LbStatus>("lb_status"),
  /** A LaunchBox pass; `force` downloads its data again ("Update now"). */
  fetchLaunchbox: (force: boolean) => invoke<boolean>("fetch_launchbox", { force }),
  setLbEnabled: (enabled: boolean) => invoke<void>("set_lb_enabled", { enabled }),
  lbSearch: (id: number, query: string | null) => invoke<LbHit[]>("lb_search", { id, query }),
  setLbMatch: (id: number, dbId: number) => invoke<void>("set_lb_match", { id, dbId }),
  /** Has a LaunchBox pass keep those entries' pictures (downloads LaunchBox's file once if needed). */
  lbBrowse: (dbIds: number[]) => invoke<void>("lb_browse", { dbIds }),
  wdStatus: () => invoke<WdStatus>("wd_status"),
  /** Games hidden because their files are gone. */
  getRemoved: () => invoke<{ count: number }>("get_removed"),
  /** Forgets the removed games for good; returns how many. */
  forgetRemoved: () => invoke<number>("forget_removed"),
  listCollections: () => invoke<Collection[]>("list_collections"),
  createCollection: (name: string, gameId: number | null) => invoke<string>("create_collection", { name, gameId }),
  createSmartCollection: (name: string, rules: CollectionRules) => invoke<string>("create_smart_collection", { name, rules }),
  renameCollection: (key: string, name: string) => invoke<void>("rename_collection", { key, name }),
  setCollectionRules: (key: string, rules: CollectionRules) => invoke<void>("set_collection_rules", { key, rules }),
  deleteCollection: (key: string) => invoke<void>("delete_collection", { key }),
  setInCollection: (key: string, gameId: number, member: boolean) => invoke<void>("set_in_collection", { key, gameId, member }),
  setCollectionOrder: (key: string, order: "added" | "title" | "release") => invoke<void>("set_collection_order", { key, order }),
  setCollectionOrderAll: (keys: string[]) => invoke<void>("set_collection_order_all", { keys }),
  setCollectionHidden: (key: string, hidden: boolean) => invoke<void>("set_collection_hidden", { key, hidden }),
  /** "auto", "game:<id>" (its clear logo) or "cover:<id>". */
  setCollectionLogo: (key: string, choice: string) => invoke<void>("set_collection_logo", { key, choice }),
  setCollectionLogoFile: (key: string, path: string) => invoke<void>("set_collection_logo_file", { key, path }),
  collectionImageCandidates: (query: string, kind: "logo" | "hero") => invoke<CoverCandidate[]>("collection_image_candidates", { query, kind }),
  setCollectionImageUrl: (key: string, url: string, kind: "logo" | "hero") => invoke<void>("set_collection_image_url", { key, url, kind }),
  removeCollectionHero: (key: string) => invoke<void>("remove_collection_hero", { key }),
  /** A cover's main color (`#rrggbb`), to tint its series; null without one. */
  coverAccent: (id: number) => invoke<string | null>("cover_accent", { id }),
  /** The colours of a cover's top and bottom edges (its 3D case's top and bottom faces). */
  coverEdges: (id: number) => invoke<[string, string] | null>("cover_edges", { id }),
  /** A Wikidata pass; `force` asks about every platform again ("Update now"). */
  fetchWikidata: (force: boolean) => invoke<boolean>("fetch_wikidata", { force }),
  setWdEnabled: (enabled: boolean) => invoke<void>("set_wd_enabled", { enabled }),
  setSgdbKey: (key: string) => invoke<void>("set_sgdb_key", { key }),
  removeSgdbKey: () => invoke<void>("remove_sgdb_key"),
  fetchExtras: () => invoke<boolean>("fetch_extras"),

  getServices: () => invoke<Services>("get_services"),
  setIgdbKeys: (clientId: string, clientSecret: string) => invoke<void>("set_igdb_keys", { clientId, clientSecret }),
  removeIgdbKeys: () => invoke<void>("remove_igdb_keys"),
  fetchMetadata: () => invoke<boolean>("fetch_metadata"),
  igdbSearch: (id: number, query: string | null = null) => invoke<IgdbCandidate[]>("igdb_search", { id, query }),
  setIgdbMatch: (id: number, igdbId: number) => invoke<void>("set_igdb_match", { id, igdbId }),
  /** Returns the username as RetroAchievements spells it. */
  setRaKeys: (user: string, key: string) => invoke<string>("set_ra_keys", { user, key }),
  removeRaKeys: () => invoke<void>("remove_ra_keys"),
  getAchievements: (id: number, refresh = false) => invoke<GameAchievements>("get_achievements", { id, refresh }),
  /** Cached achievement progress for the sidebar (never goes online); null when none is cached. */
  achievementGlance: (id: number) => invoke<AchievementGlance | null>("achievement_glance", { id }),
  appInfo: () => invoke<AppInfo>("app_info"),
  listDisplays: () => invoke<DisplayRow[]>("list_displays"),
  /** TV Mode's window: fullscreen, on the chosen TV display when it's connected. */
  tvEnter: () => invoke<{ displayMissing: boolean }>("tv_enter"),
  /** Back to the window's place from before TV Mode. */
  tvLeave: () => invoke<void>("tv_leave"),
  /** Re-fit the page zoom (after the Interface size setting changes). */
  refreshZoom: () => invoke<void>("refresh_zoom"),
  /** The built-in styles, and the id of the one in use. */
  listThemes: () => invoke<[ThemeSummary[], string]>("list_themes"),
  /** Saves the style; the window icon changes with it. Apply the result with `applyTheme`. */
  setTheme: (id: string) => invoke<AppliedTheme>("set_theme", { id }),
  getStorage: () => invoke<Storage>("get_storage"),
  /** Deletes downloaded art/info/achievement data and fetches it again; returns bytes freed. */
  clearDownloaded: () => invoke<number>("clear_downloaded"),
  clearWebCache: () => invoke<number>("clear_web_cache"),
  /** Deletes the library copy the last reset kept. */
  deleteResetBackup: () => invoke<void>("delete_reset_backup"),
  deleteLaunchboxFile: () => invoke<void>("delete_launchbox_file"),
  /** Back to a first start (keeps a copy of the library); reload the page afterwards. */
  resetApp: (forgetKeys: boolean) => invoke<void>("reset_app", { forgetKeys }),
  /** Looks for a newer KB release; null when up to date. */
  checkForUpdate: () => invoke<UpdateInfo | null>("check_for_update"),
  /** Downloads and installs the update found by the last check (KB restarts). */
  installUpdate: () => invoke<void>("install_update"),
  /** Saves an update access code (checked against the updates first). */
  setUpdateCode: (code: string) => invoke<void>("set_update_code", { code }),
  removeUpdateCode: () => invoke<void>("remove_update_code"),
  hasUpdateCode: () => invoke<boolean>("has_update_code"),
  /** A one-time message from startup (e.g. the library was moved), or null. */
  getStartupNotice: () => invoke<string | null>("get_startup_notice"),
  /** Opens a page on a known site (Backloggd, RetroAchievements...) in the default browser. */
  openUrl: (url: string) => invoke<void>("open_url", { url }),
  shutdownPc: () => invoke<void>("shutdown_pc"),
  /** Startup timing for the performance harness (a no-op unless KB_PERF is set). */
  perfMark: (name: string) => invoke<void>("perf_mark", { name }),
};

export const events = {
  onScanProgress: (cb: (p: ScanProgress) => void): Promise<UnlistenFn> =>
    listen<ScanProgress>("scan-progress", (e) => cb(e.payload)),
  onScanFinished: (cb: (s: ScanSummary) => void): Promise<UnlistenFn> =>
    listen<ScanSummary>("scan-finished", (e) => cb(e.payload)),
  onScanFailed: (cb: (msg: string) => void): Promise<UnlistenFn> =>
    listen<string>("scan-failed", (e) => cb(e.payload)),
  onArtProgress: (cb: (p: ArtProgress) => void): Promise<UnlistenFn> =>
    listen<ArtProgress>("art-progress", (e) => cb(e.payload)),
  onArtFinished: (cb: (s: ArtSummary) => void): Promise<UnlistenFn> =>
    listen<ArtSummary>("art-finished", (e) => cb(e.payload)),
  onArtFailed: (cb: (msg: string) => void): Promise<UnlistenFn> =>
    listen<string>("art-failed", (e) => cb(e.payload)),
  onMetaProgress: (cb: (p: MetaProgress) => void): Promise<UnlistenFn> =>
    listen<MetaProgress>("meta-progress", (e) => cb(e.payload)),
  onMetaFinished: (cb: (s: MetaSummary) => void): Promise<UnlistenFn> =>
    listen<MetaSummary>("meta-finished", (e) => cb(e.payload)),
  onMetaFailed: (cb: (msg: string) => void): Promise<UnlistenFn> =>
    listen<string>("meta-failed", (e) => cb(e.payload)),
  onUpdateAvailable: (cb: (u: UpdateInfo) => void): Promise<UnlistenFn> =>
    listen<UpdateInfo>("update-available", (e) => cb(e.payload)),
  /** The update access code is missing or stopped working (message to show). */
  onUpdateAccess: (cb: (message: string) => void): Promise<UnlistenFn> =>
    listen<string>("update-access", (e) => cb(e.payload)),
  onUpdateProgress: (cb: (p: UpdateProgress) => void): Promise<UnlistenFn> =>
    listen<UpdateProgress>("update-progress", (e) => cb(e.payload)),
  onLbProgress: (cb: (p: LbProgress) => void): Promise<UnlistenFn> => listen<LbProgress>("lb-progress", (e) => cb(e.payload)),
  onLbFinished: (cb: (s: LbSummary) => void): Promise<UnlistenFn> => listen<LbSummary>("lb-finished", (e) => cb(e.payload)),
  onLbFailed: (cb: (msg: string) => void): Promise<UnlistenFn> => listen<string>("lb-failed", (e) => cb(e.payload)),
  onWdProgress: (cb: (p: WdProgress) => void): Promise<UnlistenFn> => listen<WdProgress>("wd-progress", (e) => cb(e.payload)),
  onWdFinished: (cb: (s: WdSummary) => void): Promise<UnlistenFn> => listen<WdSummary>("wd-finished", (e) => cb(e.payload)),
  onWdFailed: (cb: (msg: string) => void): Promise<UnlistenFn> => listen<string>("wd-failed", (e) => cb(e.payload)),
  onSgdbProgress: (cb: (p: SgdbProgress) => void): Promise<UnlistenFn> =>
    listen<SgdbProgress>("sgdb-progress", (e) => cb(e.payload)),
  onSgdbFinished: (cb: (s: SgdbSummary) => void): Promise<UnlistenFn> =>
    listen<SgdbSummary>("sgdb-finished", (e) => cb(e.payload)),
  onSgdbFailed: (cb: (msg: string) => void): Promise<UnlistenFn> =>
    listen<string>("sgdb-failed", (e) => cb(e.payload)),
  onRaProgress: (cb: (p: RaJobProgress) => void): Promise<UnlistenFn> =>
    listen<RaJobProgress>("ra-progress", (e) => cb(e.payload)),
  onRaFinished: (cb: (s: RaSummary) => void): Promise<UnlistenFn> =>
    listen<RaSummary>("ra-finished", (e) => cb(e.payload)),
  onRaFailed: (cb: (msg: string) => void): Promise<UnlistenFn> =>
    listen<string>("ra-failed", (e) => cb(e.payload)),
  /** Data changed in the background (e.g. covers arriving): reload the library. */
  onLibraryChanged: (cb: () => void): Promise<UnlistenFn> => listen("library-changed", () => cb()),
  onGameStarted: (cb: (g: RunningGame) => void): Promise<UnlistenFn> =>
    listen<RunningGame>("game-started", (e) => cb(e.payload)),
  onGameExited: (cb: (g: GameExited) => void): Promise<UnlistenFn> =>
    listen<GameExited>("game-exited", (e) => cb(e.payload)),
  /** Controller navigation actions from the Rust gamepad thread. */
  onNav: (cb: (action: string) => void): Promise<UnlistenFn> => listen<string>("nav", (e) => cb(e.payload)),
  onRightStick: (cb: (axes: [number, number]) => void): Promise<UnlistenFn> => listen<[number, number]>("right-stick", (e) => cb(e.payload)),
  onGamepadConnected: (cb: (p: PadInfo) => void): Promise<UnlistenFn> =>
    listen<PadInfo>("gamepad-connected", (e) => cb(e.payload)),
};

/** Errors from commands arrive as plain strings. */
export function errorText(e: unknown): string {
  return typeof e === "string" ? e : e instanceof Error ? e.message : JSON.stringify(e);
}
