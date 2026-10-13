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
  /** The cover's height / width (its card takes this shape). */
  artAspect: number | null;
  /** Genre ids (from IGDB); absent when none. */
  genres?: number[];
  /** TV Mode hero (wide background) / logo versions (SteamGridDB); absent when none. */
  hero?: number;
  logo?: number;
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
  source: "libretro" | "gametdb" | "igdb" | "sgdb" | "local";
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

/** Details from IGDB (all null until matched). */
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
  artSource: "libretro" | "gametdb" | "igdb" | "sgdb" | "local" | "file" | null;
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
export function artUrl(gameId: number, version: number, size: "t" | "f" | "h" | "l" = "t"): string {
  const base = navigator.userAgent.includes("Windows") ? "http://thumb.localhost" : "thumb://localhost";
  return `${base}/${size}/${gameId}?v=${version}`;
}

/** A RetroAchievements badge ("ra") or game icon ("rai"), cached on disk by the app. */
export function raMediaUrl(name: string, kind: "ra" | "rai" = "ra"): string {
  const base = navigator.userAgent.includes("Windows") ? "http://thumb.localhost" : "thumb://localhost";
  return `${base}/${kind}/${encodeURIComponent(name)}`;
}

export interface RunningGame {
  gameId: number;
  title: string;
}

export interface GameExited {
  gameId: number;
  sessionSeconds: number;
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
  kind: "added" | "removed" | "hidden" | "moved";
  platformId: string;
  fromPlatform: string | null;
  title: string;
  variant: string | null;
}

export const api = {
  getLibrary: () => invoke<Library>("get_library"),
  getGame: (id: number) => invoke<GameDetail>("get_game", { id }),
  setFavorite: (id: number, favorite: boolean) => invoke<void>("set_favorite", { id, favorite }),
  previewLaunch: (id: number) => invoke<string[]>("preview_launch", { id }),
  launchGame: (id: number) => invoke<RunningGame>("launch_game", { id }),
  exitGame: () => invoke<void>("exit_game"),
  getRunning: () => invoke<RunningGame | null>("get_running"),

  getSettings: () => invoke<Settings>("get_settings"),
  updateEmulator: (id: string, exePath: string | null, templateOverride: string | null) =>
    invoke<void>("update_emulator", { id, exePath, templateOverride }),
  detectEmulators: (folder: string) => invoke<string[]>("detect_emulators", { folder }),
  updatePlatform: (update: PlatformUpdate) => invoke<void>("update_platform", { update }),
  validateTemplate: (template: string, needsCore: boolean) =>
    invoke<void>("validate_template", { template, needsCore }),
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
  startScan: () => invoke<boolean>("start_scan"),
  cancelScan: () => invoke<void>("cancel_scan"),
  fetchArt: () => invoke<boolean>("fetch_art"),
  refreshArt: (id: number) => invoke<boolean>("refresh_art", { id }),
  coverCandidates: (id: number, query: string | null = null, kind: "cover" | "hero" | "logo" = "cover") =>
    invoke<CoverCandidate[]>("cover_candidates", { id, query, kind }),
  setCover: (id: number, source: CoverCandidate["source"] | "file", name: string, url: string, kind: "cover" | "hero" | "logo" = "cover") =>
    invoke<void>("set_cover", { id, source, name, url, kind }),
  removeCover: (id: number) => invoke<void>("remove_cover", { id }),
  removeExtra: (id: number, kind: "hero" | "logo") => invoke<void>("remove_extra", { id, kind }),
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
  /** Startup timing for the performance harness (a no-op unless EMU_PERF is set). */
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
  onGamepadConnected: (cb: (p: PadInfo) => void): Promise<UnlistenFn> =>
    listen<PadInfo>("gamepad-connected", (e) => cb(e.payload)),
};

/** Errors from commands arrive as plain strings. */
export function errorText(e: unknown): string {
  return typeof e === "string" ? e : e instanceof Error ? e.message : JSON.stringify(e);
}
