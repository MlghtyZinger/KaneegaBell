import { createMemo, createRoot, createSignal } from "solid-js";
import { api, artUrl, collectionHeroUrl, collectionLogoUrl, events, type Collection, type GameRow } from "../api";
import { library } from "./library";
import { matchesRules } from "./smartFilter";
import { formatLastPlayed, formatPlayTime } from "./session";

/** A picture to show for a collection: a logo (shown whole) or a cover (fills). */
export interface CollectionPicture {
  url: string;
  kind: "logo" | "cover";
}

export type CollectionSort = "az" | "mine" | "newest" | "oldest" | "recent" | "most" | "size" | "genre";
export const COLLECTION_SORTS: { value: CollectionSort; label: string }[] = [
  { value: "az", label: "A–Z" },
  { value: "mine", label: "My order" },
  { value: "newest", label: "Newest release" },
  { value: "oldest", label: "Oldest release" },
  { value: "recent", label: "Recently played" },
  { value: "most", label: "Most played" },
  { value: "size", label: "Most games" },
  { value: "genre", label: "Genre" },
];

function createCollections() {
  const [sort, setSortRaw] = createSignal<CollectionSort>("az");
  function setSort(s: CollectionSort) {
    setSortRaw(s);
    void api.setSetting("collectionSort", s).catch(() => {});
  }
  void api
    .getSetting<string>("collectionSort")
    .then((v) => COLLECTION_SORTS.some((o) => o.value === v) && setSortRaw(v as CollectionSort))
    .catch(() => {});

  const [list, setList] = createSignal<Collection[]>([]);
  /** Hidden series show up too (dimmed), to be shown again. */
  const [showHidden, setShowHidden] = createSignal(false);

  async function reload() {
    try {
      setList(await api.listCollections());
    } catch {
      // Older backend / dev mock without collections: none.
    }
  }

  const byKey = createMemo(() => new Map(list().map((c) => [c.key, c])));
  const rows = createMemo(() => new Map(library.games().map((g) => [g.id, g])));
  /** A smart collection's game ids, computed live from the library and its rules. */
  const smartIds = (c: Collection): number[] =>
    c.kind === "smart" && c.rules ? library.games().filter((g) => matchesRules(g, c.rules!)).map((g) => g.id) : [];
  // The library's "col:<key>" tabs read their games from here.
  library.setCollectionLookup(() => (key: string) => {
    const c = byKey().get(key);
    return c ? (c.kind === "smart" ? smartIds(c) : c.games) : null;
  });
  /** A collection's games that are in the library, in its order (smart: its rules). */
  const gamesOf = (c: Collection): GameRow[] =>
    c.kind === "smart"
      ? smartIds(c).map((id) => rows().get(id)).filter((g): g is GameRow => !!g)
      : c.games.map((id) => rows().get(id)).filter((g): g is GameRow => !!g);
  const shown = createMemo(() =>
    list().filter((c) => (showHidden() || !c.hidden) && (c.kind === "smart" || c.kind === "mine" || gamesOf(c).length >= 2)),
  );
  /** The shown collections in the chosen order (games without a date/play time count as none and sort last). */
  const visible = createMemo(() => {
    const s = sort();
    const items = shown().map((c) => {
      const games = gamesOf(c);
      const years = games.map((g) => g.year).filter((y): y is number => !!y);
      const counts = new Map<number, number>();
      for (const g of games) for (const id of g.genres ?? []) counts.set(id, (counts.get(id) ?? 0) + 1);
      const top = [...counts].sort((a, b) => b[1] - a[1] || a[0] - b[0])[0]?.[0];
      return {
        c,
        newest: years.length ? Math.max(...years) : null,
        oldest: years.length ? Math.min(...years) : null,
        last: Math.max(0, ...games.map((g) => g.lastPlayed ?? 0)),
        played: games.reduce((n, g) => n + g.playSeconds, 0),
        size: games.length,
        genre: top === undefined ? null : (library.genres().find((x) => x.id === top)?.name ?? null),
      };
    });
    const byName = (a: (typeof items)[0], b: (typeof items)[0]) => a.c.name.localeCompare(b.c.name, undefined, { sensitivity: "base", numeric: true });
    // Missing values sort last in either direction.
    const num = (f: (i: (typeof items)[0]) => number | null, dir: 1 | -1) => (a: (typeof items)[0], b: (typeof items)[0]) => {
      const x = f(a), y = f(b);
      if (x === null) return y === null ? byName(a, b) : 1;
      if (y === null) return -1;
      return (x - y) * dir || byName(a, b);
    };
    // Placed ones in their order, then the rest as the backend lists them (yours, then series).
    if (s === "mine") return items.map((i, n) => ({ i, n })).sort((a, b) => (a.i.c.pos ?? Infinity) - (b.i.c.pos ?? Infinity) || a.n - b.n).map((x) => x.i.c);
    const cmp =
      s === "newest" ? num((i) => i.newest, -1)
      : s === "oldest" ? num((i) => i.oldest, 1)
      : s === "recent" ? num((i) => i.last || null, -1)
      : s === "most" ? num((i) => i.played || null, -1)
      : s === "size" ? num((i) => i.size, -1)
      : s === "genre" ? (a: (typeof items)[0], b: (typeof items)[0]) => (a.genre === null ? (b.genre === null ? byName(a, b) : 1) : b.genre === null ? -1 : a.genre.localeCompare(b.genre) || byName(a, b))
      : byName;
    return items.sort(cmp).map((i) => i.c);
  });
  const hiddenCount = createMemo(() => list().filter((c) => c.hidden).length);
  /** The user's own manual collections (games can be added to these). */
  const mine = createMemo(() => list().filter((c) => c.kind === "mine"));
  /** Smart collections (their games follow saved rules). */
  const smart = createMemo(() => list().filter((c) => c.kind === "smart"));

  /** The logo the user picked, if it can be shown. */
  function chosenPicture(c: Collection): CollectionPicture | null {
    const l = c.logo;
    if (!l) return null;
    if (l.type === "file") return { url: collectionLogoUrl(c.key, l.version), kind: "logo" };
    const g = rows().get(l.id);
    if (!g) return null;
    if (l.type === "game") return g.logo ? { url: artUrl(g.id, g.logo, "l"), kind: "logo" } : null;
    return g.art > 0 ? { url: artUrl(g.id, g.art, "f"), kind: "cover" } : null;
  }

  /** Series show their earliest game's clear logo when nothing was picked. */
  function autoLogo(c: Collection): string | null {
    if (c.kind === "mine") return null;
    const g = gamesOf(c).find((m) => (m.logo ?? 0) > 0);
    return g ? artUrl(g.id, g.logo!, "l") : null;
  }

  /** "12 h played · 3 of 4 games · last played yesterday", or "" when nothing was played. */
  function statsLine(c: Collection): string {
    const games = gamesOf(c);
    const played = games.filter((g) => g.playSeconds > 0);
    if (played.length === 0) return "";
    const seconds = played.reduce((n, g) => n + g.playSeconds, 0);
    const last = Math.max(...played.map((g) => g.lastPlayed ?? 0));
    return `${formatPlayTime(seconds)} played · ${played.length} of ${games.length} games · last played ${formatLastPlayed(last || null)}`;
  }

  /** The collection's own TV Mode background, if the user picked one. */
  const ownHero = (c: Collection): string | null => (c.hero ? collectionHeroUrl(c.key, c.hero) : null);

  /** Up to four covers for the mosaic. */
  const mosaic = (c: Collection): string[] =>
    gamesOf(c)
      .filter((g) => g.art > 0)
      .slice(0, 4)
      .map((g) => artUrl(g.id, g.art));

  /** Keeps the list fresh as the library changes. Call once at startup. */
  function listen() {
    void reload();
    void events.onLibraryChanged(() => void reload());
    void events.onWdFinished(() => void reload());
    void events.onScanFinished(() => void reload());
  }

  return { list, reload, listen, byKey, visible, mine, smart, hiddenCount, showHidden, setShowHidden, gamesOf, chosenPicture, autoLogo, mosaic, statsLine, ownHero, sort, setSort };
}

export const collections = createRoot(createCollections);
