import { createMemo, createRoot, createSignal } from "solid-js";
import {
  api,
  events,
  type ArtProgress,
  type ArtSummary,
  type GameChange,
  type GameRow,
  type PlatformSummary,
  type ScanProgress,
  type ScanSummary,
} from "../api";

/** A library tab: everything, a pseudo-filter, or one platform id. */
export type PlatformFilter = "all" | "favorites" | "recent" | string;

/** Lowercase ASCII words, like the Rust side's search key. */
export function searchKey(s: string): string {
  return s
    .normalize("NFKD")
    .replace(/[̀-ͯ]/g, "")
    .toLowerCase()
    .replace(/['’`]/g, "")
    .replace(/&/g, " and ")
    .replace(/[^a-z0-9]+/g, " ")
    .trim();
}

function createLibraryStore() {
  const [platforms, setPlatforms] = createSignal<PlatformSummary[]>([]);
  const [games, setGames] = createSignal<GameRow[]>([]);
  const [loaded, setLoaded] = createSignal(false);
  const [platform, setPlatform] = createSignal<PlatformFilter>("all");
  // Lives here (not in the view) so it survives the grid being unmounted while
  // a game runs: you come back to the game you launched.
  const [focused, setFocused] = createSignal(0);
  /** Active search text; when set, results span all platforms. */
  const [query, setQuery] = createSignal("");
  const [scanProgress, setScanProgress] = createSignal<ScanProgress | null>(null);
  const [lastScan, setLastScan] = createSignal<ScanSummary | null>(null);
  const [scanError, setScanError] = createSignal<string | null>(null);
  const [artProgress, setArtProgress] = createSignal<ArtProgress | null>(null);
  const [lastArt, setLastArt] = createSignal<ArtSummary | null>(null);
  /** What the last scan (or folder removal) changed; `seq` lets the toast tell reports apart. */
  const [report, setReport] = createSignal<{ title: string; changes: GameChange[]; seq: number } | null>(null);
  let reportSeq = 0;

  // Card size (the grid's smallest card width, px): zoomed with the right
  // stick, +/-, Ctrl+wheel or the slider; saved so it sticks across restarts.
  const CARD_MIN = 120;
  const CARD_MAX = 480;
  const [cardSize, setCardSizeRaw] = createSignal(210);
  let saveTimer: ReturnType<typeof setTimeout> | undefined;
  function setCardSize(px: number) {
    const v = Math.round(Math.min(CARD_MAX, Math.max(CARD_MIN, px)));
    if (v === cardSize()) return;
    setCardSizeRaw(v);
    clearTimeout(saveTimer);
    saveTimer = setTimeout(() => void api.setSetting("cardSize", v).catch(() => {}), 500);
  }
  const zoom = (dir: 1 | -1) => setCardSize(cardSize() * (dir > 0 ? 1.15 : 1 / 1.15));
  void api
    .getSetting<number>("cardSize")
    .then((v) => typeof v === "number" && setCardSizeRaw(Math.min(CARD_MAX, Math.max(CARD_MIN, v))))
    .catch(() => {});

  /** Grid layout: rows (LaunchBox-style) or stacked columns with no gaps. */
  const [stacked, setStackedRaw] = createSignal(false);
  function setStacked(on: boolean) {
    setStackedRaw(on);
    void api.setSetting("gridLayout", on ? "stacked" : "rows").catch(() => {});
  }
  void api
    .getSetting<string>("gridLayout")
    .then((v) => setStackedRaw(v === "stacked"))
    .catch(() => {});
  const publishReport = (title: string, changes: GameChange[]) => setReport({ title, changes, seq: ++reportSeq });

  // Computed once per library load, not per keystroke.
  const keys = createMemo(() => games().map((g) => searchKey(`${g.title} ${g.variant ?? ""}`)));

  const favorites = createMemo(() => games().filter((g) => g.favorite));
  const recent = createMemo(() =>
    games()
      .filter((g) => g.lastPlayed !== null)
      .sort((a, b) => (b.lastPlayed ?? 0) - (a.lastPlayed ?? 0)),
  );

  // Rows arrive pre-sorted (platform order, then title), so filters keep order.
  const visibleGames = createMemo(() => {
    const words = searchKey(query()).split(" ").filter(Boolean);
    if (words.length > 0) {
      const all = games();
      const k = keys();
      // Every typed word must appear somewhere in the title (or variant).
      return all.filter((_, i) => words.every((w) => k[i].includes(w)));
    }
    const p = platform();
    if (p === "all") return games();
    if (p === "favorites") return favorites();
    if (p === "recent") return recent();
    return games().filter((g) => g.platformId === p);
  });

  const platformTabs = createMemo<PlatformFilter[]>(() => [
    "all",
    ...(favorites().length ? ["favorites"] : []),
    ...(recent().length ? ["recent"] : []),
    ...platforms().map((p) => p.id),
  ]);

  async function reload() {
    const lib = await api.getLibrary();
    setPlatforms(lib.platforms);
    setGames(lib.games);
    // Keep the current tab if it still exists.
    if (!platformTabs().includes(platform())) setPlatform("all");
    setLoaded(true);
  }

  function cyclePlatform(delta: number) {
    setQuery("");
    const tabs = platformTabs();
    const i = tabs.indexOf(platform());
    setPlatform(tabs[(i + delta + tabs.length) % tabs.length]);
  }

  async function scan() {
    setScanError(null);
    const started = await api.startScan();
    if (started && !scanProgress()) {
      setScanProgress({ phase: "scanning", source: "", done: 0, total: 0, filesDone: 0, filesTotal: 0 });
    }
  }

  /** Subscribes to scan events for the app's lifetime. Call once at startup. */
  function listenForScans() {
    void events.onScanProgress((p) => setScanProgress(p.phase === "done" ? null : p));
    void events.onScanFinished((s) => {
      setScanProgress(null);
      setLastScan(s);
      publishReport(s.cancelled ? "Scan cancelled" : "Scan finished", s.changes);
      void reload();
    });
    void events.onScanFailed((msg) => {
      setScanProgress(null);
      setScanError(msg);
    });
    void events.onArtProgress((p) => setArtProgress(p.done >= p.total ? null : p));
    void events.onArtFinished((s) => {
      setArtProgress(null);
      setLastArt(s);
      void reload();
    });
    void events.onArtFailed(() => setArtProgress(null));
    void events.onLibraryChanged(() => void reload());
  }

  async function fetchArt() {
    await api.fetchArt();
  }

  /** Removes a ROM folder and reports which games left the library with it. */
  async function removeSource(id: number) {
    const gone = await api.removeSource(id);
    publishReport("Folder removed", gone);
    await reload();
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
    query,
    setQuery,
    platformTabs,
    favorites,
    recent,
    visibleGames,
    cyclePlatform,
    reload,
    scan,
    scanProgress,
    lastScan,
    scanError,
    artProgress,
    lastArt,
    fetchArt,
    report,
    removeSource,
    cardSize,
    setCardSize,
    zoom,
    CARD_MIN,
    CARD_MAX,
    stacked,
    setStacked,
  };
}

export const library = createRoot(createLibraryStore);
