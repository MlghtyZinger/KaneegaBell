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
}

export interface PlatformSummary {
  id: string;
  name: string;
  shortName: string;
  count: number;
}

export interface Library {
  platforms: PlatformSummary[];
  games: GameRow[];
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
  files: [string, number | null][];
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
  removeSource: (id: number) => invoke<void>("remove_source", { id }),
  suggestPlatform: (path: string) => invoke<string | null>("suggest_platform", { path }),
  listDir: (path: string | null, extensions: string[] = []) =>
    invoke<DirListing>("list_dir", { path, extensions }),
  setSetting: (key: string, value: unknown) => invoke<void>("set_setting", { key, value }),
  startScan: () => invoke<boolean>("start_scan"),
  cancelScan: () => invoke<void>("cancel_scan"),
};

export const events = {
  onScanProgress: (cb: (p: ScanProgress) => void): Promise<UnlistenFn> =>
    listen<ScanProgress>("scan-progress", (e) => cb(e.payload)),
  onScanFinished: (cb: (s: ScanSummary) => void): Promise<UnlistenFn> =>
    listen<ScanSummary>("scan-finished", (e) => cb(e.payload)),
  onScanFailed: (cb: (msg: string) => void): Promise<UnlistenFn> =>
    listen<string>("scan-failed", (e) => cb(e.payload)),
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
