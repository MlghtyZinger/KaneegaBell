import { createMemo, createRoot, createSignal } from "solid-js";
import { api, events, type GameRow, type PlatformSummary, type ScanProgress, type ScanSummary } from "../api";

/** "all" shows every platform; otherwise a platform id. */
export type PlatformFilter = "all" | string;

function createLibraryStore() {
  const [platforms, setPlatforms] = createSignal<PlatformSummary[]>([]);
  const [games, setGames] = createSignal<GameRow[]>([]);
  const [loaded, setLoaded] = createSignal(false);
  const [platform, setPlatform] = createSignal<PlatformFilter>("all");
  // Lives here (not in the view) so it survives the grid being unmounted while
  // a game runs: you come back to the game you launched.
  const [focused, setFocused] = createSignal(0);
  const [scanProgress, setScanProgress] = createSignal<ScanProgress | null>(null);
  const [lastScan, setLastScan] = createSignal<ScanSummary | null>(null);
  const [scanError, setScanError] = createSignal<string | null>(null);

  // Rows arrive pre-sorted (platform order, then title), so a filter keeps order.
  const visibleGames = createMemo(() => {
    const p = platform();
    const all = games();
    return p === "all" ? all : all.filter((g) => g.platformId === p);
  });

  const platformTabs = createMemo<PlatformFilter[]>(() => ["all", ...platforms().map((p) => p.id)]);

  async function reload() {
    const lib = await api.getLibrary();
    setPlatforms(lib.platforms);
    setGames(lib.games);
    // Keep the current platform if it still has games.
    if (platform() !== "all" && !lib.platforms.some((p) => p.id === platform())) setPlatform("all");
    setLoaded(true);
  }

  function cyclePlatform(delta: number) {
    const tabs = platformTabs();
    const i = tabs.indexOf(platform());
    setPlatform(tabs[(i + delta + tabs.length) % tabs.length]);
  }

  async function scan() {
    setScanError(null);
    const started = await api.startScan();
    if (started) setScanProgress({ phase: "scanning", source: "", done: 0, total: 0 });
  }

  /** Subscribes to scan events for the app's lifetime. Call once at startup. */
  function listenForScans() {
    void events.onScanProgress((p) => setScanProgress(p.phase === "done" ? null : p));
    void events.onScanFinished((s) => {
      setScanProgress(null);
      setLastScan(s);
      void reload();
    });
    void events.onScanFailed((msg) => {
      setScanProgress(null);
      setScanError(msg);
    });
  }

  return {
    listenForScans,
    platforms,
    games,
    loaded,
    platform,
    setPlatform,
    focused,
    setFocused,
    platformTabs,
    visibleGames,
    cyclePlatform,
    reload,
    scan,
    scanProgress,
    lastScan,
    scanError,
  };
}

export const library = createRoot(createLibraryStore);
