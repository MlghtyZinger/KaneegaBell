import { createMemo, createRoot, createSignal } from "solid-js";
import { api, artUrl, collectionLogoUrl, events, type Collection, type GameRow } from "../api";
import { library } from "./library";
import { matchesRules } from "./smartFilter";

/** A picture to show for a collection: a logo (shown whole) or a cover (fills). */
export interface CollectionPicture {
  url: string;
  kind: "logo" | "cover";
}

function createCollections() {
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
  const visible = createMemo(() =>
    list().filter((c) => (showHidden() || !c.hidden) && (c.kind === "smart" || c.kind === "mine" || gamesOf(c).length >= 2)),
  );
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

  return { list, reload, listen, byKey, visible, mine, smart, hiddenCount, showHidden, setShowHidden, gamesOf, chosenPicture, autoLogo, mosaic };
}

export const collections = createRoot(createCollections);
