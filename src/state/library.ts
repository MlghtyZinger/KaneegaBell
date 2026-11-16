import { createMemo, createRoot, createSignal } from "solid-js";
import {
  api,
  events,
  type ArtProgress,
  type ArtSummary,
  type GameChange,
  type GameRow,
  type Genre,
  type MetaProgress,
  type MetaSummary,
  type PlatformSummary,
  type RaJobProgress,
  type RaSummary,
  type SgdbProgress,
  type SgdbSummary,
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
  const [metaProgress, setMetaProgress] = createSignal<MetaProgress | null>(null);
  const [lastMeta, setLastMeta] = createSignal<MetaSummary | null>(null);
  const [metaError, setMetaError] = createSignal<string | null>(null);
  const [raProgress, setRaProgress] = createSignal<RaJobProgress | null>(null);
  const [lastRa, setLastRa] = createSignal<RaSummary | null>(null);
  const [raError, setRaError] = createSignal<string | null>(null);
  const [sgdbProgress, setSgdbProgress] = createSignal<SgdbProgress | null>(null);
  const [lastSgdb, setLastSgdb] = createSignal<SgdbSummary | null>(null);
  const [sgdbError, setSgdbError] = createSignal<string | null>(null);
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

  /** Mixed views (All, Favorites, search) in per-platform sections, so each row holds one cover shape. */
  const [grouped, setGroupedRaw] = createSignal(true);
  function setGrouped(on: boolean) {
    setGroupedRaw(on);
    void api.setSetting("groupByPlatform", on).catch(() => {});
  }
  void api
    .getSetting<boolean>("groupByPlatform")
    .then((v) => setGroupedRaw(v !== false))
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

  /** Genres (from IGDB) and the active genre filter (applies on top of the tab or search). */
  const [genres, setGenres] = createSignal<Genre[]>([]);
  const [genre, setGenre] = createSignal<number | null>(null);

  // Rows arrive pre-sorted (platform order, then title), so filters keep order.
  const tabOrSearch = createMemo(() => {
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
  const visibleGames = createMemo(() => {
    const id = genre();
    const list = tabOrSearch();
    return id === null ? list : list.filter((g) => g.genres?.includes(id));
  });
  /** How many games in the current tab/search have each genre (for the picker). */
  const genreCounts = createMemo(() => {
    const counts = new Map<number, number>();
    for (const g of tabOrSearch()) for (const id of g.genres ?? []) counts.set(id, (counts.get(id) ?? 0) + 1);
    return counts;
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
    setGenres(lib.genres ?? []);
    if (genre() !== null && !lib.genres?.some((x) => x.id === genre())) setGenre(null);
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
    void events.onMetaProgress((p) => setMetaProgress(p.done >= p.total ? null : p));
    void events.onMetaFinished((s) => {
      setMetaProgress(null);
      setMetaError(null);
      setLastMeta(s);
      void reload();
    });
    void events.onMetaFailed((msg) => {
      setMetaProgress(null);
      setMetaError(msg);
    });
    void events.onRaProgress((p) => setRaProgress(p.done >= p.total ? null : p));
    void events.onRaFinished((s) => {
      setRaProgress(null);
      setRaError(null);
      setLastRa(s);
    });
    void events.onRaFailed((msg) => {
      setRaProgress(null);
      setRaError(msg);
    });
    void events.onSgdbProgress((p) => setSgdbProgress(p.done >= p.total ? null : p));
    void events.onSgdbFinished((s) => {
      setSgdbProgress(null);
      setSgdbError(null);
      setLastSgdb(s);
    });
    void events.onSgdbFailed((msg) => {
      setSgdbProgress(null);
      setSgdbError(msg);
    });
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
    grouped,
    setGrouped,
    genres,
    genre,
    setGenre,
    genreCounts,
    metaProgress,
    lastMeta,
    metaError,
    raProgress,
    lastRa,
    raError,
    sgdbProgress,
    lastSgdb,
    sgdbError,
  };
}

export const library = createRoot(createLibraryStore);
